use anyhow::Result;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};
use url::Url;

use super::{
    colour::extract_palette,
    event::{Event, LyricLine, TrackInfo},
    lrclib,
};

#[derive(Debug, Clone, PartialEq)]
struct MetadataUpdate {
    title: String,
    artist: String,
    album: String,
    art_url: Option<String>,
    track_id: String,
}

impl MetadataUpdate {
    fn from_metadata(metadata: &mpris::Metadata) -> Self {
        Self {
            title: metadata.title().unwrap_or("Unknown").to_string(),
            artist: metadata.artists().unwrap_or_default().join(", "),
            album: metadata.album_name().unwrap_or("").to_string(),
            art_url: metadata.art_url().map(|s| s.to_string()),
            track_id: metadata
                .track_id()
                .map(|id| id.to_string())
                .unwrap_or_default(),
        }
    }
}

enum MprisUpdate {
    Metadata(MetadataUpdate),
    Status(mpris::PlaybackStatus),
    Position(std::time::Duration),
}

#[derive(serde::Deserialize)]
struct ITunesResponse {
    results: Vec<ITunesResult>,
}

#[derive(serde::Deserialize)]
struct ITunesResult {
    #[serde(rename = "artworkUrl100")]
    artwork_url: Option<String>,
}

#[derive(serde::Deserialize)]
struct CanvasResponse {
    #[serde(rename = "canvas_url")]
    url: Option<String>,
}

pub struct MprisWatcher;

impl MprisWatcher {
    pub async fn run(
        tx: Sender<Event>,
        mut visible_rx: tokio::sync::watch::Receiver<bool>,
        mut show_lyrics_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        info!("MPRIS watcher started");

        // The reqwest Client builder can perform blocking I/O (e.g. loading system certificates).
        // Since `run` is an async function, we wrap this in `spawn_blocking` to avoid stalling the executor.
        let http_client = tokio::task::spawn_blocking(|| {
            reqwest::Client::builder()
                .user_agent("cosmic-wallpaper/1.0")
                .timeout(std::time::Duration::from_secs(10))
                .build()
        })
        .await
        .map_err(|e| anyhow::anyhow!("Tokio spawn_blocking failed: {}", e))??;
        let (update_tx, mut update_rx) = tokio::sync::mpsc::channel(16);
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();

        let poll_visible = visible_rx.clone();
        std::thread::spawn(move || {
            let mut finder = mpris::PlayerFinder::new();
            let mut last_status = mpris::PlaybackStatus::Stopped;
            let mut last_track_id = String::new();

            loop {
                if shutdown_rx.recv_timeout(std::time::Duration::from_millis(500))
                    != Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                {
                    break;
                }

                // Suspend D-Bus heavy polling when wallpaper is out of view
                if !*poll_visible.borrow() {
                    continue;
                }

                let f = match finder.as_ref() {
                    Ok(f) => f,
                    Err(_) => {
                        finder = mpris::PlayerFinder::new();
                        continue;
                    }
                };

                // Prioritize any player that is currently playing
                let active_player = f
                    .find_all()
                    .ok()
                    .and_then(|players| {
                        players.into_iter().find(|p| {
                            p.get_playback_status()
                                .unwrap_or(mpris::PlaybackStatus::Stopped)
                                == mpris::PlaybackStatus::Playing
                        })
                    })
                    .or_else(|| f.find_active().ok());

                if let Some(player) = active_player {
                    let current_status = player
                        .get_playback_status()
                        .unwrap_or(mpris::PlaybackStatus::Stopped);
                    let metadata_opt = player.get_metadata().ok();
                    let current_track_id_raw = metadata_opt
                        .as_ref()
                        .and_then(|m| m.track_id().map(|id| id.to_string()))
                        .unwrap_or_default();

                    let metadata_update = metadata_opt.as_ref().map(MetadataUpdate::from_metadata);

                    let effective_track_id = if current_track_id_raw.is_empty() {
                        // Fallback to title + artist for track id if it's missing
                        if let Some(metadata) = &metadata_update {
                            format!("{}-{}", metadata.title, metadata.artist)
                        } else {
                            String::new()
                        }
                    } else {
                        current_track_id_raw
                    };

                    if effective_track_id != last_track_id {
                        if let Some(metadata) = metadata_update {
                            let _ = update_tx.blocking_send(MprisUpdate::Metadata(metadata));
                        }
                        last_track_id = effective_track_id;
                    }

                    if current_status != last_status {
                        let _ = update_tx.blocking_send(MprisUpdate::Status(current_status));
                        last_status = current_status;
                    }

                    if current_status == mpris::PlaybackStatus::Playing {
                        if let Ok(pos) = player.get_position() {
                            // Use try_send for high-frequency, non-critical updates to avoid backpressure
                            match update_tx.try_send(MprisUpdate::Position(pos)) {
                                Ok(_) => {}
                                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {}
                                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break,
                            }
                        }
                    }
                } else if last_status != mpris::PlaybackStatus::Stopped {
                    let _ = update_tx
                        .blocking_send(MprisUpdate::Status(mpris::PlaybackStatus::Stopped));
                    last_status = mpris::PlaybackStatus::Stopped;
                    last_track_id.clear();
                }
            }
        });

        // Ensure shutdown signals are preserved across the async execution to keep threads alive
        let _shutdown_guard = shutdown_tx;

        let mut is_playing = false;
        let mut is_timed_out = false;
        let mut paused_since: Option<tokio::time::Instant> = None;
        let timeout_duration = tokio::time::Duration::from_secs(15);
        let mut last_metadata: Option<MetadataUpdate> = None;
        let mut last_processed_metadata: Option<MetadataUpdate> = None;

        let cache_cap = std::num::NonZeroUsize::new(50)
            .ok_or_else(|| anyhow::anyhow!("Failed to initialize non-zero cache capacity"))?;
        let mut palette_cache: lru::LruCache<String, Box<[[f32; 3]]>> =
            lru::LruCache::new(cache_cap);
        let mut lyrics_cache: lru::LruCache<String, Option<Box<[LyricLine]>>> =
            lru::LruCache::new(cache_cap);
        let mut video_cache: lru::LruCache<String, Option<String>> = lru::LruCache::new(cache_cap);
        let mut video_cancel_tx: Option<tokio::sync::watch::Sender<bool>> = None;

        let mut last_show_lyrics = *show_lyrics_rx.borrow();

        loop {
            let update_opt = tokio::select! {
                Ok(_) = visible_rx.changed() => { None }
                Ok(_) = show_lyrics_rx.changed() => {
                    let current_show_lyrics = *show_lyrics_rx.borrow();
                    if current_show_lyrics != last_show_lyrics {
                        last_show_lyrics = current_show_lyrics;
                        if current_show_lyrics {
                            last_processed_metadata = None;
                        }
                    }
                    None
                }
                res = tokio::time::timeout(tokio::time::Duration::from_millis(250), update_rx.recv()) => {
                    match res {
                        Ok(Some(u)) => Some(u),
                        Ok(None) => break,
                        Err(_) => None,
                    }
                }
            };

            let visible = *visible_rx.borrow();
            let current_show_lyrics = *show_lyrics_rx.borrow();

            if let Some(update) = update_opt {
                // If we receive any event at all, it means a player is active, so we are no longer timed out.
                is_timed_out = false;

                match update {
                    MprisUpdate::Metadata(meta) => {
                        let is_empty = (meta.title == "Unknown" || meta.title.trim().is_empty())
                            && meta.artist.trim().is_empty();
                        if !is_empty {
                            last_metadata = Some(meta);
                        }
                    }
                    MprisUpdate::Status(status) => {
                        let playing = status == mpris::PlaybackStatus::Playing;
                        if playing != is_playing {
                            is_playing = playing;
                            if is_playing {
                                info!("Playback resumed");
                                is_timed_out = false;
                                paused_since = None;
                                let _ = tx.send(Event::PlaybackResumed).await;
                            } else {
                                info!("Playback paused/stopped");
                                paused_since = Some(tokio::time::Instant::now());
                                let _ = tx.send(Event::PlaybackStopped).await;
                            }
                        } else if !playing && paused_since.is_none() && !is_timed_out {
                            // If we start up and a player is already paused, start the timer
                            paused_since = Some(tokio::time::Instant::now());
                        }
                    }
                    MprisUpdate::Position(pos) => {
                        // Drop continuous position updates if we aren't rendering to save channel capacity
                        if visible {
                            let _ = tx.send(Event::PlaybackPosition(pos)).await;
                        }
                    }
                }
            }

            // Check if we should relinquish the source due to pause timeout
            if !is_playing && !is_timed_out {
                if let Some(time) = paused_since {
                    if time.elapsed() >= timeout_duration {
                        info!("Relinquishing MPRIS source due to inactivity timeout");
                        is_timed_out = true;
                        paused_since = None;
                        last_metadata = None;
                        last_processed_metadata = None;
                        if let Some(cancel) = video_cancel_tx.take() {
                            let _ = cancel.send(true);
                        }
                        let _ = tx.send(Event::PlayerShutDown).await;
                    }
                }
            }

            // If we are visible and have unprocessed metadata, fetch the art, palette, and lyrics!
            if visible && !is_timed_out && last_metadata != last_processed_metadata {
                if let Some(meta) = last_metadata.as_ref() {
                    info!("Fetching track info for: {} - {}", meta.artist, meta.title);
                    let track_info = Self::build_track_info(
                        meta,
                        &mut palette_cache,
                        &mut lyrics_cache,
                        &mut video_cache,
                        current_show_lyrics,
                        &http_client,
                    )
                    .await;

                    // Safely kill the previous FFmpeg process if one is running
                    if let Some(cancel) = video_cancel_tx.take() {
                        let _ = cancel.send(true);
                    }
                    // Spin up the new FFmpeg background decoder pipeline!
                    if let Some(url) = &track_info.video_url {
                        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
                        let (recycle_tx, recycle_rx) = tokio::sync::mpsc::channel(3);
                        video_cancel_tx = Some(cancel_tx);
                        let tx_clone = tx.clone();
                        let url_clone = url.clone();
                        tokio::spawn(async move {
                            let _ = super::video::VideoDecoder::run_decoder(
                                url_clone.to_string(),
                                tx_clone,
                                cancel_rx,
                                recycle_rx,
                                recycle_tx,
                            )
                            .await;
                        });
                    }

                    let _ = tx.send(Event::TrackChanged(Box::new(track_info))).await;
                    last_processed_metadata = Some(meta.clone());
                }
            }
        }

        Ok(())
    }

    async fn build_track_info(
        meta: &MetadataUpdate,
        palette_cache: &mut lru::LruCache<String, Box<[[f32; 3]]>>,
        lyrics_cache: &mut lru::LruCache<String, Option<Box<[LyricLine]>>>,
        video_cache: &mut lru::LruCache<String, Option<String>>,
        fetch_lyrics: bool,
        client: &reqwest::Client,
    ) -> TrackInfo {
        let cache_key = meta
            .art_url
            .clone()
            .unwrap_or_else(|| format!("fallback:{}:{}", meta.artist, meta.album));
        let cached_palette = palette_cache.get(&cache_key).cloned();

        let lyrics_cache_key = format!("{}:{}:{}", meta.artist, meta.album, meta.title);
        let cached_lyrics = lyrics_cache.get(&lyrics_cache_key).cloned();

        let art_future = async {
            let mut local_img = None;
            if let Some(art_url) = &meta.art_url {
                info!("Extracted art_url from MPRIS metadata: {}", art_url);
                match Self::fetch_album_art(art_url, client).await {
                    Ok(img) => {
                        local_img = Some(img);
                        info!("Successfully loaded and decoded primary album art!");
                    }
                    Err(e) => {
                        warn!(
                            "Failed to load local album art (likely Flatpak isolation): {}",
                            e
                        );
                    }
                }
            }

            let final_img = if let Some(img) = local_img {
                Some(img)
            } else {
                info!("Attempting to fetch fallback album art online...");
                match Self::fetch_fallback_album_art(&meta.artist, &meta.album, &meta.title, client)
                    .await
                {
                    Ok(img) => Some(img),
                    Err(e) => {
                        warn!(
                            "Fallback art failed: {}. Generating dynamic placeholder.",
                            e
                        );
                        let mut img = image::RgbaImage::new(640, 640);
                        for y in 0..640 {
                            for x in 0..640 {
                                let r = ((x as f32 / 640.0) * 80.0) as u8 + 20;
                                let b = ((y as f32 / 640.0) * 80.0) as u8 + 40;
                                img.put_pixel(x, y, image::Rgba([r, 20, b, 255]));
                            }
                        }
                        Some(image::DynamicImage::ImageRgba8(img))
                    }
                }
            };

            if let Some(img) = final_img {
                let cached = cached_palette.clone();
                // Optimization: Offload heavy CPU-bound palette extraction and image conversion
                // to a dedicated blocking thread. This saves ~50-100ms of executor stall time.
                tokio::task::spawn_blocking(move || {
                    let palette = cached.unwrap_or_else(|| extract_palette(&img));

                    // Optimisation: Limit album art size to 1024x1024 to prevent massive RAM usage.
                    let resized_img = if img.width() > 1024 || img.height() > 1024 {
                        img.thumbnail(1024, 1024)
                    } else {
                        img
                    };

                    let rgba = resized_img.into_rgba8();
                    (Some(rgba), Some(palette))
                })
                .await
                .unwrap_or((None, None))
            } else {
                (None, None)
            }
        };

        let lyrics_future = async {
            if !fetch_lyrics {
                None
            } else if let Some(cached) = cached_lyrics.clone() {
                cached
            } else {
                lrclib::fetch_synced_lyrics(&meta.title, &meta.artist, &meta.album, client).await
            }
        };

        // Optimization: Extract track ID once to avoid redundant string searches and allocations
        // The track_id is needed both for fetching the canvas and caching the result
        let raw_id = if meta.track_id.contains("spotify:track:") {
            meta.track_id.split(':').next_back()
        } else if meta.track_id.contains("open.spotify.com/track/")
            || meta.track_id.contains("/com/spotify/track/")
        {
            meta.track_id.split('/').next_back()
        } else {
            None
        };

        let video_future = async {
            if let Some(id) = raw_id {
                if let Some(cached) = video_cache.get(id) {
                    cached.clone()
                } else {
                    Self::fetch_spotify_canvas(id, client).await
                }
            } else {
                None
            }
        };

        let ((album_art, palette), lyrics, video_url) =
            tokio::join!(art_future, lyrics_future, video_future);

        if cached_palette.is_none() {
            if let Some(p) = &palette {
                palette_cache.put(cache_key, p.clone());
            }
        }

        if cached_lyrics.is_none() && fetch_lyrics {
            lyrics_cache.put(lyrics_cache_key, lyrics.clone());
        }

        if let Some(id) = raw_id {
            if !video_cache.contains(id) {
                video_cache.put(id.to_string(), video_url.clone());
            }
        }

        if lyrics.is_some() {
            info!("Synced lyrics loaded for {} - {}", meta.artist, meta.title);
        }

        TrackInfo {
            title: meta.title.clone().into_boxed_str(),
            artist: meta.artist.clone().into_boxed_str(),
            album: meta.album.clone().into_boxed_str(),
            album_art,
            palette,
            lyrics,
            video_url: video_url.map(|u| u.into_boxed_str()),
        }
    }

    fn decode_image_safely(bytes: impl AsRef<[u8]>) -> Result<image::DynamicImage> {
        let mut reader = image::io::Reader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| anyhow::anyhow!("Failed to guess image format: {}", e))?;

        let mut limits = image::io::Limits::default();
        // Prevent OOM from maliciously crafted or ultra-high-res local album art.
        // Caps to 4K resolution (~67MB uncompressed RGB)
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(1024 * 1024 * 128); // 128 MB maximum buffer allocation
        reader.limits(limits);

        reader
            .decode()
            .map_err(|e| anyhow::anyhow!("Failed to decode image: {}", e))
    }

    async fn fetch_album_art(
        url_str: &str,
        client: &reqwest::Client,
    ) -> Result<image::DynamicImage> {
        info!("Attempting to fetch album art from: {}", url_str);
        if url_str.starts_with("http") {
            let bytes = client
                .get(url_str)
                .send()
                .await
                .map_err(|e| {
                    warn!("HTTP request failed for art: {}", e);
                    e
                })?
                .bytes()
                .await?;

            // Optimization: Image decoding is a synchronous, CPU-intensive task.
            // Offloading this to spawn_blocking prevents it from stalling the main async executor.
            return tokio::task::spawn_blocking(move || {
                Self::decode_image_safely(&bytes).map_err(|e| {
                    warn!("Failed to decode HTTP image data: {}", e);
                    e
                })
            })
            .await
            .unwrap_or_else(|e| Err(anyhow::anyhow!("Image decoding task panicked: {}", e)));
        }

        // Use the `url` crate for robust parsing of file:// paths
        if let Ok(url) = Url::parse(url_str) {
            if url.scheme() == "file" {
                if let Ok(path) = url.to_file_path() {
                    if !Self::is_safe_path(&path) {
                        anyhow::bail!(
                            "Security violation: Attempted path traversal via file:// URL: {:?}",
                            path
                        );
                    }
                    info!("Successfully parsed file path: {:?}", path);
                    let bytes = tokio::fs::read(&path).await.map_err(|e| {
                        warn!("Failed to read art file from disk at {:?}: {}", path, e);
                        e
                    })?;
                    return tokio::task::spawn_blocking(move || {
                        Self::decode_image_safely(&bytes).map_err(|e| {
                            warn!("Failed to decode image from disk {:?}: {}", path, e);
                            e
                        })
                    })
                    .await
                    .unwrap_or_else(|e| {
                        Err(anyhow::anyhow!("Image decoding task panicked: {}", e))
                    });
                }
                warn!(
                    "Could not cleanly convert URL to valid file path: {}",
                    url_str
                );
            }
        }

        // Fallback for absolute paths that are not valid file URLs (e.g. /tmp/art.png)
        info!("Attempting raw path fallback read for: {}", url_str);
        let path = std::path::Path::new(url_str);
        if !Self::is_safe_path(path) {
            anyhow::bail!(
                "Security violation: Attempted path traversal or unsafe raw path: {}",
                url_str
            );
        }

        let bytes = tokio::fs::read(path).await.map_err(|e| {
            warn!("Failed to read raw path {}: {}", url_str, e);
            e
        })?;
        let url_str_owned = url_str.to_string();
        tokio::task::spawn_blocking(move || {
            Self::decode_image_safely(&bytes).map_err(|e| {
                warn!(
                    "Failed to decode image from raw path {}: {}",
                    url_str_owned, e
                );
                e
            })
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!("Image decoding task panicked: {}", e)))
    }

    async fn fetch_fallback_album_art(
        artist: &str,
        album: &str,
        title: &str,
        client: &reqwest::Client,
    ) -> Result<image::DynamicImage> {
        let search_str = if album.is_empty() || album == "Unknown" {
            format!("{} {}", artist, title)
        } else {
            format!("{} {}", artist, album)
        };

        let resp: ITunesResponse = client
            .get("https://itunes.apple.com/search")
            .query(&[
                ("term", search_str.as_str()),
                ("entity", "song"),
                ("limit", "1"),
            ])
            .send()
            .await?
            .json()
            .await?;

        if let Some(first) = resp.results.first() {
            if let Some(art_url) = &first.artwork_url {
                let high_res_url = art_url.replace("100x100bb", "600x600bb");
                let bytes = client.get(&high_res_url).send().await?.bytes().await?;
                return tokio::task::spawn_blocking(move || Self::decode_image_safely(&bytes))
                    .await
                    .unwrap_or_else(|e| {
                        Err(anyhow::anyhow!("Image decoding task panicked: {}", e))
                    });
            }
        }
        anyhow::bail!("No fallback art found on iTunes")
    }

    fn is_safe_path(path: &std::path::Path) -> bool {
        let canonical_path = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => return false,
        };

        // Ensure path is absolute and does not contain any '..' components
        if !canonical_path.is_absolute()
            || canonical_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return false;
        }

        // Restrict to common album art locations for desktop media players:
        // 1. /tmp/ (used by some players for temporary art)
        // 2. /run/user/ (used by some players for art storage)
        // 3. User's HOME directory
        let safe_prefixes = [
            std::path::Path::new("/tmp"),
            std::path::Path::new("/run/user"),
        ];

        if safe_prefixes.iter().any(|p| {
            std::fs::canonicalize(p)
                .map(|cp| canonical_path.starts_with(cp))
                .unwrap_or(false)
        }) {
            return true;
        }

        if let Ok(home) = std::env::var("HOME") {
            let home_path = std::path::Path::new(&home);
            let music_path = home_path.join("Music");
            let cache_path = home_path.join(".cache");
            if [music_path, cache_path].iter().any(|p| {
                std::fs::canonicalize(p)
                    .map(|cp| canonical_path.starts_with(cp))
                    .unwrap_or(false)
            }) {
                return true;
            }
        }

        false
    }

    async fn fetch_spotify_canvas(track_id: &str, client: &reqwest::Client) -> Option<String> {
        // Note: The official Spotify Web API does NOT expose Canvas URLs.
        // To get them, the community routes requests through API proxies that
        // handle the internal gRPC/Protobuf token auth (e.g. 'spotify-canvas-api').
        // Replace this URL with your local instance or a trusted public proxy API!
        let proxy_url = "http://localhost:3000/api/canvas";

        if let Ok(resp) = client
            .get(proxy_url)
            .query(&[("track_id", track_id)])
            .send()
            .await
        {
            if let Ok(canvas) = resp.json::<CanvasResponse>().await {
                return canvas.url;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
