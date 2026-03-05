// =============================================================================
// modules/mod.rs
// =============================================================================
// Rust requires you to explicitly declare submodules here.
// Each module lives in its own file under src/modules/
// =============================================================================

pub mod config;     // Configuration loading and defaults
pub mod state;      // Shared application state
pub mod event;      // Event types passed between subsystems
pub mod mpris;      // MPRIS music player watcher
pub mod audio;      // PipeWire audio capture + FFT
pub mod weather;    // Weather API polling
pub mod renderer;   // wgpu GPU renderer + render loop
pub mod wayland;    // Wayland layer surface setup
pub mod colour;     // Album art colour extraction utilities
