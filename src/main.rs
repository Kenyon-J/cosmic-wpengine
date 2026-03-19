mod modules;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::info;

use modules::{
    audio::AudioCapture, config::Config, mpris::MprisWatcher, renderer::Renderer, state::AppState,
    wayland::WaylandManager, weather::WeatherWatcher,
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

            let mpris_tx = event_tx.clone();
            tokio::task::spawn_local(async move { MprisWatcher::run(mpris_tx).await });

            let audio_tx = event_tx.clone();
            tokio::spawn(async move { AudioCapture::run(audio_tx).await });

            let weather_tx = event_tx.clone();
            let weather_config = config.weather.clone();
            tokio::spawn(async move { WeatherWatcher::run(weather_tx, weather_config).await });

            let wayland_manager = WaylandManager::new()?;

            let mut renderer: Renderer = Renderer::new(&wayland_manager, state).await?;

            info!("All subsystems started. Entering render loop.");

            renderer.run(event_rx, wayland_manager).await?;

            Ok(())
        })
        .await
}
