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
    event::{Event, TrackInfo},
    colour::extract_palette,
};

pub struct MprisWatcher;

impl MprisWatcher {
    pub async fn run(tx: Sender<Event>) -> Result<()> {
        info!("MPRIS watcher started");

        // The mpris crate handles all the D-Bus communication for us.
        // PlayerFinder scans the session bus for any MPRIS-compliant player.
        let finder = mpris::PlayerFinder::new()
            .map_err(|e| anyhow::anyhow!("Could not connect to D-Bus: {}", e))?;

        loop {
            // Find the most recently active player.
            // If multiple players are running, this picks the one that last
            // received a Play command.
            match finder.find_active() {
                Ok(player) => {
                    info!("Found active player: {}", player.identity());
                    Self::watch_player(&player, &tx).await;
                }
                Err(_) => {
                    // No player found — send stopped event and wait
                    let _ = tx.send(Event::PlaybackStopped).await;
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                }
            }
        }
    }

    /// Watch a specific player until it disappears or goes silent.
    async fn watch_player(player: &mpris::Player, tx: &Sender<Event>) {
        let mut last_track_id = String::new();

        loop {
            // Check if the player is still running
            if !player.is_running() {
                info!("Player stopped running");
                let _ = tx.send(Event::PlaybackStopped).await;
                return;
            }

            // Get current playback status
            let is_playing = matches!(
                player.get_playback_status(),
                Ok(mpris::PlaybackStatus::Playing)
            );

            if !is_playing {
                let _ = tx.send(Event::PlaybackStopped).await;
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }

            // Get current track metadata
            if let Ok(metadata) = player.get_metadata() {
                let track_id = metadata.track_id()
                    .map(|id| id.to_string())
                    .unwrap_or_default();

                // Only process a track change if the track actually changed
                if track_id != last_track_id {
                    last_track_id = track_id;
                    info!(
                        "Track changed: {} - {}",
                        metadata.artists().unwrap_or_default().join(", "),
                        metadata.title().unwrap_or("Unknown")
                    );

                    let track_info = Self::build_track_info(&metadata).await;
                    let _ = tx.send(Event::TrackChanged(track_info)).await;
                }
            }

            // Poll every second — MPRIS doesn't push changes, we have to pull.
            // A future improvement would be to use D-Bus signals instead.
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    /// Build a TrackInfo from MPRIS metadata, including fetching album art.
    async fn build_track_info(metadata: &mpris::Metadata) -> TrackInfo {
        let title = metadata.title().unwrap_or("Unknown").to_string();
        let artist = metadata.artists()
            .unwrap_or_default()
            .join(", ");
        let album = metadata.album_name().unwrap_or("").to_string();

        // Album art URL — MPRIS provides this as a file:// or https:// URI
        let (album_art, palette) = if let Some(art_url) = metadata.art_url() {
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
        };

        TrackInfo { title, artist, album, album_art, palette }
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
