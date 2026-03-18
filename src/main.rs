// =============================================================================
// cosmic-wallpaper — main.rs
// =============================================================================
// This is the entry point for the wallpaper engine. It is responsible for:
//   1. Loading configuration
//   2. Spawning all subsystems (MPRIS, PipeWire, Weather) as async tasks
//   3. Creating the Wayland surface (the actual wallpaper layer)
//   4. Running the main render loop
//
// Think of this file as the "conductor" — it doesn't do much itself, but it
// starts and coordinates everything else.
// =============================================================================

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
    // Initialise logging — you'll see output in your terminal when running
    tracing_subscriber::fmt::init();
    info!("Starting cosmic-wallpaper...");

    // LocalSet is a container that allows running tasks which are NOT thread-safe
    // (i.e. not `Send`). The mpris crate uses `Rc` internally — a reference
    // counter that only works on a single thread — so we can't use the normal
    // tokio::spawn which may move tasks between threads. LocalSet pins everything
    // to the current thread, making `Rc`-based types safe to use.
    let local = tokio::task::LocalSet::new();

    // run_until drives the LocalSet on the current thread until the given
    // future completes. Everything inside here has access to spawn_local.
    local
        .run_until(async move {
            // Load config from ~/.config/cosmic-wallpaper/config.toml
            // If the file doesn't exist, we'll use sensible defaults
            let config = Config::load_or_default()?;
            info!("Config loaded: {:?}", config);

            // AppState is the shared "brain" of the application.
            // It holds the current wallpaper mode, album art, audio data, weather, etc.
            // All subsystems read from or write to this state.
            let state = AppState::new(config.clone());

            // We use channels to communicate between subsystems.
            // A channel is like a message queue — one side sends, the other receives.
            // This keeps each subsystem independent and avoids shared mutable state.
            let (event_tx, event_rx) = mpsc::channel(64);

            // --- Spawn subsystems as independent async tasks ---
            // Each task runs concurrently and sends events back via the channel.

            // MPRIS uses spawn_local because PlayerFinder contains Rc (not thread-safe).
            // spawn_local means "run this task on the same thread as the LocalSet".
            let mpris_tx = event_tx.clone();
            tokio::task::spawn_local(async move { MprisWatcher::run(mpris_tx).await });

            // Audio and weather are fine with regular spawn — they don't use Rc.
            let audio_tx = event_tx.clone();
            tokio::spawn(async move { AudioCapture::run(audio_tx).await });

            let weather_tx = event_tx.clone();
            let weather_config = config.weather.clone();
            tokio::spawn(async move { WeatherWatcher::run(weather_tx, weather_config).await });

            // --- Set up Wayland surface ---
            // This creates the layer surface that sits behind all your windows.
            // It's what we'll draw the wallpaper onto.
            let wayland_manager = WaylandManager::new()?;

            // --- Create the GPU renderer ---
            // The renderer owns the wgpu device and draws each frame.
            let mut renderer: Renderer = Renderer::new(&wayland_manager, state).await?;

            info!("All subsystems started. Entering render loop.");

            // --- Main render loop ---
            // On each iteration we:
            //   1. Process any pending events (new album art, audio frame, weather update)
            //   2. Update the visual state
            //   3. Draw a frame to the Wayland surface
            renderer.run(event_rx, wayland_manager).await?;

            Ok(())
        })
        .await
}
