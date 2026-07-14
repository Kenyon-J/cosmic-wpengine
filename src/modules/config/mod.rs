pub mod types;
pub use types::*;
mod tests;
use super::event::Event;
use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::mpsc::Sender;
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub mode: WallpaperMode,
    pub fps: u32,
    pub weather: WeatherConfig,
    pub audio: AudioConfig,
    pub appearance: AppearanceConfig,
}
impl Config {
    /// Clamps hand-editable values whose out-of-range settings would crash or
    /// destabilise the engine rather than just look wrong: fps = 0 panics in
    /// `Duration::from_secs_f64(1.0 / fps)`, and smoothing >= 1.0 flips the
    /// band lerp factor negative, making the visualiser diverge to NaN.
    pub fn sanitise(&mut self) {
        self.fps = self.fps.max(1);
        self.audio.smoothing = self.audio.smoothing.clamp(0.0, 0.99);
    }

    pub fn load_or_default() -> Result<Self> {
        let path = Self::config_path();

        // Extract default themes so users can find and edit them!
        let _ = ThemeLayout::write_defaults();

        let videos_dir = Self::config_dir().join("videos");
        let _ = std::fs::create_dir_all(&videos_dir);

        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Config>(&text) {
                Ok(mut config) => {
                    config.sanitise();
                    Ok(config)
                }
                Err(e) => {
                    tracing::error!(
                        "Syntax error in config.toml: {}. Falling back to default configuration!",
                        e
                    );
                    let _ = std::fs::rename(&path, path.with_extension("toml.bak"));
                    let default_config = Config::default();
                    let _ = std::fs::write(&path, toml::to_string_pretty(&default_config)?);
                    Ok(default_config)
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let config = Config::default();
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                {
                    use std::io::Write;
                    let _ = file.write_all(toml::to_string_pretty(&config)?.as_bytes());
                    tracing::info!("Created default config at {:?}", path);
                } else {
                    tracing::warn!(
                        "Config file may have been created concurrently at {:?}",
                        path
                    );
                }
                Ok(config)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Strict variant for live reloads: returns an error instead of replacing
    /// the file with defaults. `load_or_default`'s rename-and-rewrite fallback
    /// is right at startup (the app must come up with *something*), but from
    /// the file watcher it meant a typo saved mid-edit - or an editor's
    /// partial atomic write - could permanently wipe the user's settings.
    fn load_strict() -> Result<Self> {
        let text = std::fs::read_to_string(Self::config_path())?;
        let mut config: Config = toml::from_str(&text)?;
        config.sanitise();
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn available_videos() -> Vec<String> {
        let mut videos = Vec::new();
        let videos_dir = Self::config_dir().join("videos");
        if let Ok(entries) = std::fs::read_dir(videos_dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        if let Some(name) = entry.file_name().to_str() {
                            videos.push(name.to_string());
                        }
                    }
                }
            }
        }
        videos.sort();
        videos
    }

    pub fn config_dir() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
                PathBuf::from(home).join(".config")
            });
        base.join("cosmic-wallpaper")
    }

    fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub async fn watch(
        tx: Sender<Event>,
        watch_tx: tokio::sync::watch::Sender<Config>,
    ) -> Result<()> {
        let path = Self::config_path();
        let parent = path
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .to_path_buf();
        let path_clone = path.clone();
        let shaders_dir = parent.join("shaders");

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
        if shaders_dir.exists() {
            let _ = watcher.watch(&shaders_dir, RecursiveMode::NonRecursive);
        }
        if cosmic_bg_dir.exists() {
            let _ = watcher.watch(&cosmic_bg_dir, RecursiveMode::NonRecursive);
        }

        while let Some(res) = notify_rx.recv().await {
            if let Ok(event) = res {
                let is_our_config = event.paths.iter().any(|p| p == &path_clone);
                let is_our_shader = event.paths.iter().any(|p| p.starts_with(&shaders_dir));
                let is_cosmic_bg = event.paths.iter().any(|p| p.starts_with(&cosmic_bg_dir));

                if is_our_config || is_our_shader || is_cosmic_bg {
                    // Slight debounce to ensure the text editor has finished writing the file
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    // FLUSH pending events to prevent GUI sliders from queueing 100+ re-renders and locking up the engine!
                    while notify_rx.try_recv().is_ok() {}

                    // Safely offload synchronous I/O parsing to the blocking thread pool
                    match tokio::task::spawn_blocking(|| {
                        let config = Self::load_strict()?;
                        let theme = ThemeLayout::load(&config.audio.style);
                        Ok::<_, anyhow::Error>((config, theme))
                    })
                    .await
                    {
                        Ok(Ok((config, theme))) => {
                            let _ = watch_tx.send(config.clone());
                            let _ = tx
                                .send(Event::ConfigUpdated(Box::new(config), Box::new(theme)))
                                .await;
                        }
                        Ok(Err(e)) => {
                            tracing::warn!(
                                "Config reload failed ({}); keeping the current settings. \
                                 Fix config.toml and save again to apply.",
                                e
                            );
                        }
                        Err(e) => {
                            tracing::warn!("Config reload task failed: {}", e);
                        }
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
                temperature_unit: TemperatureUnit::Celsius,
                hide_effects: false,
            },
            audio: AudioConfig {
                style: "monstercat".to_string(),
                bands: 64,
                smoothing: 0.7,
                show_lyrics: true,
            },
            appearance: AppearanceConfig::default(),
        }
    }
}
