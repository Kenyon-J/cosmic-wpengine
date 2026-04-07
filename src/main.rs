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
            let weather_config_rx = config_watch_rx.clone();
            tokio::spawn(async move { WeatherWatcher::run(weather_tx, weather_config_rx).await });

            let video_tx = event_tx.clone();
            let video_config_rx = config_watch_rx.clone();
            tokio::spawn(async move {
                spawn_video_watcher(video_tx, video_config_rx).await;
            });
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

fn start_video_decoder(
    video: &str,
    video_tx: mpsc::Sender<modules::event::Event>,
    video_config_rx: tokio::sync::watch::Receiver<Config>,
) -> Option<tokio::sync::watch::Sender<bool>> {
    let full_path = Config::config_dir().join("videos").join(video);
    if full_path.exists() {
        let (c_tx, c_rx) = tokio::sync::watch::channel(false);
        let (recycle_tx, recycle_rx) = tokio::sync::mpsc::channel(3);
        let tx_clone = video_tx.clone();

        let thread_config_rx = video_config_rx.clone();
        tokio::spawn(async move {
            let c_config_rx = thread_config_rx;
            let _ = modules::video::VideoDecoder::run_local_decoder(
                full_path.to_string_lossy().to_string(),
                tx_clone,
                c_rx,
                c_config_rx,
                recycle_rx,
                recycle_tx,
            )
            .await;
        });
        Some(c_tx)
    } else {
        None
    }
}

async fn spawn_video_watcher(
    video_tx: mpsc::Sender<modules::event::Event>,
    mut video_config_rx: tokio::sync::watch::Receiver<Config>,
) {
    let mut local_video_cancel_tx: Option<tokio::sync::watch::Sender<bool>> = None;

    // Read initial state
    let mut current_video_path: Option<String> = {
        let cfg = video_config_rx.borrow();
        cfg.appearance.video_background_path.clone()
    };

    if let Some(video) = &current_video_path {
        local_video_cancel_tx =
            start_video_decoder(video, video_tx.clone(), video_config_rx.clone());
    }

    while video_config_rx.changed().await.is_ok() {
        let path = video_config_rx
            .borrow()
            .appearance
            .video_background_path
            .clone();

        if path != current_video_path {
            if let Some(cancel) = local_video_cancel_tx.take() {
                let _ = cancel.send(true);
            }
            current_video_path = path.clone();

            if let Some(video) = &current_video_path {
                local_video_cancel_tx =
                    start_video_decoder(video, video_tx.clone(), video_config_rx.clone());
            }
        }
    }
}
