use anyhow::Result;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};
use url::Url;

use super::{
    colour::extract_palette,
    event::{Event, LyricLine, TrackInfo},
    lrclib,
    utils::is_safe_ip,
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

    /// Key for the palette cache: art is the palette's source, so the art URL
    /// identifies it. Falls back to artist/album when no URL is present.
    fn palette_cache_key(&self) -> String {
        self.art_url
            .clone()
            .unwrap_or_else(|| format!("fallback:{}:{}", self.artist, self.album))
    }

    fn lyrics_cache_key(&self) -> String {
        format!("{}:{}:{}", self.artist, self.album, self.title)
    }

    /// Extracts the bare Spotify track ID (used for canvas lookup/caching)
    /// from the various shapes players report `mpris:trackid` in.
    fn spotify_track_id(&self) -> Option<&str> {
        if self.track_id.contains("spotify:track:") {
            self.track_id.split(':').next_back()
        } else if self.track_id.contains("open.spotify.com/track/")
            || self.track_id.contains("/com/spotify/track/")
        {
            self.track_id.split('/').next_back()
        } else {
            None
        }
    }

    /// Identity used for change detection: the player-reported track ID, or
    /// title+artist for players that don't set a usable one.
    fn effective_track_id(&self) -> String {
        if self.track_id.is_empty() {
            format!("{}-{}", self.title, self.artist)
        } else {
            self.track_id.clone()
        }
    }

    /// True when the art URL points at local disk (file:// or a raw path),
    /// meaning it can be loaded in the fast stage without touching the network.
    fn has_local_art(&self) -> bool {
        self.art_url.as_deref().is_some_and(|u| !is_http_url(u))
    }
}

/// URL schemes are case-insensitive (RFC 3986), and some players report
/// uppercase ones - a plain `starts_with("http")` would misroute those to the
/// local-file path.
fn is_http_url(url: &str) -> bool {
    url.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("http"))
}

enum MprisUpdate {
    Metadata(MetadataUpdate),
    Status(mpris::PlaybackStatus),
    Position(std::time::Duration),
    /// Result of a background network fetch (remote art, lyrics, canvas video)
    /// for a track whose fast, local-only info was already sent to the renderer.
    Assets(Box<FetchedAssets>),
}

/// Which slow (network-bound) assets still need fetching after the fast,
/// local-only stage of a track change, plus context the fetch task needs.
struct RemoteNeeds {
    art: bool,
    /// Palette already known from cache for this track's art key. When set, the
    /// art fetch reuses it instead of re-extracting (and it must not be re-cached).
    cached_palette: Option<Box<[[f32; 3]]>>,
    lyrics: bool,
    video: bool,
}

impl RemoteNeeds {
    fn any(&self) -> bool {
        self.art || self.lyrics || self.video
    }
}

struct FetchedAssets {
    meta: MetadataUpdate,
    /// Fetched art plus the palette derived from (or cached for) it.
    art: Option<(image::RgbaImage, Box<[[f32; 3]]>)>,
    /// True when `art`'s palette came from the cache rather than fresh extraction.
    palette_was_cached: bool,
    /// Outer `None` = lyrics were not fetched; inner `None` = fetched, none found.
    /// The distinction matters because "none found" is itself worth caching.
    lyrics: Option<Option<Box<[LyricLine]>>>,
    /// Same outer/inner semantics as `lyrics`.
    video_url: Option<Option<String>>,
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
        config_rx: tokio::sync::watch::Receiver<crate::modules::config::Config>,
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

        // Kept for the background asset-fetch tasks spawned by the main loop;
        // `update_tx` itself moves into the player-selection thread below.
        let assets_update_tx = update_tx.clone();

        let poll_visible = visible_rx.clone();
        std::thread::spawn(move || {
            let mut finder = mpris::PlayerFinder::new();
            let mut last_status = mpris::PlaybackStatus::Stopped;
            let mut last_track_id = String::new();
            let mut watched_bus: Option<String> = None;

            // Bumped every time the watched player changes (or disappears), so a
            // previously-spawned `run_event_watcher` thread - which may still be blocked
            // waiting on a D-Bus signal for the player it was watching - knows to stop
            // forwarding updates instead of racing the new one.
            let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));

            loop {
                // This interval is now only a coarse "did a different player become the
                // best candidate" fallback check - actual track/status changes on the
                // currently watched player are pushed instantly by its dedicated event
                // thread below, via D-Bus PropertiesChanged signals instead of polling.
                if shutdown_rx.recv_timeout(std::time::Duration::from_millis(1000))
                    != Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                {
                    generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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

                // Prioritize any player that is currently playing, but stick with
                // the currently watched one while it is still playing: apps that
                // register multiple MPRIS interfaces for the same playback (e.g.
                // Electron players exposing both their own name and a chromium
                // instance) would otherwise steal the watch from each other on
                // every transient status flicker, re-announcing the same track
                // and invalidating in-flight asset fetches.
                let active_player = f
                    .find_all()
                    .ok()
                    .and_then(|players| {
                        let mut playing: Vec<mpris::Player> = players
                            .into_iter()
                            .filter(|p| {
                                p.get_playback_status()
                                    .unwrap_or(mpris::PlaybackStatus::Stopped)
                                    == mpris::PlaybackStatus::Playing
                            })
                            .collect();
                        if let Some(bus) = &watched_bus {
                            if let Some(idx) = playing.iter().position(|p| p.bus_name() == bus) {
                                return Some(playing.swap_remove(idx));
                            }
                        }
                        playing.into_iter().next()
                    })
                    .or_else(|| f.find_active().ok());

                if let Some(player) = active_player {
                    let current_status = player
                        .get_playback_status()
                        .unwrap_or(mpris::PlaybackStatus::Stopped);

                    if watched_bus.as_deref() != Some(player.bus_name()) {
                        // Selection changed to a different player: snapshot its current
                        // metadata/status immediately (the event watcher only reports
                        // *future* changes relative to whatever state it sees at
                        // subscribe time), then hand it off to a dedicated thread that
                        // blocks on D-Bus signals for this player specifically.
                        if let Some(metadata) = player.get_metadata().ok().as_ref() {
                            let meta = MetadataUpdate::from_metadata(metadata);
                            last_track_id = meta.effective_track_id();
                            let _ = update_tx.blocking_send(MprisUpdate::Metadata(meta));
                        }
                        let _ = update_tx.blocking_send(MprisUpdate::Status(current_status));
                        last_status = current_status;

                        watched_bus = Some(player.bus_name().to_string());
                        let my_generation =
                            generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                        let bus_name = player.bus_name().to_string();
                        let gen_handle = generation.clone();
                        let event_tx = update_tx.clone();
                        std::thread::spawn(move || {
                            run_event_watcher(bus_name, gen_handle, my_generation, event_tx);
                        });
                    } else {
                        // Freshness fallback for the watched player: its event thread
                        // delivers changes instantly *when the player emits D-Bus
                        // signals*, but some players never do (and a watcher thread
                        // can die), which would otherwise leave us stuck on stale
                        // data forever. Duplicates with the event thread are fine -
                        // the main loop dedups both metadata (by content equality)
                        // and status (by playing-state comparison).
                        if let Some(metadata) = player.get_metadata().ok().as_ref() {
                            let meta = MetadataUpdate::from_metadata(metadata);
                            let track_id = meta.effective_track_id();
                            if track_id != last_track_id {
                                last_track_id = track_id;
                                let _ = update_tx.blocking_send(MprisUpdate::Metadata(meta));
                            }
                        }
                        if current_status != last_status {
                            last_status = current_status;
                            let _ = update_tx.blocking_send(MprisUpdate::Status(current_status));
                        }
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
                } else if last_status != mpris::PlaybackStatus::Stopped || watched_bus.is_some() {
                    let _ = update_tx
                        .blocking_send(MprisUpdate::Status(mpris::PlaybackStatus::Stopped));
                    last_status = mpris::PlaybackStatus::Stopped;
                    watched_bus = None;
                    generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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

        // Optimization: Use `rustc_hash::FxBuildHasher` instead of the default SipHash for the LRU caches.
        // FxHash provides a measurable speedup for string keys in internal caching where cryptographic collision
        // resistance is unnecessary, reducing overhead during rapid track changes or metadata updates.
        let mut palette_cache: lru::LruCache<String, Box<[[f32; 3]]>, rustc_hash::FxBuildHasher> =
            lru::LruCache::with_hasher(cache_cap, rustc_hash::FxBuildHasher);
        let mut lyrics_cache: lru::LruCache<
            String,
            Option<Box<[LyricLine]>>,
            rustc_hash::FxBuildHasher,
        > = lru::LruCache::with_hasher(cache_cap, rustc_hash::FxBuildHasher);
        let mut video_cache: lru::LruCache<String, Option<String>, rustc_hash::FxBuildHasher> =
            lru::LruCache::with_hasher(cache_cap, rustc_hash::FxBuildHasher);

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
                match update {
                    MprisUpdate::Metadata(meta) => {
                        // A player is evidently active, so we are no longer timed out.
                        is_timed_out = false;
                        let is_empty = (meta.title == "Unknown" || meta.title.trim().is_empty())
                            && meta.artist.trim().is_empty();
                        if !is_empty {
                            last_metadata = Some(meta);
                        }
                    }
                    MprisUpdate::Status(status) => {
                        is_timed_out = false;
                        let playing = status == mpris::PlaybackStatus::Playing;
                        if playing != is_playing {
                            is_playing = playing;
                            if is_playing {
                                info!("Playback resumed");
                                paused_since = None;
                                let _ = tx.send(Event::PlaybackResumed).await;
                            } else {
                                info!("Playback paused/stopped");
                                paused_since = Some(tokio::time::Instant::now());
                                let _ = tx.send(Event::PlaybackStopped).await;
                            }
                        } else if !playing && paused_since.is_none() {
                            // If we start up and a player is already paused, start the timer
                            paused_since = Some(tokio::time::Instant::now());
                        }
                    }
                    MprisUpdate::Position(pos) => {
                        is_timed_out = false;
                        // Drop continuous position updates if we aren't rendering to save channel capacity
                        if visible {
                            let _ = tx.send(Event::PlaybackPosition(pos)).await;
                        }
                    }
                    // Deliberately does NOT reset is_timed_out: this is the result of
                    // our own background fetch, not evidence of player activity.
                    MprisUpdate::Assets(assets) => {
                        // Cache what was fetched even if it arrives stale - the same
                        // track will likely come around again.
                        if let Some((_, palette)) = &assets.art {
                            if !assets.palette_was_cached {
                                palette_cache.put(assets.meta.palette_cache_key(), palette.clone());
                            }
                        }
                        if let Some(lyrics) = &assets.lyrics {
                            lyrics_cache.put(assets.meta.lyrics_cache_key(), lyrics.clone());
                        }
                        if let Some(video_url) = &assets.video_url {
                            if let Some(id) = assets.meta.spotify_track_id() {
                                if !video_cache.contains(id) {
                                    video_cache.put(id.to_string(), video_url.clone());
                                }
                            }
                        }

                        // Only forward to the renderer if these assets still belong to
                        // the currently displayed track.
                        if !is_timed_out && last_processed_metadata.as_ref() == Some(&assets.meta) {
                            let assets = *assets;
                            let (album_art, palette) = match assets.art {
                                Some((img, palette)) => (Some(img), Some(palette)),
                                None => (None, None),
                            };
                            let lyrics = assets.lyrics.flatten();
                            if lyrics.is_some() {
                                info!(
                                    "Synced lyrics loaded for {} - {}",
                                    assets.meta.artist, assets.meta.title
                                );
                            }
                            let video_url = assets.video_url.flatten();
                            if let Some(url) = &video_url {
                                spawn_canvas_decoder(&tx, &mut video_cancel_tx, url);
                            }
                            let _ = tx
                                .send(Event::TrackAssetsLoaded(Box::new(TrackInfo {
                                    title: assets.meta.title.into_boxed_str(),
                                    artist: assets.meta.artist.into_boxed_str(),
                                    album: assets.meta.album.into_boxed_str(),
                                    album_art,
                                    palette,
                                    lyrics,
                                    video_url: video_url.map(|u| u.into_boxed_str()),
                                })))
                                .await;
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

            // If we are visible and have unprocessed metadata, announce the track
            // right away using only fast, local data (disk art, caches), and hand
            // anything network-bound to a background task. Blocking this loop on
            // network fetches previously stalled the visible track change - and all
            // other MPRIS updates - for up to the full HTTP timeout.
            if visible && !is_timed_out && last_metadata != last_processed_metadata {
                if let Some(meta) = last_metadata.clone() {
                    info!("Track changed: {} - {}", meta.artist, meta.title);
                    let (track_info, needs) = Self::build_fast_track_info(
                        &meta,
                        &mut palette_cache,
                        &mut lyrics_cache,
                        &mut video_cache,
                        current_show_lyrics,
                    )
                    .await;

                    // Kill the previous track's FFmpeg canvas decoder, if any, and
                    // start a new one when the canvas URL was already cached.
                    if let Some(cancel) = video_cancel_tx.take() {
                        let _ = cancel.send(true);
                    }
                    if let Some(url) = track_info.video_url.as_deref() {
                        spawn_canvas_decoder(&tx, &mut video_cancel_tx, url);
                    }

                    let _ = tx.send(Event::TrackChanged(Box::new(track_info))).await;
                    last_processed_metadata = Some(meta.clone());

                    if needs.any() {
                        let client = http_client.clone();
                        let assets_tx = assets_update_tx.clone();
                        // Snapshot the proxy URL at fetch time so a config
                        // reload takes effect on the next track change.
                        let canvas_proxy = config_rx.borrow().audio.canvas_proxy_url.clone();
                        tokio::spawn(async move {
                            let assets = Self::fetch_remote_assets(
                                meta,
                                needs,
                                canvas_proxy.as_deref(),
                                &client,
                            )
                            .await;
                            let _ = assets_tx.send(MprisUpdate::Assets(Box::new(assets))).await;
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Fast, local-only stage of a track change: album art loaded from disk
    /// (when the player provided a file path), palette/lyrics/canvas served
    /// from cache. Returns what still needs a network round-trip so the
    /// caller can fetch it in the background without holding up the display.
    async fn build_fast_track_info(
        meta: &MetadataUpdate,
        palette_cache: &mut lru::LruCache<String, Box<[[f32; 3]]>, rustc_hash::FxBuildHasher>,
        lyrics_cache: &mut lru::LruCache<
            String,
            Option<Box<[LyricLine]>>,
            rustc_hash::FxBuildHasher,
        >,
        video_cache: &mut lru::LruCache<String, Option<String>, rustc_hash::FxBuildHasher>,
        fetch_lyrics: bool,
    ) -> (TrackInfo, RemoteNeeds) {
        let cached_palette = palette_cache.get(&meta.palette_cache_key()).cloned();

        let (lyrics, needs_lyrics) = if !fetch_lyrics {
            (None, false)
        } else if let Some(cached) = lyrics_cache.get(&meta.lyrics_cache_key()) {
            (cached.clone(), false)
        } else {
            (None, true)
        };

        let (video_url, needs_video) = match meta.spotify_track_id() {
            None => (None, false),
            Some(id) => match video_cache.get(id) {
                Some(cached) => (cached.clone(), false),
                None => (None, true),
            },
        };

        let (album_art, palette, needs_art) = if meta.has_local_art() {
            match Self::fetch_album_art(meta.art_url.as_deref().unwrap_or_default()).await {
                Ok(img) => {
                    info!("Successfully loaded and decoded local album art!");
                    let (art, palette) = Self::process_art(img, cached_palette.clone()).await;
                    if cached_palette.is_none() {
                        if let Some(p) = &palette {
                            palette_cache.put(meta.palette_cache_key(), p.clone());
                        }
                    }
                    (art, palette, false)
                }
                Err(e) => {
                    // Local read failed (likely Flatpak isolation): defer to the
                    // network fallback in the background fetch.
                    warn!("Failed to load local album art: {}", e);
                    (None, cached_palette.clone(), true)
                }
            }
        } else {
            // Remote art URL (or none at all, meaning the iTunes fallback):
            // network-bound, so fetched in the background. A cached palette can
            // still colour the scene immediately.
            (None, cached_palette.clone(), true)
        };

        let track_info = TrackInfo {
            title: meta.title.clone().into_boxed_str(),
            artist: meta.artist.clone().into_boxed_str(),
            album: meta.album.clone().into_boxed_str(),
            album_art,
            palette,
            lyrics,
            video_url: video_url.map(|u| u.into_boxed_str()),
        };

        (
            track_info,
            RemoteNeeds {
                art: needs_art,
                cached_palette,
                lyrics: needs_lyrics,
                video: needs_video,
            },
        )
    }

    /// Slow, network-bound stage of a track change. Runs as a detached task so
    /// the main loop stays responsive; the result is routed back through the
    /// update channel, which caches it and drops it if it arrived stale.
    async fn fetch_remote_assets(
        meta: MetadataUpdate,
        needs: RemoteNeeds,
        canvas_proxy_url: Option<&str>,
        client: &reqwest::Client,
    ) -> FetchedAssets {
        let art_future = async {
            if !needs.art {
                return None;
            }

            let mut img = None;
            if let Some(url) = meta.art_url.as_deref() {
                if is_http_url(url) {
                    img = Self::fetch_album_art(url).await.ok();
                }
            }

            let img = match img {
                Some(img) => Some(img),
                None => {
                    info!("Attempting to fetch fallback album art online...");
                    match Self::fetch_fallback_album_art(
                        &meta.artist,
                        &meta.album,
                        &meta.title,
                        client,
                    )
                    .await
                    {
                        Ok(img) => Some(img),
                        Err(e) => {
                            warn!(
                                "Fallback art failed: {}. Generating dynamic placeholder.",
                                e
                            );
                            Self::generate_placeholder_art().await
                        }
                    }
                }
            };

            match img {
                Some(img) => {
                    let (art, palette) = Self::process_art(img, needs.cached_palette.clone()).await;
                    art.zip(palette)
                }
                None => None,
            }
        };

        let lyrics_future = async {
            if needs.lyrics {
                Some(
                    lrclib::fetch_synced_lyrics(&meta.title, &meta.artist, &meta.album, client)
                        .await,
                )
            } else {
                None
            }
        };

        let video_future = async {
            if !needs.video {
                return None;
            }
            match meta.spotify_track_id() {
                Some(id) => Some(Self::fetch_spotify_canvas(id, canvas_proxy_url, client).await),
                None => None,
            }
        };

        let (art, lyrics, video_url) = tokio::join!(art_future, lyrics_future, video_future);

        FetchedAssets {
            palette_was_cached: needs.cached_palette.is_some(),
            meta,
            art,
            lyrics,
            video_url,
        }
    }

    /// Resizes oversized art and derives its colour palette (reusing a cached
    /// palette when supplied), off the async executor.
    async fn process_art(
        img: image::DynamicImage,
        cached_palette: Option<Box<[[f32; 3]]>>,
    ) -> (Option<image::RgbaImage>, Option<Box<[[f32; 3]]>>) {
        // Optimization: Offload heavy CPU-bound palette extraction and image conversion
        // to a dedicated blocking thread. This saves ~50-100ms of executor stall time.
        tokio::task::spawn_blocking(move || {
            let palette = cached_palette.unwrap_or_else(|| extract_palette(&img));

            // Optimisation: Limit album art size to 1024x1024 to prevent massive RAM usage.
            let resized_img = if img.width() > 1024 || img.height() > 1024 {
                img.thumbnail(1024, 1024)
            } else {
                img
            };

            (Some(resized_img.into_rgba8()), Some(palette))
        })
        .await
        .unwrap_or((None, None))
    }

    /// Dynamic gradient stand-in for when no art can be found anywhere.
    async fn generate_placeholder_art() -> Option<image::DynamicImage> {
        tokio::task::spawn_blocking(|| {
            let mut img = image::RgbaImage::new(640, 640);
            for y in 0..640 {
                for x in 0..640 {
                    let r = ((x as f32 / 640.0) * 80.0) as u8 + 20;
                    let b = ((y as f32 / 640.0) * 80.0) as u8 + 40;
                    img.put_pixel(x, y, image::Rgba([r, 20, b, 255]));
                }
            }
            image::DynamicImage::ImageRgba8(img)
        })
        .await
        .ok()
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

    async fn fetch_album_art(url_str: &str) -> Result<image::DynamicImage> {
        info!("Attempting to fetch album art from: {}", url_str);
        if is_http_url(url_str) {
            let parsed_url = Url::parse(url_str)?;
            let host_str = parsed_url
                .host_str()
                .ok_or_else(|| anyhow::anyhow!("No host in URL"))?;
            let port = parsed_url.port_or_known_default().unwrap_or(80);

            let mut safe_addr = None;
            let host_port = format!("{}:{}", host_str, port);
            if let Ok(mut addrs) = tokio::net::lookup_host(&host_port).await {
                for addr in addrs.by_ref() {
                    if is_safe_ip(addr.ip()) {
                        safe_addr = Some(addr);
                        break;
                    }
                }
            }

            let safe_addr =
                safe_addr.ok_or_else(|| anyhow::anyhow!("No safe IP found (SSRF protection)"))?;

            let safe_client = reqwest::Client::builder()
                .user_agent("cosmic-wallpaper/1.0")
                .timeout(std::time::Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .resolve(host_str, safe_addr)
                .build()?;

            let mut response = safe_client.get(url_str).send().await.map_err(|e| {
                warn!("HTTP request failed for art: {}", e);
                e
            })?;

            let mut bytes = Vec::new();
            const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024; // 10 MB limit
            while let Some(chunk) = response.chunk().await? {
                if bytes.len() + chunk.len() > MAX_IMAGE_SIZE {
                    return Err(anyhow::anyhow!(
                        "Image size exceeds 10MB limit (OOM protection)"
                    ));
                }
                bytes.extend_from_slice(&chunk);
            }

            // Optimization: Image decoding is a synchronous, CPU-intensive task.
            // Offloading this to spawn_blocking prevents it from stalling the main async executor.
            return tokio::task::spawn_blocking(move || {
                Self::decode_image_safely(&bytes).map_err(|e| {
                    warn!("Failed to decode HTTP image data: {}", e);
                    e
                })
            })
            .await
            .map_err(|e| anyhow::anyhow!("Image decoding task panicked: {}", e))?;
        }

        // Use the `url` crate for robust parsing of file:// paths
        if let Ok(url) = Url::parse(url_str) {
            if url.scheme() == "file" {
                if let Ok(path) = url.to_file_path() {
                    let real_path = match Self::resolve_safe_path(&path) {
                        Some(p) => p,
                        None => anyhow::bail!(
                            "Security violation: Attempted path traversal via file:// URL: {:?}",
                            path
                        ),
                    };
                    info!("Successfully parsed file path: {:?}", real_path);
                    let bytes = tokio::fs::read(&real_path).await.map_err(|e| {
                        warn!(
                            "Failed to read art file from disk at {:?}: {}",
                            real_path, e
                        );
                        e
                    })?;
                    return tokio::task::spawn_blocking(move || {
                        Self::decode_image_safely(&bytes).map_err(|e| {
                            warn!("Failed to decode image from disk {:?}: {}", real_path, e);
                            e
                        })
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!("Image decoding task panicked: {}", e))?;
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
        let real_path = match Self::resolve_safe_path(path) {
            Some(p) => p,
            None => anyhow::bail!(
                "Security violation: Attempted path traversal or unsafe raw path: {}",
                url_str
            ),
        };

        let bytes = tokio::fs::read(&real_path).await.map_err(|e| {
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
        .map_err(|e| anyhow::anyhow!("Image decoding task panicked: {}", e))?
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

        let mut response = client
            .get("https://itunes.apple.com/search")
            .query(&[
                ("term", search_str.as_str()),
                ("entity", "song"),
                ("limit", "1"),
            ])
            .send()
            .await?;

        let mut bytes = Vec::new();
        const MAX_JSON_SIZE: usize = 10 * 1024 * 1024; // 10 MB limit
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > MAX_JSON_SIZE {
                return Err(anyhow::anyhow!("JSON payload exceeds limit"));
            }
            bytes.extend_from_slice(&chunk);
        }
        let resp: ITunesResponse = serde_json::from_slice(&bytes)?;

        if let Some(first) = resp.results.first() {
            if let Some(art_url) = &first.artwork_url {
                let high_res_url = art_url.replace("100x100bb", "600x600bb");
                return Self::fetch_album_art(&high_res_url).await;
            }
        }
        anyhow::bail!("No fallback art found on iTunes")
    }

    fn resolve_safe_path(path: &std::path::Path) -> Option<std::path::PathBuf> {
        // Ensure path is absolute and does not contain any '..' components
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return None;
        }

        // Canonicalize to resolve symlinks and prevent bypasses.
        // If the path does not exist, we reject it.
        let real_path = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => return None,
        };

        // Restrict to common album art locations for desktop media players:
        // 1. /tmp/ (used by some players for temporary art)
        // 2. /run/user/ (used by some players for art storage)
        // 3. ~/Music, ~/.cache and ~/.local/share (players' media libraries
        //    and art caches) - deliberately NOT all of HOME, so untrusted
        //    MPRIS metadata can't point us at ~/.ssh and friends.
        let safe_prefixes = [
            std::path::Path::new("/tmp"),
            std::path::Path::new("/run/user"),
        ];

        if safe_prefixes
            .iter()
            .any(|p| std::fs::canonicalize(p).is_ok_and(|real_p| real_path.starts_with(real_p)))
        {
            return Some(real_path);
        }

        if let Ok(home) = std::env::var("HOME") {
            let home_path = std::path::Path::new(&home);

            for subdir in ["Music", ".cache", ".local/share"] {
                if let Ok(real_prefix) = std::fs::canonicalize(home_path.join(subdir)) {
                    if real_path.starts_with(real_prefix) {
                        return Some(real_path);
                    }
                }
            }
        }

        None
    }

    async fn fetch_spotify_canvas(
        track_id: &str,
        proxy_url: Option<&str>,
        client: &reqwest::Client,
    ) -> Option<String> {
        // Note: The official Spotify Web API does NOT expose Canvas URLs.
        // To get them, the community routes requests through API proxies that
        // handle the internal gRPC/Protobuf token auth (e.g. 'spotify-canvas-api').
        // The proxy is configured via `audio.canvas_proxy_url` and canvas is
        // disabled when unset: defaulting to a well-known localhost port would
        // let any local process impersonate the proxy and feed the engine
        // attacker-controlled video URLs.
        let proxy_url = proxy_url?;

        if let Ok(mut resp) = client
            .get(proxy_url)
            .query(&[("track_id", track_id)])
            .send()
            .await
        {
            let mut bytes = Vec::new();
            const MAX_JSON_SIZE: usize = 10 * 1024 * 1024; // 10 MB limit
            while let Ok(Some(chunk)) = resp.chunk().await {
                if bytes.len() + chunk.len() > MAX_JSON_SIZE {
                    return None;
                }
                bytes.extend_from_slice(&chunk);
            }
            if let Ok(canvas) = serde_json::from_slice::<CanvasResponse>(&bytes) {
                return canvas.url;
            }
        }
        None
    }
}

/// Cancels any running canvas decoder and spins up a new FFmpeg decoder
/// pipeline for `url`, streaming frames to the renderer via `tx`.
fn spawn_canvas_decoder(
    tx: &Sender<Event>,
    video_cancel_tx: &mut Option<tokio::sync::watch::Sender<bool>>,
    url: &str,
) {
    if let Some(cancel) = video_cancel_tx.take() {
        let _ = cancel.send(true);
    }
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    let (recycle_tx, recycle_rx) = tokio::sync::mpsc::channel(3);
    *video_cancel_tx = Some(cancel_tx);
    let tx = tx.clone();
    let url = url.to_string();
    tokio::spawn(async move {
        let _ = super::video::VideoDecoder::run_decoder(url, tx, cancel_rx, recycle_rx, recycle_tx)
            .await;
    });
}

/// Blocks on D-Bus signals for one specific player, forwarding translated updates as
/// they arrive instead of polling for them. `mpris`'s types (`Player`, `PlayerFinder`)
/// hold an `Rc` and aren't `Send`, so this runs on its own dedicated OS thread and
/// D-Bus connection rather than being handed a `Player` from the selection thread
/// above. It exits as soon as `generation` no longer matches `my_generation` - meaning
/// the selection thread picked a different player - or the watched player quits.
fn run_event_watcher(
    bus_name: String,
    generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
    my_generation: u64,
    update_tx: tokio::sync::mpsc::Sender<MprisUpdate>,
) {
    let is_current = || generation.load(std::sync::atomic::Ordering::SeqCst) == my_generation;

    let Ok(finder) = mpris::PlayerFinder::new() else {
        return;
    };
    let Ok(players) = finder.iter_players() else {
        return;
    };

    let mut target_player = None;
    for player in players.flatten() {
        if player.bus_name() == bus_name {
            target_player = Some(player);
            break;
        }
    }
    let Some(player) = target_player else {
        return;
    };

    let Ok(events) = player.events() else {
        return;
    };

    for event in events {
        // The selection thread moved on to a different player (or none at all)
        // since this watcher was spawned - stop forwarding stale updates.
        if !is_current() {
            return;
        }

        let Ok(event) = event else { continue };

        match event {
            mpris::Event::TrackChanged(metadata) => {
                let _ = update_tx.blocking_send(MprisUpdate::Metadata(
                    MetadataUpdate::from_metadata(&metadata),
                ));
            }
            mpris::Event::Playing => {
                let _ =
                    update_tx.blocking_send(MprisUpdate::Status(mpris::PlaybackStatus::Playing));
            }
            mpris::Event::Paused => {
                let _ = update_tx.blocking_send(MprisUpdate::Status(mpris::PlaybackStatus::Paused));
            }
            // Stopped is a playback state, not a player exit - some players emit
            // it transiently between tracks - so keep watching for what follows.
            mpris::Event::Stopped => {
                let _ =
                    update_tx.blocking_send(MprisUpdate::Status(mpris::PlaybackStatus::Stopped));
            }
            mpris::Event::PlayerShutDown => {
                let _ =
                    update_tx.blocking_send(MprisUpdate::Status(mpris::PlaybackStatus::Stopped));
                // The player left the bus; the selection thread will pick a
                // successor (or report silence) on its next scan.
                return;
            }
            // Instant seek correction instead of waiting on the next drift-correction tick.
            mpris::Event::Seeked { position_in_us } => {
                let _ = update_tx.try_send(MprisUpdate::Position(
                    std::time::Duration::from_micros(position_in_us),
                ));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
