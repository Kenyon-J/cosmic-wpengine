mod modules;

use anyhow::Result;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::mpsc;
use tracing::info;

use modules::{
    audio::AudioCapture, config::Config, mpris::MprisWatcher, renderer::Renderer,
    state::AppState, tray::WallpaperTray, wayland::WaylandManager, weather::WeatherWatcher,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting cosmic-wallpaper...");

    let local = tokio::task::LocalSet::new();

    local
        .run_until(async move {
            let config = Config::load_or_default()?;
            info!("Config loaded: {:?}", config);

            let state = AppState::new(config.clone());

            let (event_tx, event_rx) = mpsc::channel(64);

            let is_visible = Arc::new(AtomicBool::new(true));
            let show_lyrics = Arc::new(AtomicBool::new(config.audio.show_lyrics));

            let mpris_tx = event_tx.clone();
            let mpris_vis = is_visible.clone();
            let mpris_lyrics = show_lyrics.clone();
            tokio::task::spawn_local(async move { MprisWatcher::run(mpris_tx, mpris_vis, mpris_lyrics).await });

            let audio_tx = event_tx.clone();
            let audio_vis = is_visible.clone();
            tokio::spawn(async move { AudioCapture::run(audio_tx, audio_vis).await });

            let weather_tx = event_tx.clone();
            let weather_config = config.weather.clone();
            tokio::spawn(async move { WeatherWatcher::run(weather_tx, weather_config).await });

            let config_tx = event_tx.clone();
            tokio::spawn(async move {
                if let Err(e) = Config::watch(config_tx).await { tracing::warn!("Config watcher failed: {}", e); }
            });
            
            let tray = WallpaperTray::new(config.clone());
            ksni::TrayService::new(tray).spawn();

            let wayland_manager = WaylandManager::new()?;

            let mut renderer: Renderer = Renderer::new(&wayland_manager, state, show_lyrics).await?;

            info!("All subsystems started. Entering render loop.");

            renderer.run(event_rx, wayland_manager, is_visible).await?;

            Ok(())
        })
        .await
}
