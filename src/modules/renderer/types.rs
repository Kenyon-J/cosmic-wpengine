#[repr(C, align(16))]
#[derive(Copy, Clone, Debug)]
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
#[derive(Copy, Clone, Debug)]
pub struct Particle {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub lifetime: f32,
    pub scale: f32,
}
