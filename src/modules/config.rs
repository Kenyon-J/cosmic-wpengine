// =============================================================================
// modules/config.rs
// =============================================================================
// Handles loading the config file from disk and providing defaults.
//
// Config lives at: ~/.config/cosmic-wallpaper/config.toml
//
// If the file doesn't exist, we create it with sensible defaults so the
// user has something to edit.
// =============================================================================

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The top-level config struct. Maps directly to the TOML file structure.
/// #[derive(Deserialize, Serialize)] means serde can automatically
/// convert between this struct and TOML text.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Which wallpaper mode to use
    pub mode: WallpaperMode,

    /// Target frames per second for the render loop.
    /// Lower values save GPU/CPU. 30fps is fine for wallpapers.
    pub fps: u32,

    /// Weather subsystem config (optional — user may not want this)
    pub weather: WeatherConfig,

    /// Audio visualiser config
    pub audio: AudioConfig,
}

/// Which visual mode the wallpaper runs in.
/// The renderer switches behaviour based on this.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WallpaperMode {
    /// Show album art from whatever is playing, blurred and colour-graded
    AlbumArt,

    /// Audio frequency visualiser — reacts to music in real time
    AudioVisualiser,

    /// Change the scene based on current weather conditions
    Weather,

    /// Cycle through modes automatically
    Auto,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WeatherConfig {
    /// Whether weather integration is enabled at all
    pub enabled: bool,

    /// Open-Meteo is a free, no-API-key weather service — perfect for this
    pub latitude: f64,
    pub longitude: f64,

    /// How often to poll for new weather data, in minutes
    pub poll_interval_minutes: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioConfig {
    /// How many frequency bands to show in the visualiser
    pub bands: usize,

    /// Smoothing factor 0.0–1.0. Higher = smoother but less reactive.
    pub smoothing: f32,
}

impl Config {
    /// Load config from disk, or create a default config file if none exists.
    pub fn load_or_default() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&text)?;
            Ok(config)
        } else {
            let config = Config::default();
            // Write default config to disk so user can edit it
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, toml::to_string_pretty(&config)?)?;
            tracing::info!("Created default config at {:?}", path);
            Ok(config)
        }
    }

    fn config_path() -> PathBuf {
        // Respects XDG_CONFIG_HOME if set, falls back to ~/.config
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").expect("HOME not set");
                PathBuf::from(home).join(".config")
            });
        base.join("cosmic-wallpaper").join("config.toml")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: WallpaperMode::Auto,
            fps: 30,
            weather: WeatherConfig {
                enabled: false,
                // Default to London — user should change this
                latitude: 51.5,
                longitude: -0.1,
                poll_interval_minutes: 15,
            },
            audio: AudioConfig {
                bands: 64,
                smoothing: 0.7,
            },
        }
    }
}
