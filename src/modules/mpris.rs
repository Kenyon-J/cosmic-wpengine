// =============================================================================
// modules/mpris.rs
// =============================================================================
// Watches for music player changes using the MPRIS D-Bus standard.
//
// MPRIS is supported by: Spotify, VLC, Firefox, Chromium, mpd, Rhythmbox,
// Lollypop, and most other Linux media players. If it plays audio on Linux,
// it probably speaks MPRIS.
//
// This module:
//   1. Finds any active MPRIS player on the D-Bus session
//   2. Watches for track changes and playback state changes
//   3. When a track changes, fetches album art and extracts colours
//   4. Sends TrackChanged or PlaybackStopped events to the renderer
// =============================================================================

use anyhow::Result;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use super::{
    colour::extract_palette,
    event::{Event, TrackInfo},
    lrclib,
};

#[derive(Debug, Clone)]
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
            track_id: metadata.track_id().map(|id| id.to_string()).unwrap_or_default(),
        }
    }
}

enum MprisUpdate {
    Metadata(MetadataUpdate),
    Status(mpris::PlaybackStatus),
    Position(std::time::Duration),
    ShutDown,
}

pub struct MprisWatcher;

impl MprisWatcher {
    pub async fn run(tx: Sender<Event>) -> Result<()> {
        info!("MPRIS watcher started");

        let (update_tx, mut update_rx) = tokio::sync::mpsc::channel(16);

        // Spawn a dedicated thread for the blocking D-Bus event stream.
        // This avoids blocking the async runtime and lets us use D-Bus signals!
        std::thread::spawn(move || {
            let finder = match mpris::PlayerFinder::new() {
                Ok(f) => f,
                Err(_) => return,
            };

            loop {
                let player = match finder.find_active() {
                    Ok(p) => p,
                    Err(_) => {
                        let _ = update_tx.blocking_send(MprisUpdate::Status(mpris::PlaybackStatus::Stopped));
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        continue;
                    }
                };

                info!("Found active player: {}", player.identity());

                // Send initial state immediately
                if let Ok(metadata) = player.get_metadata() {
                    let _ = update_tx.blocking_send(MprisUpdate::Metadata(MetadataUpdate::from_metadata(&metadata)));
                }
                if let Ok(status) = player.get_playback_status() {
                    let _ = update_tx.blocking_send(MprisUpdate::Status(status));
                }
                if let Ok(pos) = player.get_position() {
                    let _ = update_tx.blocking_send(MprisUpdate::Position(pos));
                }

                // Block on D-Bus signals (player.events() is a blocking iterator)
                if let Ok(events) = player.events() {
                    for event in events {
                        match event {
                            Ok(mpris::Event::TrackChanged(metadata)) => {
                                let _ = update_tx.blocking_send(MprisUpdate::Metadata(MetadataUpdate::from_metadata(&metadata)));
                                if let Ok(pos) = player.get_position() {
                                    let _ = update_tx.blocking_send(MprisUpdate::Position(pos));
                                }
                            }
                            Ok(mpris::Event::Playing) => {
                                let _ = update_tx.blocking_send(MprisUpdate::Status(mpris::PlaybackStatus::Playing));
                                if let Ok(pos) = player.get_position() {
                                    let _ = update_tx.blocking_send(MprisUpdate::Position(pos));
                                }
                            }
                            Ok(mpris::Event::Paused) => {
                                let _ = update_tx.blocking_send(MprisUpdate::Status(mpris::PlaybackStatus::Paused));
                                if let Ok(pos) = player.get_position() {
                                    let _ = update_tx.blocking_send(MprisUpdate::Position(pos));
                                }
                            }
                            Ok(mpris::Event::Stopped) => {
                                let _ = update_tx.blocking_send(MprisUpdate::Status(mpris::PlaybackStatus::Stopped));
                            }
                            Ok(mpris::Event::Seeked(pos)) => {
                                let _ = update_tx.blocking_send(MprisUpdate::Position(pos));
                            }
                            Ok(mpris::Event::PlayerShutDown) => {
                                let _ = update_tx.blocking_send(MprisUpdate::ShutDown);
                                break; // Exit the event loop to find a new player
                            }
                            _ => {}
                        }
                    }
                } else {
                    // Fallback if we fail to subscribe to events
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        });

        let mut last_track_id = String::new();
        let mut is_playing = false;
        let mut last_metadata: Option<MetadataUpdate> = None;

        while let Some(update) = update_rx.recv().await {
            match update {
                MprisUpdate::Metadata(meta) => {
                    last_metadata = Some(meta.clone());
                    
                    if is_playing && meta.track_id != last_track_id {
                        last_track_id = meta.track_id.clone();
                        info!("Track changed: {} - {}", meta.artist, meta.title);

                        let track_info = Self::build_track_info(&meta).await;
                        let _ = tx.send(Event::TrackChanged(track_info)).await;
                    } else if !is_playing {
                        // Just update the id, wait for playing status to trigger visual change
                        last_track_id = meta.track_id.clone();
                    }
                }
                MprisUpdate::Status(status) => {
                    let playing = status == mpris::PlaybackStatus::Playing;
                    if playing != is_playing {
                        is_playing = playing;
                        if is_playing {
                            // Resumed playing
                            if let Some(meta) = &last_metadata {
                                info!("Playback resumed: {} - {}", meta.artist, meta.title);
                                let track_info = Self::build_track_info(meta).await;
                                let _ = tx.send(Event::TrackChanged(track_info)).await;
                            }
                        } else {
                            // Paused or stopped
                            info!("Playback paused/stopped");
                            let _ = tx.send(Event::PlaybackStopped).await;
                        }
                    }
                }
                MprisUpdate::Position(pos) => {
                    let _ = tx.send(Event::PlaybackPosition(pos)).await;
                }
                MprisUpdate::ShutDown => {
                    if is_playing {
                        is_playing = false;
                        info!("Player shut down");
                        let _ = tx.send(Event::PlaybackStopped).await;
                    }
                    last_metadata = None;
                    last_track_id = String::new();
                }
            }
        }

        Ok(())
    }

    /// Build a TrackInfo from the extracted metadata, including fetching album art.
    async fn build_track_info(meta: &MetadataUpdate) -> TrackInfo {
        let art_future = async {
            if let Some(art_url) = &meta.art_url {
                match Self::fetch_album_art(art_url).await {
                    Ok(img) => {
                        let colours = extract_palette(&img);
                        (Some(img), Some(colours))
                    }
                    Err(e) => {
                        warn!("Failed to load album art: {}", e);
                        (None, None)
                    }
                }
            } else {
                (None, None)
            }
        };

        let lyrics_future = lrclib::fetch_synced_lyrics(&meta.title, &meta.artist, &meta.album);

        let ((album_art, palette), lyrics) = tokio::join!(art_future, lyrics_future);

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
        }
    }

    /// Fetch album art from a URL or local file path.
    async fn fetch_album_art(url: &str) -> Result<image::DynamicImage> {
        if url.starts_with("file://") {
            // Local file — decode directly
            let path = url.trim_start_matches("file://");
            let img = image::open(path)?;
            Ok(img)
        } else if url.starts_with("http") {
            // Remote URL — fetch with reqwest
            let bytes = reqwest::get(url).await?.bytes().await?;
            let img = image::load_from_memory(&bytes)?;
            Ok(img)
        } else {
            anyhow::bail!("Unsupported art URL scheme: {}", url)
        }
    }
}
