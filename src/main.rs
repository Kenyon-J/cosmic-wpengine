use cosmic_wallpaper::modules;

use anyhow::Result;
use ksni::TrayMethods;
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
    if let Some((out_path, compare_path, style)) = render_frame_harness_args() {
        return modules::renderer::render_frame_to_png(
            &out_path,
            compare_path.as_deref(),
            style.as_deref(),
        )
        .await;
    }

    info!("Starting cosmic-wallpaper...");

    let local = tokio::task::LocalSet::new();

    local
        .run_until(async move {
            let config = Config::load_or_default()?;
            info!("Config loaded: {:?}", config);
            modules::i18n::set_language(config.language.as_deref());

            let state = AppState::new(config.clone());

            let (event_tx, event_rx) = mpsc::channel(64);

            let (is_visible_tx, is_visible_rx) = tokio::sync::watch::channel(true);
            // Largest monitor size in physical pixels, published by the
            // renderer so the local video decoder can convert frames at
            // display size rather than the video's own (often larger) size.
            let (video_size_tx, video_size_rx) = tokio::sync::watch::channel(None);
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
            let video_vis_rx = is_visible_rx.clone();
            tokio::spawn(async move {
                spawn_video_watcher(video_tx, video_config_rx, video_vis_rx, video_size_rx).await;
            });
            let config_tx = event_tx.clone();
            tokio::spawn(async move {
                if let Err(e) = Config::watch(config_tx, config_watch_tx).await {
                    tracing::warn!("Config watcher failed: {}", e);
                }
            });

            let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);

            let tray = WallpaperTray::new(shutdown_tx);
            // The returned Handle only holds a Weak reference to the spawned
            // service - dropping it does not stop the tray, which keeps
            // running via its own background task. Nothing here currently
            // needs to push a live update or shut it down through the
            // handle, so it's discarded.
            tray.spawn().await?;

            let mut wayland_manager = WaylandManager::new()?;

            let mut renderer: Renderer =
                Renderer::new(&wayland_manager, state, show_lyrics_tx).await?;

            info!("All subsystems started. Entering render loop.");

            tokio::select! {
                res = renderer.run(event_rx, &mut wayland_manager, is_visible_tx, video_size_tx) => {
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

/// Parses `--render-frame <out.png> [--compare <baseline.png>] [--style
/// <name>]` out of the process args. `None` for any normal launch (no
/// `--render-frame` present). `--style` overrides the harness's default
/// synthetic scene's theme/audio style - e.g. to render against a theme
/// with a custom visualiser shader set, without touching a real saved
/// theme file.
fn render_frame_harness_args() -> Option<(
    std::path::PathBuf,
    Option<std::path::PathBuf>,
    Option<String>,
)> {
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
    let style = args
        .iter()
        .position(|a| a == "--style")
        .and_then(|i| args.get(i + 1))
        .cloned();
    Some((out_path, compare_path, style))
}

/// Receivers a local video decoder watches besides its own cancel signal.
struct VideoDecoderInputs {
    config_rx: tokio::sync::watch::Receiver<Config>,
    visible_rx: tokio::sync::watch::Receiver<bool>,
    size_rx: tokio::sync::watch::Receiver<Option<(u32, u32)>>,
}

fn start_video_decoder(
    video: &str,
    video_tx: mpsc::Sender<modules::event::Event>,
    inputs: &VideoDecoderInputs,
) -> Option<tokio::sync::watch::Sender<bool>> {
    let video_name = std::path::Path::new(video).file_name()?;
    let full_path = Config::config_dir().join("videos").join(video_name);
    if full_path.exists() {
        let (c_tx, c_rx) = tokio::sync::watch::channel(false);
        let (recycle_tx, recycle_rx) = tokio::sync::mpsc::channel(3);
        let tx_clone = video_tx.clone();

        let config_rx = inputs.config_rx.clone();
        let visible_rx = inputs.visible_rx.clone();
        let size_rx = inputs.size_rx.clone();
        tokio::spawn(async move {
            let _ = modules::video::VideoDecoder::run_local_decoder(
                full_path.to_string_lossy().to_string(),
                tx_clone,
                c_rx,
                config_rx,
                visible_rx,
                size_rx,
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
    visible_rx: tokio::sync::watch::Receiver<bool>,
    size_rx: tokio::sync::watch::Receiver<Option<(u32, u32)>>,
) {
    let inputs = VideoDecoderInputs {
        config_rx: video_config_rx.clone(),
        visible_rx,
        size_rx,
    };
    let mut local_video_cancel_tx: Option<tokio::sync::watch::Sender<bool>> = None;

    // The decoder is restarted when either the video or the hardware
    // decoding preference changes (the decoder reads that preference once,
    // when it opens the file).
    let video_settings = |config: &Config| {
        (
            config.appearance.video_background_path.clone(),
            config.appearance.hardware_video_decode,
        )
    };
    let mut current = video_settings(&video_config_rx.borrow());

    if let Some(video) = &current.0 {
        local_video_cancel_tx = start_video_decoder(video, video_tx.clone(), &inputs);
    }

    while video_config_rx.changed().await.is_ok() {
        let latest = video_settings(&video_config_rx.borrow());

        if latest != current {
            if let Some(cancel) = local_video_cancel_tx.take() {
                let _ = cancel.send(true);
            }
            current = latest;

            if let Some(video) = &current.0 {
                local_video_cancel_tx = start_video_decoder(video, video_tx.clone(), &inputs);
            }
        }
    }
}
