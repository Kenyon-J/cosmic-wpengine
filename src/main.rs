use cosmic_wallpaper::modules;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::info;

use modules::{
    audio::AudioCapture, config::Config, mpris::MprisWatcher, renderer::Renderer, state::AppState,
    tray::WallpaperTray, wayland::WaylandManager, weather::WeatherWatcher,
};

#[tokio::main]
async fn main() -> Result<()> {
    modules::logging::init("engine");

    // Hidden dev-only path, never reachable from a normal launch: renders
    // one frame against a fixed synthetic scene and exits, instead of
    // starting the engine proper. See modules::renderer::render_frame_to_png
    // and docs/PLAN-renderer-decomposition.md (phase 4) for why this exists -
    // it's the renderer decomposition's acceptance harness, letting a
    // refactor's before/after be diffed without a live desktop session.
    if let Some((out_path, compare_path)) = render_frame_harness_args() {
        return modules::renderer::render_frame_to_png(&out_path, compare_path.as_deref()).await;
    }

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

            let (config_watch_tx, config_watch_rx) = tokio::sync::watch::channel(config.clone());

            let mpris_tx = event_tx.clone();
            let mpris_vis_rx = is_visible_rx.clone();
            let mpris_lyrics_rx = show_lyrics_rx.clone();
            let mpris_config_rx = config_watch_rx.clone();
            tokio::task::spawn_local(async move {
                MprisWatcher::run(mpris_tx, mpris_vis_rx, mpris_lyrics_rx, mpris_config_rx).await
            });

            let audio_tx = event_tx.clone();
            let audio_vis_rx = is_visible_rx.clone();
            tokio::spawn(async move { AudioCapture::run(audio_tx, audio_vis_rx).await });

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

            let mut wayland_manager = WaylandManager::new()?;

            let mut renderer: Renderer =
                Renderer::new(&wayland_manager, state, show_lyrics_tx).await?;

            info!("All subsystems started. Entering render loop.");

            tokio::select! {
                res = renderer.run(event_rx, &mut wayland_manager, is_visible_tx) => {
                    res?;
                }
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received. Initiating graceful exit...");
                }
            }

            // Teardown order is load-bearing: the renderer's wgpu surfaces hold
            // raw pointers into wayland_manager's wl_surfaces/wl_display, and
            // Vulkan touches them when a surface is destroyed. Dropping the
            // connection first made every graceful exit segfault inside
            // libvulkan_radeon AFTER "Exited cleanly" was logged (the nightly
            // logout coredumps). The manager must outlive the renderer.
            drop(renderer);
            drop(wayland_manager);

            info!("Exited cleanly.");

            Ok(())
        })
        .await
}

/// Parses `--render-frame <out.png> [--compare <baseline.png>]` out of the
/// process args. `None` for any normal launch (no such flag present).
fn render_frame_harness_args() -> Option<(std::path::PathBuf, Option<std::path::PathBuf>)> {
    let args: Vec<String> = std::env::args().collect();
    let out_path = args
        .iter()
        .position(|a| a == "--render-frame")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from)?;
    let compare_path = args
        .iter()
        .position(|a| a == "--compare")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);
    Some((out_path, compare_path))
}

fn start_video_decoder(
    video: &str,
    video_tx: mpsc::Sender<modules::event::Event>,
    video_config_rx: tokio::sync::watch::Receiver<Config>,
) -> Option<tokio::sync::watch::Sender<bool>> {
    let video_name = std::path::Path::new(video).file_name()?;
    let full_path = Config::config_dir().join("videos").join(video_name);
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
