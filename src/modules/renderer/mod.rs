pub(crate) mod audio_analysis;
pub(crate) mod blur;
pub mod core;
pub(crate) mod draw;
pub(crate) mod frame_params;
mod harness;
pub(crate) mod pipelines;
pub mod text;
pub mod types;
pub mod utils;

#[allow(unused_imports)]
pub use self::core::{GpuOutput, Renderer};
/// The only public surface of the dev-only offscreen render harness (see
/// `harness.rs`) - everything else it touches is crate-internal, reached
/// through this one entry point so the engine binary (a separate crate
/// from this library, same as an example binary would be) can call it.
pub use self::harness::render_frame_to_png;
