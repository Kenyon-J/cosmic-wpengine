use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};
use std::sync::{Arc, Mutex};

use super::config::{Config, TemperatureUnit, WallpaperMode};

pub struct WallpaperTray {
    pub config: Arc<Mutex<Config>>,
}

impl WallpaperTray {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
        }
    }

    fn update_config<F>(&self, f: F)
    where
        F: FnOnce(&mut Config),
    {
        let mut cfg = self.config.lock().unwrap();
        f(&mut cfg);
        if let Err(e) = cfg.save() {
            tracing::error!("Failed to save config from tray: {}", e);
        }
    }
}

impl ksni::Tray for WallpaperTray {
    fn icon_name(&self) -> String {
        "preferences-desktop-wallpaper".into()
    }

    fn title(&self) -> String {
        "COSMIC Wallpaper".into()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let current_config = self.config.lock().unwrap().clone();

        vec![
            CheckmarkItem {
                label: "Enable Frosted Blur".into(),
                checked: !current_config.appearance.disable_blur,
                activate: Box::new(|this: &mut Self| {
                    this.update_config(|c| c.appearance.disable_blur = !c.appearance.disable_blur);
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Transparent Background".into(),
                checked: current_config.appearance.transparent_background,
                activate: Box::new(|this: &mut Self| {
                    this.update_config(|c| c.appearance.transparent_background = !c.appearance.transparent_background);
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Show Synced Lyrics".into(),
                checked: current_config.audio.show_lyrics,
                activate: Box::new(|this: &mut Self| {
                    this.update_config(|c| c.audio.show_lyrics = !c.audio.show_lyrics);
                }),
                ..Default::default()
            }
            .into(),
        SubMenu {
            label: "Wallpaper Mode".into(),
            submenu: vec![
                CheckmarkItem {
                    label: "Auto (Reactive)".into(),
                    checked: current_config.mode == WallpaperMode::Auto,
                    activate: Box::new(|this: &mut Self| this.update_config(|c| c.mode = WallpaperMode::Auto)),
                    ..Default::default()
                }.into(),
                CheckmarkItem {
                    label: "Album Art".into(),
                    checked: current_config.mode == WallpaperMode::AlbumArt,
                    activate: Box::new(|this: &mut Self| this.update_config(|c| c.mode = WallpaperMode::AlbumArt)),
                    ..Default::default()
                }.into(),
                CheckmarkItem {
                    label: "Audio Visualiser".into(),
                    checked: current_config.mode == WallpaperMode::AudioVisualiser,
                    activate: Box::new(|this: &mut Self| this.update_config(|c| c.mode = WallpaperMode::AudioVisualiser)),
                    ..Default::default()
                }.into(),
                CheckmarkItem {
                    label: "Weather (Ambient)".into(),
                    checked: current_config.mode == WallpaperMode::Weather,
                    activate: Box::new(|this: &mut Self| this.update_config(|c| c.mode = WallpaperMode::Weather)),
                    ..Default::default()
                }.into(),
            ],
            ..Default::default()
        }.into(),
        CheckmarkItem {
            label: "Enable Weather Integration".into(),
            checked: current_config.weather.enabled,
            activate: Box::new(|this: &mut Self| {
                this.update_config(|c| c.weather.enabled = !c.weather.enabled);
            }),
            ..Default::default()
        }.into(),
        {
            let mut styles = vec!["bars".to_string(), "monstercat".to_string(), "waveform".to_string()];
            if let Ok(entries) = std::fs::read_dir(super::config::Config::config_dir().join("shaders")) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.ends_with(".toml") || name.ends_with(".wgsl") {
                            let style_name = name.trim_end_matches(".toml").trim_end_matches(".wgsl").to_string();
                            if !styles.contains(&style_name) { styles.push(style_name); }
                        }
                    }
                }
            }
            styles.sort();
            
            let mut style_menu: Vec<ksni::MenuItem<Self>> = Vec::new();
            for style in styles {
                let style_clone = style.clone();
                let label = match style.as_str() {
                    "bars" => "Circular (Bars)".to_string(),
                    "monstercat" => "Monstercat (Linear)".to_string(),
                    "waveform" => "Waveform".to_string(),
                    custom => {
                        let mut c = custom.chars();
                        match c.next() { None => String::new(), Some(f) => f.to_uppercase().collect::<String>() + c.as_str() }
                    }
                };
                style_menu.push(CheckmarkItem {
                    label,
                    checked: current_config.audio.style == style,
                    activate: Box::new(move |this: &mut Self| {
                        let s = style_clone.clone();
                        this.update_config(move |c| c.audio.style = s);
                    }),
                    ..Default::default()
                }.into());
            }

            SubMenu {
                label: "Visualiser Style".into(),
                submenu: style_menu,
                ..Default::default()
            }.into()
        },
            CheckmarkItem {
                label: "Use Fahrenheit (°F)".into(),
                checked: matches!(current_config.weather.temperature_unit, TemperatureUnit::Fahrenheit),
                activate: Box::new(|this: &mut Self| {
                    this.update_config(|c| c.weather.temperature_unit = match c.weather.temperature_unit {
                        TemperatureUnit::Celsius => TemperatureUnit::Fahrenheit,
                        TemperatureUnit::Fahrenheit => TemperatureUnit::Celsius,
                    });
                }),
                ..Default::default()
            }.into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Quit Engine".into(),
                activate: Box::new(|_| {
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}