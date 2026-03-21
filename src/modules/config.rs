use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::mpsc::Sender;

use super::event::Event;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub mode: WallpaperMode,
    pub fps: u32,
    pub weather: WeatherConfig,
    pub audio: AudioConfig,
    #[serde(default)]
    pub appearance: AppearanceConfig,
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
#[serde(rename_all = "snake_case")]
pub enum AudioStyle {
    Bars,
    Waveform,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioConfig {
    #[serde(default = "default_audio_style")]
    pub style: AudioStyle,
    pub bands: usize,
    pub smoothing: f32,
    pub color_top: Option<[f32; 3]>,
    pub color_bottom: Option<[f32; 3]>,
    #[serde(default = "default_show_lyrics")]
    pub show_lyrics: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppearanceConfig {
    #[serde(default)]
    pub disable_blur: bool,
    #[serde(default)]
    pub transparent_background: bool,
    pub custom_background_path: Option<String>,
}

fn default_show_lyrics() -> bool { true }
fn default_audio_style() -> AudioStyle { AudioStyle::Bars }

impl AppearanceConfig {
    pub fn resolved_background_path(&self) -> Option<String> {
        if self.custom_background_path.is_some() {
            return self.custom_background_path.clone();
        }

        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_default();
                PathBuf::from(home).join(".config")
            });

        let cosmic_bg_dir = base.join("cosmic/com.system76.CosmicBackground/v1");

        if let Ok(entries) = std::fs::read_dir(cosmic_bg_dir) {
            for entry in entries.flatten() {
                if let Ok(contents) = std::fs::read_to_string(entry.path()) {
                    // COSMIC uses RON format, storing wallpaper paths like: Path("/path/to/img.jpg")
                    if let Some(start_idx) = contents.find("Path(\"") {
                        let path_start = start_idx + 6;
                        if let Some(end_offset) = contents[path_start..].find("\")") {
                            let path = &contents[path_start..path_start + end_offset];
                            if std::path::Path::new(path).exists() {
                                return Some(path.to_string());
                            }
                        }
                    }
                }
            }
        }
        None
    }
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

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
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

    pub async fn watch(tx: Sender<Event>) -> Result<()> {
        let path = Self::config_path();
        let parent = path.parent().unwrap_or(std::path::Path::new("")).to_path_buf();
        let path_clone = path.clone();

        let cosmic_bg_dir = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_default();
                PathBuf::from(home).join(".config")
            })
            .join("cosmic/com.system76.CosmicBackground/v1");

        let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel();

        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = notify_tx.send(res);
        })?;

        watcher.watch(&parent, RecursiveMode::NonRecursive)?;
        if cosmic_bg_dir.exists() {
            let _ = watcher.watch(&cosmic_bg_dir, RecursiveMode::NonRecursive);
        }

        while let Some(res) = notify_rx.recv().await {
            if let Ok(event) = res {
                let is_our_config = event.paths.iter().any(|p| p == &path_clone);
                let is_cosmic_bg = event.paths.iter().any(|p| p.starts_with(&cosmic_bg_dir));

                if is_our_config || is_cosmic_bg {
                    // Slight debounce to ensure the text editor has finished writing the file
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    if let Ok(config) = Self::load_or_default() {
                        let _ = tx.send(Event::ConfigUpdated(config)).await;
                    }
                }
            }
        }
        Ok(())
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
                style: AudioStyle::Bars,
                bands: 64,
                smoothing: 0.7,
                color_top: None,
                color_bottom: None,
                show_lyrics: true,
            },
            appearance: AppearanceConfig::default(),
        }
    }
}
