use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub mode: WallpaperMode,
    pub fps: u32,
    pub weather: WeatherConfig,
    pub audio: AudioConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WallpaperMode {
    AlbumArt,
    AudioVisualiser,
    Weather,
    Auto,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WeatherConfig {
    pub enabled: bool,
    pub latitude: f64,
    pub longitude: f64,
    pub poll_interval_minutes: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioConfig {
    pub bands: usize,
    pub smoothing: f32,
}

impl Config {
    pub fn load_or_default() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&text)?;
            Ok(config)
        } else {
            let config = Config::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, toml::to_string_pretty(&config)?)?;
            tracing::info!("Created default config at {:?}", path);
            Ok(config)
        }
    }

    fn config_path() -> PathBuf {
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
