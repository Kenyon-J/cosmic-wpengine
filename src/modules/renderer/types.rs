#[repr(C, align(16))]
#[derive(Copy, Clone, Debug)]
pub struct ArtUniforms {
    pub color_and_transition: [f32; 4],
    pub res: [f32; 2],
    pub art_position: [f32; 2],
    pub audio_energy: f32,
    pub mode: u32,
    pub bg_alpha: f32,
    pub art_size: f32,
    pub shape: u32,
    pub blur_opacity: f32,
    pub image_res: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Particle {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub lifetime: f32,
    pub scale: f32,
    pub padding: [f32; 2], // Pad to 32 bytes to satisfy WGSL alignment rules
}
