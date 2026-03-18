// =============================================================================
// modules/mod.rs
// =============================================================================
// Rust requires you to explicitly declare submodules here.
// Each module lives in its own file under src/modules/
// =============================================================================

pub mod audio; // PipeWire audio capture + FFT
pub mod colour;
pub mod config; // Configuration loading and defaults
pub mod event; // Event types passed between subsystems
pub mod lrclib; // Synced lyrics fetching
pub mod mpris; // MPRIS music player watcher
pub mod renderer; // wgpu GPU renderer + render loop
pub mod state; // Shared application state
pub mod wayland; // Wayland layer surface setup
pub mod weather; // Weather API polling
