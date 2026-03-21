use ksni::menu::{CheckmarkItem, StandardItem};
use std::sync::{Arc, Mutex};

use super::config::Config;

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