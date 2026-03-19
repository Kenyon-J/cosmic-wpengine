use anyhow::Result;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use super::{
    colour::extract_palette,
    event::{Event, TrackInfo},
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

#[derive(serde::Deserialize)]
struct ITunesResponse {
    results: Vec<ITunesResult>,
}

#[derive(serde::Deserialize)]
struct ITunesResult {
    #[serde(rename = "artworkUrl100")]
    artwork_url: Option<String>,
}

pub struct MprisWatcher;

impl MprisWatcher {
    pub async fn run(tx: Sender<Event>) -> Result<()> {
        info!("MPRIS watcher started");

        let (update_tx, mut update_rx) = tokio::sync::mpsc::channel(16);
        
        // Background position polling to handle media players that fail to send Seeked signals
        let poll_tx = tx.clone();
        tokio::task::spawn_blocking(move || {
            loop {
                let finder = match mpris::PlayerFinder::new() {
                    Ok(f) => f,
                    Err(_) => continue, // Keep retrying if DBus is temporarily unavailable
                };
                std::thread::sleep(std::time::Duration::from_secs(1));
                if let Ok(player) = finder.find_active() {
                    if let Ok(status) = player.get_playback_status() {
                        if status == mpris::PlaybackStatus::Playing {
                            if let Ok(pos) = player.get_position() {
                                let _ = poll_tx.blocking_send(Event::PlaybackPosition(pos));
                            }
                        }
                    }
                }
            }
        });

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

                if let Ok(metadata) = player.get_metadata() {
                    let _ = update_tx.blocking_send(MprisUpdate::Metadata(MetadataUpdate::from_metadata(&metadata)));
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
                            Ok(mpris::Event::Seeked { position_in_us }) => {
                                let pos = std::time::Duration::from_micros(position_in_us);
                                let _ = update_tx.blocking_send(MprisUpdate::Position(pos));
                            }
                            Ok(mpris::Event::PlayerShutDown) => {
                                let _ = update_tx.blocking_send(MprisUpdate::ShutDown);
                                break;
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
        let mut last_metadata: Option<MetadataUpdate> = None;

        while let Some(update) = update_rx.recv().await {
            match update {
                MprisUpdate::Metadata(meta) => {
                    // Ignore empty metadata transitions often sent between tracks
                    let is_empty = (meta.title == "Unknown" || meta.title.trim().is_empty())
                        && meta.artist.trim().is_empty();
                    if is_empty {
                        continue;
                    }

                    let changed = last_metadata.as_ref() != Some(&meta);
                    if changed {
                        last_metadata = Some(meta.clone());
                        
                        info!("Track changed: {} - {}", meta.artist, meta.title);
                        let track_info = Self::build_track_info(&meta).await;
                        let _ = tx.send(Event::TrackChanged(track_info)).await;
                    }
                }
                MprisUpdate::Status(status) => {
                    let playing = status == mpris::PlaybackStatus::Playing;
                    if playing != is_playing {
                        is_playing = playing;
                        if is_playing {
                            if let Some(meta) = &last_metadata {
                                info!("Playback resumed: {} - {}", meta.artist, meta.title);
                                let track_info = Self::build_track_info(meta).await;
                                let _ = tx.send(Event::TrackChanged(track_info)).await;
                            }
                        } else {
                            info!("Playback paused/stopped");
                            let _ = tx.send(Event::PlaybackStopped).await;
                        }
                    }
                }
                MprisUpdate::Position(pos) => {
                    let _ = tx.send(Event::PlaybackPosition(pos)).await;
                }
                MprisUpdate::ShutDown => {
                    is_playing = false;
                    info!("Player shut down");
                    let _ = tx.send(Event::PlayerShutDown).await;
                    last_metadata = None;
                }
            }
        }

        Ok(())
    }

    async fn build_track_info(meta: &MetadataUpdate) -> TrackInfo {
        let art_future = async {
            let mut local_art = None;
            if let Some(art_url) = &meta.art_url {
                match Self::fetch_album_art(art_url).await {
                    Ok(img) => {
                        let colours = extract_palette(&img);
                        local_art = Some((Some(img), Some(colours)));
                    }
                    Err(e) => {
                        warn!("Failed to load local album art (likely Flatpak isolation): {}", e);
                    }
                }
            }
            
            if let Some(art) = local_art {
                art
            } else {
                info!("Attempting to fetch fallback album art online...");
                match Self::fetch_fallback_album_art(&meta.artist, &meta.album, &meta.title).await {
                    Ok(img) => {
                        let colours = extract_palette(&img);
                        (Some(img), Some(colours))
                    }
                    Err(e) => {
                        warn!("Fallback art failed: {}", e);
                        (None, None)
                    }
                }
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

    async fn fetch_album_art(url: &str) -> Result<image::DynamicImage> {
        // Properly decode all URL percent-encoding (e.g. %27 -> apostrophe)
        let mut decoded_url = String::with_capacity(url.len());
        let mut chars = url.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '%' {
                let h1 = chars.next().unwrap_or('0');
                let h2 = chars.next().unwrap_or('0');
                let hex = format!("{}{}", h1, h2);
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    decoded_url.push(byte as char);
                } else {
                    decoded_url.push('%'); decoded_url.push(h1); decoded_url.push(h2);
                }
            } else {
                decoded_url.push(c);
            }
        }

        if decoded_url.starts_with("file://") {
            let path = decoded_url.trim_start_matches("file://");
            let bytes = std::fs::read(path)?;
            let img = image::load_from_memory(&bytes)?;
            Ok(img)
        } else if decoded_url.starts_with("http") {
            let bytes = reqwest::get(&decoded_url).await?.bytes().await?;
            let img = image::load_from_memory(&bytes)?;
            Ok(img)
        } else {
            anyhow::bail!("Unsupported art URL scheme: {}", decoded_url)
        }
    }

    async fn fetch_fallback_album_art(artist: &str, album: &str, title: &str) -> Result<image::DynamicImage> {
        let search_str = if album.is_empty() || album == "Unknown" {
            format!("{} {}", artist, title)
        } else {
            format!("{} {}", artist, album)
        };
        
        let term = search_str.replace(" ", "+");
        let url = format!("https://itunes.apple.com/search?term={}&entity=song&limit=1", term);
        
        let resp: ITunesResponse = reqwest::get(&url).await?.json().await?;
        
        if let Some(first) = resp.results.first() {
            if let Some(art_url) = &first.artwork_url {
                let high_res_url = art_url.replace("100x100bb", "600x600bb");
                let bytes = reqwest::get(&high_res_url).await?.bytes().await?;
                return Ok(image::load_from_memory(&bytes)?);
            }
        }
        anyhow::bail!("No fallback art found")
    }
}
