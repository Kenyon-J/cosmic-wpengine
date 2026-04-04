mod modules;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::info;

use modules::{
    audio::AudioCapture, config::Config, mpris::MprisWatcher, renderer::Renderer, state::AppState,
    tray::WallpaperTray, wayland::WaylandManager, weather::WeatherWatcher,
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

            let (is_visible_tx, is_visible_rx) = tokio::sync::watch::channel(true);
            let (show_lyrics_tx, show_lyrics_rx) =
                tokio::sync::watch::channel(config.audio.show_lyrics);

            let mpris_tx = event_tx.clone();
            let mpris_vis_rx = is_visible_rx.clone();
            let mpris_lyrics_rx = show_lyrics_rx.clone();
            tokio::task::spawn_local(async move {
                MprisWatcher::run(mpris_tx, mpris_vis_rx, mpris_lyrics_rx).await
            });

            let audio_tx = event_tx.clone();
            let audio_vis_rx = is_visible_rx.clone();
            tokio::spawn(async move { AudioCapture::run(audio_tx, audio_vis_rx).await });

            let (config_watch_tx, config_watch_rx) = tokio::sync::watch::channel(config.clone());

            let weather_tx = event_tx.clone();
            tokio::spawn(async move { WeatherWatcher::run(weather_tx, config_watch_rx).await });

            let config_tx = event_tx.clone();
            tokio::spawn(async move {
                if let Err(e) = Config::watch(config_tx, config_watch_tx).await {
                    tracing::warn!("Config watcher failed: {}", e);
                }
            });

            let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);

            let tray = WallpaperTray::new(shutdown_tx);
            ksni::TrayService::new(tray).spawn();

            let wayland_manager = WaylandManager::new()?;

            let mut renderer: Renderer =
                Renderer::new(&wayland_manager, state, show_lyrics_tx).await?;

            info!("All subsystems started. Entering render loop.");

            tokio::select! {
                res = renderer.run(event_rx, wayland_manager, is_visible_tx) => {
                    res?;
                }
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received. Initiating graceful exit...");
                }
            }

            info!("Exited cleanly.");

            Ok(())
        })
        .await
}
