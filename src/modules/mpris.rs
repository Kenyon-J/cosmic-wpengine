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
    ShutDown,
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
        is_visible: std::sync::Arc<std::sync::atomic::AtomicBool>,
        show_lyrics: std::sync::Arc<std::sync::atomic::AtomicBool>,
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
        .unwrap()?;
        let (update_tx, mut update_rx) = tokio::sync::mpsc::channel(16);

        // Background position polling to handle media players that fail to send Seeked signals
        let poll_tx = tx.clone();
        let poll_visible = is_visible.clone();
        std::thread::spawn(move || {
            let mut finder = mpris::PlayerFinder::new();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                // Suspend D-Bus heavy polling when wallpaper is out of view
                if !poll_visible.load(std::sync::atomic::Ordering::Relaxed) {
                    continue;
                }

                let f = match finder.as_ref() {
                    Ok(f) => f,
                    Err(_) => {
                        finder = mpris::PlayerFinder::new();
                        continue;
                    }
                };

                if let Ok(player) = f.find_active() {
                    if let Ok(mpris::PlaybackStatus::Playing) = player.get_playback_status() {
                        if let Ok(pos) = player.get_position() {
                            let _ = poll_tx.blocking_send(Event::PlaybackPosition(pos));
                        }
                    }
                }
            }
        });

        std::thread::spawn(move || {
            let mut finder = mpris::PlayerFinder::new();

            loop {
                let f = match finder.as_ref() {
                    Ok(f) => f,
                    Err(_) => {
                        finder = mpris::PlayerFinder::new();
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        continue;
                    }
                };

                let player = match f.find_active() {
                    Ok(p) => p,
                    Err(_) => {
                        let _ = update_tx
                            .blocking_send(MprisUpdate::Status(mpris::PlaybackStatus::Stopped));
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        continue;
                    }
                };

                // Prioritize any player that is currently playing over the "most recently active"
                let active_player = f.find_all().ok().and_then(|players| {
                    players.into_iter().find(|p| {
                        p.get_playback_status()
                            .unwrap_or(mpris::PlaybackStatus::Stopped)
                            == mpris::PlaybackStatus::Playing
                    })
                });

                let player = match active_player {
                    Some(p) => p,
                    None => player, // Fallback to the most recently active player if none are playing
                };

                if let Ok(metadata) = player.get_metadata() {
                    let _ = update_tx.blocking_send(MprisUpdate::Metadata(
                        MetadataUpdate::from_metadata(&metadata),
                    ));
                }
                if let Ok(status) = player.get_playback_status() {
                    let _ = update_tx.blocking_send(MprisUpdate::Status(status));
                }
                if let Ok(pos) = player.get_position() {
                    let _ = update_tx.blocking_send(MprisUpdate::Position(pos));
                }

                if let Ok(events) = player.events() {
                    for event in events {
                        match event {
                            Ok(mpris::Event::TrackChanged(metadata)) => {
                                let _ = update_tx.blocking_send(MprisUpdate::Metadata(
                                    MetadataUpdate::from_metadata(&metadata),
                                ));
                                if let Ok(pos) = player.get_position() {
                                    let _ = update_tx.blocking_send(MprisUpdate::Position(pos));
                                }
                            }
                            Ok(mpris::Event::Playing) => {
                                let _ = update_tx.blocking_send(MprisUpdate::Status(
                                    mpris::PlaybackStatus::Playing,
                                ));
                                if let Ok(pos) = player.get_position() {
                                    let _ = update_tx.blocking_send(MprisUpdate::Position(pos));
                                }
                            }
                            Ok(mpris::Event::Paused) => {
                                let _ = update_tx.blocking_send(MprisUpdate::Status(
                                    mpris::PlaybackStatus::Paused,
                                ));
                                if let Ok(pos) = player.get_position() {
                                    let _ = update_tx.blocking_send(MprisUpdate::Position(pos));
                                }
                            }
                            Ok(mpris::Event::Stopped) => {
                                let _ = update_tx.blocking_send(MprisUpdate::Status(
                                    mpris::PlaybackStatus::Stopped,
                                ));
                            }
                            Ok(mpris::Event::Seeked { position_in_us }) => {
                                let pos = std::time::Duration::from_micros(position_in_us);
                                let _ = update_tx.blocking_send(MprisUpdate::Position(pos));
                            }
                            Ok(mpris::Event::PlayerShutDown) => {
                                let _ = update_tx.blocking_send(MprisUpdate::ShutDown);
                                break;
                            }
                            Err(e) => {
                                warn!("MPRIS Event stream error: {}", e);
                                let _ = update_tx.blocking_send(MprisUpdate::ShutDown);
                                break; // Break out of the infinite iterator safely!
                            }
                            _ => {}
                        }
                    }
                } else {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        });

        let mut is_playing = false;
        let mut is_timed_out = false;
        let mut paused_since: Option<tokio::time::Instant> = None;
        let timeout_duration = tokio::time::Duration::from_secs(15);
        let mut last_metadata: Option<MetadataUpdate> = None;
        let mut last_processed_metadata: Option<MetadataUpdate> = None;
        let mut palette_cache: std::collections::HashMap<String, Vec<[f32; 3]>> =
            std::collections::HashMap::new();
        let mut lyrics_cache: std::collections::HashMap<String, Option<Vec<LyricLine>>> =
            std::collections::HashMap::new();
        let mut video_cache: std::collections::HashMap<String, Option<String>> =
            std::collections::HashMap::new();
        let mut video_cancel_tx: Option<tokio::sync::watch::Sender<bool>> = None;

        let mut last_show_lyrics = show_lyrics.load(std::sync::atomic::Ordering::Relaxed);

        loop {
            // Wake up every 250ms to check visibility state even if no MPRIS events fire
            let update_opt = match tokio::time::timeout(
                tokio::time::Duration::from_millis(250),
                update_rx.recv(),
            )
            .await
            {
                Ok(Some(u)) => Some(u),
                Ok(None) => break,
                Err(_) => None,
            };

            let visible = is_visible.load(std::sync::atomic::Ordering::Relaxed);
            let current_show_lyrics = show_lyrics.load(std::sync::atomic::Ordering::Relaxed);

            if current_show_lyrics != last_show_lyrics {
                last_show_lyrics = current_show_lyrics;
                if current_show_lyrics {
                    // Force a rebuild to fetch the lyrics immediately if toggled back on
                    last_processed_metadata = None;
                }
            }

            if let Some(update) = update_opt {
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
                    MprisUpdate::ShutDown => {
                        is_playing = false;
                        is_timed_out = false;
                        paused_since = None;
                        info!("Player shut down");
                        if let Some(cancel) = video_cancel_tx.take() {
                            let _ = cancel.send(true);
                        }
                        let _ = tx.send(Event::PlayerShutDown).await;
                        last_metadata = None;
                        last_processed_metadata = None;
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
                if let Some(meta) = last_metadata.clone() {
                    // Prevent boundless memory growth in caches for a long-running process
                    if palette_cache.len() > 50 {
                        info!("Clearing MPRIS caches to free memory...");
                        palette_cache.clear();
                        lyrics_cache.clear();
                        video_cache.clear();
                    }

                    info!("Fetching track info for: {} - {}", meta.artist, meta.title);
                    let track_info = Self::build_track_info(
                        &meta,
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
                        video_cancel_tx = Some(cancel_tx);
                        let tx_clone = tx.clone();
                        let url_clone = url.clone();
                        tokio::spawn(async move {
                            let _ = super::video::VideoDecoder::run_decoder(
                                url_clone, tx_clone, cancel_rx,
                            )
                            .await;
                        });
                    }

                    let _ = tx.send(Event::TrackChanged(track_info)).await;
                    last_processed_metadata = Some(meta);
                }
            }
        }

        Ok(())
    }

    async fn build_track_info(
        meta: &MetadataUpdate,
        palette_cache: &mut std::collections::HashMap<String, Vec<[f32; 3]>>,
        lyrics_cache: &mut std::collections::HashMap<String, Option<Vec<LyricLine>>>,
        video_cache: &mut std::collections::HashMap<String, Option<String>>,
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
                // Offload the heavy CPU blocking work to a dedicated Tokio worker thread
                tokio::task::spawn_blocking(move || {
                    let palette = cached.unwrap_or_else(|| extract_palette(&img));
                    let rgba = img.into_rgba8();
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

        let video_future = async {
            let mut track_id = None;
            if meta.track_id.contains("spotify:track:") {
                track_id = meta.track_id.split(':').next_back();
            } else if meta.track_id.contains("open.spotify.com/track/")
                || meta.track_id.contains("/com/spotify/track/")
            {
                track_id = meta.track_id.split('/').next_back();
            }

            if let Some(id) = track_id {
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
                palette_cache.insert(cache_key, p.clone());
            }
        }

        if cached_lyrics.is_none() && fetch_lyrics {
            lyrics_cache.insert(lyrics_cache_key, lyrics.clone());
        }

        let raw_id = if meta.track_id.contains("spotify:track:") {
            meta.track_id.split(':').next_back()
        } else if meta.track_id.contains("open.spotify.com/track/")
            || meta.track_id.contains("/com/spotify/track/")
        {
            meta.track_id.split('/').next_back()
        } else {
            None
        };

        if let Some(id) = raw_id {
            if !video_cache.contains_key(id) {
                video_cache.insert(id.to_string(), video_url.clone());
            }
        }

        if lyrics.is_some() {
            info!("Synced lyrics loaded for {} - {}", meta.artist, meta.title);
        }

        TrackInfo {
            title: meta.title.clone(),
            artist: meta.artist.clone(),
            album: meta.album.clone(),
            album_art,
            palette,
            lyrics,
            video_url,
        }
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
            return tokio::task::spawn_blocking(move || {
                image::load_from_memory(&bytes).map_err(|e| {
                    warn!("Failed to decode HTTP image data: {}", e);
                    e.into()
                })
            })
            .await?;
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
                        image::load_from_memory(&bytes).map_err(|e| {
                            warn!("Failed to decode image from disk {:?}: {}", path, e);
                            e.into()
                        })
                    })
                    .await?;
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
            image::load_from_memory(&bytes).map_err(|e| {
                warn!(
                    "Failed to decode image from raw path {}: {}",
                    url_str_owned, e
                );
                e.into()
            })
        })
        .await?
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
                return Ok(
                    tokio::task::spawn_blocking(move || image::load_from_memory(&bytes)).await??,
                );
            }
        }
        anyhow::bail!("No fallback art found on iTunes")
    }

    fn is_safe_path(path: &std::path::Path) -> bool {
        // Ensure path is absolute and does not contain any '..' components
        if !path.is_absolute()
            || path
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

        if safe_prefixes.iter().any(|p| path.starts_with(p)) {
            return true;
        }

        if let Ok(home) = std::env::var("HOME") {
            if path.starts_with(home) {
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
        let proxy_url = format!("http://localhost:3000/api/canvas?track_id={}", track_id);

        if let Ok(resp) = client.get(&proxy_url).send().await {
            if let Ok(canvas) = resp.json::<CanvasResponse>().await {
                return canvas.url;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_is_safe_path() {
        std::env::set_var("HOME", "/home/testuser");

        // Valid absolute paths in safe locations
        assert!(MprisWatcher::is_safe_path(Path::new("/tmp/art.png")));
        assert!(MprisWatcher::is_safe_path(Path::new(
            "/run/user/1000/art.jpg"
        )));
        assert!(MprisWatcher::is_safe_path(Path::new(
            "/home/testuser/Music/cover.png"
        )));

        // Path traversal attempts
        assert!(!MprisWatcher::is_safe_path(Path::new("/tmp/../etc/passwd")));
        assert!(!MprisWatcher::is_safe_path(Path::new(
            "/run/user/../../var/log"
        )));

        // Relative paths
        assert!(!MprisWatcher::is_safe_path(Path::new("art.png")));
        assert!(!MprisWatcher::is_safe_path(Path::new("./art.png")));
    }
}
