//! CPU-side mirrors of the WGSL uniform/storage structs. All of them derive
//! `bytemuck::Pod`, which statically proves "no padding bytes, any bit
//! pattern valid" — so uploads use `bytemuck::bytes_of`/`cast_slice` instead
//! of unsafe `from_raw_parts` pointer casts. Pod's no-padding rule also
//! forbids `repr(align)`: the previous `align(16)` on `ArtUniforms` only
//! over-aligned the Rust struct (WGSL layout depends solely on the field
//! offsets, which repr(C) already fixes), so dropping it changes no bytes.
//! The explicit `_padding` fields keep each struct's size a multiple of 16
//! to satisfy WGSL uniform-buffer sizing.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ArtUniforms {
    pub color_and_transition: [f32; 4], // offset 0
    pub uv_transform: [f32; 4],         // offset 16 (scale_x, scale_y, offset_x, offset_y)
    pub art_position: [f32; 2],         // offset 32
    pub blur_step: [f32; 2],            // offset 40
    pub audio_energy: f32,              // offset 48
    pub mode: u32,                      // offset 52
    pub bg_alpha: f32,                  // offset 56
    pub art_size: f32,                  // offset 60
    pub shape: u32,                     // offset 64
    pub blur_opacity: f32,              // offset 68
    pub screen_aspect: f32,             // offset 72
    pub _padding: u32,                  // offset 76
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Particle {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub lifetime: f32,
    pub scale: f32,
}

/// Uniforms for the audio visualiser pass (`visualiser.wgsl`'s
/// `VisualiserUniforms`).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VisUniforms {
    pub res: [f32; 2],
    pub bands: u32,
    pub pulse: f32,
    pub top: [f32; 4],
    pub bottom: [f32; 4],
    pub pos_size_rot: [f32; 4],
    pub amplitude: f32,
    pub shape: u32,
    pub time: f32,
    pub align: u32,
    pub is_waveform: u32,
    pub _padding: [u32; 3],
}

/// Uniforms for the ambient (procedural sky) background pass.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct AmbUniforms {
    pub res: [f32; 2],
    pub time: f32,
    pub weather: u32,
    pub sky: [f32; 4],
    pub bg_alpha: f32,
    pub _padding: [f32; 3],
}
