use anyhow::Result;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Receiver;
use tracing::{info, warn};
use wgpu_text::glyph_brush::{HorizontalAlign, Layout, Section, Text};

use super::{
    colour::{lerp_colour, time_to_sky_colour},
    event::Event,
    state::{AppState, SceneHint},
    wayland::WaylandManager,
    visualiser_pass::VisualiserPass,
};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct Particle {
    pos: [f32; 2],
    vel: [f32; 2],
    lifetime: f32,
    scale: f32,
    padding: [f32; 2], // Pad to 32 bytes to satisfy WGSL alignment rules
}

pub struct GpuOutput {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub text_brush: wgpu_text::TextBrush,
}

pub struct Renderer {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    outputs: Vec<GpuOutput>,
    fonts: Vec<wgpu_text::glyph_brush::ab_glyph::FontArc>,
    visualiser_pass: VisualiserPass,
    album_art_pipeline: wgpu::RenderPipeline,
    album_art_layout: wgpu::BindGroupLayout,
    album_art_bg_uniform_buffer: wgpu::Buffer,
    album_art_fg_uniform_buffer: wgpu::Buffer,
    album_art_bg_bind_group: Option<wgpu::BindGroup>,
    album_art_fg_bind_group: Option<wgpu::BindGroup>,
    previous_album_view: wgpu::TextureView,
    current_album_view: wgpu::TextureView,
    current_album_texture: Option<wgpu::Texture>,
    ambient_pipeline: wgpu::RenderPipeline,
    ambient_bind_group: wgpu::BindGroup,
    ambient_uniform_buffer: wgpu::Buffer,
    custom_bg_uniform_buffer: wgpu::Buffer,
    custom_bg_bind_group: Option<wgpu::BindGroup>,
    current_bg_path: Option<String>,
    _particle_buffer: wgpu::Buffer,
    weather_compute_uniform_buffer: wgpu::Buffer,
    weather_compute_bind_group: wgpu::BindGroup,
    weather_compute_pipeline: wgpu::ComputePipeline,
    weather_render_bind_group: wgpu::BindGroup,
    weather_render_pipeline: wgpu::RenderPipeline,
    start_time: Instant,
    state: AppState,
    frame_duration: Duration,
    show_lyrics_atomic: std::sync::Arc<std::sync::atomic::AtomicBool>,
    bass_moving_average: f32,
    beat_pulse: f32,
    last_beat_time: Instant,
    treble_moving_average: f32,
    treble_pulse: f32,
    last_treble_time: Instant,
    theme: super::config::ThemeLayout,
    a_weighting_curve: Vec<f32>,
    frequency_bin_ranges: Vec<(usize, usize)>,
    waveform_bin_ranges: Vec<(usize, usize)>,
    lyric_bounce_value: f32,
    lyric_bounce_velocity: f32,
    cached_track_str: String,
    cached_weather_str: String,
}

impl Renderer {
    pub async fn new(
        wayland_manager: &WaylandManager, 
        state: AppState,
        show_lyrics_atomic: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self> {
        let fps = state.config.fps;

        info!("Initialising wgpu renderer...");

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });

        let outputs_info = wayland_manager.outputs();
        if outputs_info.is_empty() {
            anyhow::bail!("No Wayland outputs found to render to");
        }

        let mut surfaces = Vec::new();
        for info in &outputs_info {
            let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: info.raw_display_handle(),
                raw_window_handle: info.raw_window_handle(),
            };
            surfaces.push(unsafe { instance.create_surface_unsafe(target) }?);
        }

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surfaces[0]),
            force_fallback_adapter: false,
        }).await.expect("No suitable GPU adapter found");

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("COSMIC Wallpaper Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None
        ).await?;

        // Lookup a reliable system font used in Linux/COSMIC environments
        let font_bytes = std::fs::read("/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf")
            .or_else(|_| std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"))
            .or_else(|_| std::fs::read("/usr/share/fonts/liberation/LiberationSans-Regular.ttf"))
            .or_else(|_| std::fs::read("/usr/share/fonts/TTF/DejaVuSans.ttf")) // Arch Linux
            .or_else(|_| std::fs::read("/usr/share/fonts/TTF/LiberationSans-Regular.ttf")) // Arch Linux
            .or_else(|_| std::fs::read("/usr/share/fonts/noto/NotoSans-Regular.ttf")) // Arch/Fedora
            .or_else(|_| std::fs::read("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf")) // Debian/Ubuntu
            .or_else(|_| std::fs::read("/usr/share/fonts/cantarell/Cantarell-Regular.ttf")) // Fedora/GNOME
            .or_else(|_| std::fs::read("/usr/share/fonts/TTF/Cantarell-Regular.ttf")) // Arch GNOME
            .expect("Could not find a valid system font! Please install 'ttf-dejavu', 'ttf-liberation', or 'noto-fonts'.");
        let primary_font = wgpu_text::glyph_brush::ab_glyph::FontArc::try_from_vec(font_bytes).unwrap();

        let mut fonts = vec![primary_font];
        
        let fallback_paths = [
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", // Arch
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", // Ubuntu/Debian
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc", // Fedora
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf", // Generic Fallback
            "/usr/share/fonts/wqy-microhei/wqy-microhei.ttc", // Generic Fallback
        ];
        for path in fallback_paths {
            if let Ok(bytes) = std::fs::read(path) {
                if let Ok(fallback_font) = wgpu_text::glyph_brush::ab_glyph::FontArc::try_from_vec(bytes) {
                    fonts.push(fallback_font);
                }
            }
        }

        let mut outputs = Vec::new();
        for (info, surface) in outputs_info.into_iter().zip(surfaces) {
            let caps = surface.get_capabilities(&adapter);
            let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);

            let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
                wgpu::CompositeAlphaMode::PreMultiplied
            } else if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
                wgpu::CompositeAlphaMode::PostMultiplied
            } else {
                caps.alpha_modes[0]
            };

            let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
                wgpu::PresentMode::Mailbox
            } else if caps.present_modes.contains(&wgpu::PresentMode::FifoRelaxed) {
                wgpu::PresentMode::FifoRelaxed
            } else {
                caps.present_modes[0]
            };

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: info.width,
                height: info.height,
                present_mode,
                alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);
            
            let text_brush = wgpu_text::BrushBuilder::using_fonts(fonts.clone())
                .build(&device, info.width, info.height, format);
                
            outputs.push(GpuOutput { surface, config, text_brush });
        }

        let config_format = outputs[0].config.format;

        // --- Visualiser Pipeline Setup ---
        let visualiser_pass = VisualiserPass::new(&device, config_format, state.config.audio.bands, &state.config.audio.style).await;

        // --- Album Art Pipeline Setup ---
        let empty_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("Empty Album Art Texture"),
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &empty_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[0, 0, 0, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );

        let previous_album_view = empty_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let current_album_view = empty_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let album_art_bg_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Album Art BG Uniform Buffer"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let album_art_fg_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Album Art FG Uniform Buffer"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let album_art_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Album Art Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(48),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let album_art_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Album Art Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("album_art.wgsl").into()),
        });

        let album_art_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Album Art Pipeline Layout"),
            bind_group_layouts: &[&album_art_layout],
            push_constant_ranges: &[],
        });

        let album_art_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Album Art Render Pipeline"),
            layout: Some(&album_art_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &album_art_shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &album_art_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // --- Ambient Pipeline Setup ---
        let ambient_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Ambient Uniform Buffer"),
            size: 64, 
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let ambient_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Ambient Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(64),
                    },
                    count: None,
                },
            ],
        });

        let ambient_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ambient Bind Group"),
            layout: &ambient_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ambient_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let ambient_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Ambient Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ambient.wgsl").into()),
        });

        let ambient_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Ambient Pipeline Layout"),
            bind_group_layouts: &[&ambient_bind_group_layout],
            push_constant_ranges: &[],
        });

        let ambient_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Ambient Render Pipeline"),
            layout: Some(&ambient_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &ambient_shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &ambient_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let custom_bg_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Custom BG Uniform Buffer"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // --- Weather Compute Pipeline Setup ---
        let mut initial_particles = Vec::with_capacity(10000);
        for i in 0..10000 {
            initial_particles.push(Particle {
                pos: [
                    (i as f32 * 12.9898).sin().fract() * 2.0 - 0.5, // Random X scatter
                    -0.2 - (i as f32 * 0.01),                       // Staggered starting heights
                ],
                vel: [0.0, 0.5 + (i as f32 % 5.0) * 0.1], // Base downward velocity
                lifetime: 5.0 + (i as f32 % 5.0),
                scale: 1.0,
                padding: [0.0; 2],
            });
        }

        let particle_buffer_size = (initial_particles.len() * std::mem::size_of::<Particle>()) as wgpu::BufferAddress;
        let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Storage Buffer"),
            size: particle_buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let particles_bytes = unsafe {
            std::slice::from_raw_parts(initial_particles.as_ptr() as *const u8, particle_buffer_size as usize)
        };
        queue.write_buffer(&particle_buffer, 0, particles_bytes);

        let weather_compute_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Weather Compute Uniform Buffer"),
            size: 16, // delta_time, wind_x, gravity, padding
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let weather_compute_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Weather Compute Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry { // Storage Buffer (Read/Write)
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(particle_buffer_size),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Uniform Buffer (delta_time, physics)
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });

        let weather_compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Weather Compute Bind Group"),
            layout: &weather_compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: particle_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: weather_compute_uniform_buffer.as_entire_binding() },
            ],
        });

        let weather_compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Weather Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("weather_compute.wgsl").into()),
        });

        let weather_compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Weather Compute Pipeline Layout"),
            bind_group_layouts: &[&weather_compute_bind_group_layout],
            push_constant_ranges: &[],
        });

        let weather_compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Weather Compute Pipeline"),
            layout: Some(&weather_compute_pipeline_layout),
            module: &weather_compute_shader,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        });

        // --- Weather Render Pipeline Setup ---
        let weather_render_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Weather Render Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(particle_buffer_size),
                    },
                    count: None,
                },
            ],
        });

        let weather_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Weather Render Bind Group"),
            layout: &weather_render_bind_group_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: particle_buffer.as_entire_binding() }],
        });

        let weather_render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Weather Render Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("weather_render.wgsl").into()),
        });

        let weather_render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Weather Render Pipeline Layout"),
            bind_group_layouts: &[&weather_render_bind_group_layout],
            push_constant_ranges: &[],
        });

        let weather_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Weather Render Pipeline"),
            layout: Some(&weather_render_pipeline_layout),
            vertex: wgpu::VertexState { module: &weather_render_shader, entry_point: "vs_main", buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &weather_render_shader, entry_point: "fs_main", targets: &[Some(wgpu::ColorTargetState { format: config_format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: None, multisample: wgpu::MultisampleState::default(), multiview: None,
        });

        let theme = super::config::ThemeLayout::load(&state.config.audio.style);
        let a_weighting_curve = Self::build_a_weighting_curve(state.config.audio.bands);
        let frequency_bin_ranges = Self::build_frequency_bin_ranges(state.config.audio.bands);
        let waveform_bin_ranges = Self::build_waveform_bin_ranges(state.config.audio.bands);

        let mut renderer = Self {
            instance,
            adapter,
            device,
            queue,
            outputs,
            fonts,
            visualiser_pass,
            album_art_pipeline,
            album_art_layout,
            album_art_bg_uniform_buffer,
            album_art_fg_uniform_buffer,
            album_art_bg_bind_group: None,
            album_art_fg_bind_group: None,
            previous_album_view,
            current_album_view,
            current_album_texture: None,
            ambient_pipeline,
            ambient_bind_group,
            ambient_uniform_buffer,
            custom_bg_uniform_buffer,
            custom_bg_bind_group: None,
            current_bg_path: None,
            _particle_buffer: particle_buffer,
            weather_compute_uniform_buffer,
            weather_compute_bind_group,
            weather_compute_pipeline,
            weather_render_bind_group,
            weather_render_pipeline,
            start_time: Instant::now(),
            state,
            frame_duration: Duration::from_secs_f64(1.0 / fps as f64),
            show_lyrics_atomic,
            bass_moving_average: 0.0,
            beat_pulse: 0.0,
            last_beat_time: Instant::now(),
            treble_moving_average: 0.0,
            treble_pulse: 0.0,
            last_treble_time: Instant::now(),
            theme,
            a_weighting_curve,
            frequency_bin_ranges,
            waveform_bin_ranges,
            lyric_bounce_value: 0.0,
            lyric_bounce_velocity: 0.0,
            cached_track_str: String::new(),
            cached_weather_str: String::new(),
        };

        let path = renderer.state.config.appearance.resolved_background_path();
        renderer.current_bg_path = path.clone();
        renderer.load_custom_background(path.as_deref());

        info!("Renderer initialised at {}fps", fps);
        Ok(renderer)
    }

    pub async fn run(
        &mut self,
        mut event_rx: Receiver<Event>,
        mut wayland_manager: WaylandManager,
        is_visible: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<()> {
        let mut last_frame = Instant::now();
        
        let mut interval = tokio::time::interval(self.frame_duration);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        wayland_manager.update_opaque_regions(self.state.config.appearance.transparent_background);

        loop {
            interval.tick().await;
            
            let occluded = wayland_manager.is_occluded();
            is_visible.store(!occluded, std::sync::atomic::Ordering::Relaxed);

            wayland_manager.dispatch_events()?;
            
            let current_outputs = wayland_manager.outputs();
            if current_outputs.len() != self.outputs.len() {
                info!("Monitor configuration changed ({} -> {} outputs), rebuilding GPU surfaces...", self.outputs.len(), current_outputs.len());
                self.outputs.clear();
                for info in &current_outputs {
                    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: info.raw_display_handle(),
                        raw_window_handle: info.raw_window_handle(),
                    };
                    let surface = unsafe { self.instance.create_surface_unsafe(target) }
                        .expect("Failed to recreate surface");

                    let caps = surface.get_capabilities(&self.adapter);
                    let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);

                    let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
                        wgpu::CompositeAlphaMode::PreMultiplied
                    } else if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
                        wgpu::CompositeAlphaMode::PostMultiplied
                    } else {
                        caps.alpha_modes[0]
                    };

                    let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
                        wgpu::PresentMode::Mailbox
                    } else if caps.present_modes.contains(&wgpu::PresentMode::FifoRelaxed) {
                        wgpu::PresentMode::FifoRelaxed
                    } else {
                        caps.present_modes[0]
                    };

                    let config = wgpu::SurfaceConfiguration {
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        format,
                        width: info.width.max(1),
                        height: info.height.max(1),
                        present_mode,
                        alpha_mode,
                        view_formats: vec![],
                        desired_maximum_frame_latency: 2,
                    };
                    surface.configure(&self.device, &config);
                    
                    let text_brush = wgpu_text::BrushBuilder::using_fonts(self.fonts.clone())
                        .build(&self.device, config.width, config.height, format);
                        
                    self.outputs.push(GpuOutput { surface, config, text_brush });
                }
            }
            
            for (i, win) in wayland_manager.app_data.windows.iter().enumerate() {
                if let Some(gpu_out) = self.outputs.get_mut(i) {
                    if gpu_out.config.width != win.width || gpu_out.config.height != win.height {
                        info!("Resizing output {} to {}x{}", i, win.width, win.height);
                        gpu_out.config.width = win.width.max(1);
                        gpu_out.config.height = win.height.max(1);
                        gpu_out.surface.configure(&self.device, &gpu_out.config);
                        gpu_out.text_brush.resize_view(gpu_out.config.width as f32, gpu_out.config.height as f32, &self.queue);
                    }
                }
            }

            let mut transparent_changed = false;
            let previous_duration = self.frame_duration;

            while let Ok(event) = event_rx.try_recv() {
                if let Event::ConfigUpdated(ref config) = event {
                    if config.appearance.transparent_background != self.state.config.appearance.transparent_background {
                        transparent_changed = true;
                    }
                }
                self.handle_event(event).await;
            }

            if transparent_changed {
                wayland_manager.update_opaque_regions(self.state.config.appearance.transparent_background);
            }

            if self.frame_duration != previous_duration {
                interval = tokio::time::interval(self.frame_duration);
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            }

            self.state.update_time();

            let now = Instant::now();
            let delta = now.duration_since(last_frame).as_secs_f32();
            self.state.tick_transition(delta);
            last_frame = now;
            
            // Exponential decay for the beat pulse so it snaps up and softly falls down
            self.beat_pulse *= (-12.0 * delta).exp();
            // Treble decays slightly faster for snappier, rapid hi-hats
            self.treble_pulse *= (-15.0 * delta).exp();

            // Spring physics for organic lyric bounce (Hooke's Law)
            let stiffness = 150.0;
            let damping = 12.0; // Slightly underdamped for a natural spring overshoot
            let spring_force = -stiffness * self.lyric_bounce_value - damping * self.lyric_bounce_velocity;
            self.lyric_bounce_velocity += spring_force * delta;
            self.lyric_bounce_value += self.lyric_bounce_velocity * delta;

            if wayland_manager.any_monitor_ready() {
                self.draw_frame(&mut wayland_manager, delta)?;
            }
        }
    }

    async fn handle_event(&mut self, event: Event) {
        match event {
            Event::ConfigUpdated(config) => {
                self.show_lyrics_atomic.store(config.audio.show_lyrics, std::sync::atomic::Ordering::Relaxed);
                self.frame_duration = Duration::from_secs_f64(1.0 / config.fps as f64);
                
                let new_bg = config.appearance.resolved_background_path();
                if new_bg != self.current_bg_path {
                    self.load_custom_background(new_bg.as_deref());
                    self.current_bg_path = new_bg;
                }

                if config.audio.bands != self.state.config.audio.bands {
                    self.state.audio_bands = vec![0.0; config.audio.bands];
                    self.a_weighting_curve = Self::build_a_weighting_curve(config.audio.bands);
                    self.frequency_bin_ranges = Self::build_frequency_bin_ranges(config.audio.bands);
                    self.waveform_bin_ranges = Self::build_waveform_bin_ranges(config.audio.bands);
                }
                
                // Always reload the shader pipeline so live WGSL edits apply instantly!
                let format = self.outputs[0].config.format;
                self.visualiser_pass.reload(&self.device, format, &config.audio.style, config.audio.bands).await;

                // Always reload the theme layout so live edits to the .toml apply instantly!
                self.theme = super::config::ThemeLayout::load(&config.audio.style);
                self.state.config = config;
                self.update_weather_string();
                info!("Live settings applied!");
            }
            Event::TrackChanged(track) => {
                info!("Now playing: {} - {}", track.artist, track.title);
                if let Some(art) = &track.album_art {
                    self.update_album_art_texture(art);
                }
                self.cached_track_str = format!("{} — {}\n{}", track.title, track.artist, track.album);
                self.state.previous_palette = self.state.current_track.as_ref().and_then(|t| t.palette.clone());
                self.state.current_track = Some(track);
                self.state.is_playing = true;
                self.state.begin_transition();
            }

            Event::PlaybackStopped => {
                self.state.is_playing = false;
                // We intentionally do not clear the track here so it remains visible while paused
            }

            Event::PlaybackResumed => {
                self.state.is_playing = true;
            }

            Event::VideoFrame(frame) => {
                self.update_video_frame(&frame);
            }

            Event::PlayerShutDown => {
                self.cached_track_str.clear();
                self.state.previous_palette = self.state.current_track.as_ref().and_then(|t| t.palette.clone());
                self.state.current_track = None;
                self.state.is_playing = false;
                self.state.begin_transition();
            }

            Event::PlaybackPosition(pos) => {
                self.state.playback_position = pos;
            }

            Event::AudioFrame { bands, waveform } => {
                let smoothing = self.state.config.audio.smoothing;
                let target_len = self.state.audio_bands.len();
                
                let min_freq = 40.0f32;
                let max_freq = 16000.0f32;
                let sample_rate = 48000.0f32;
                let fft_size = 2048.0f32;
                let freq_per_bin = sample_rate / fft_size;
                
                // --- Smart Beat Detection ---
                // We focus strictly on the low-end frequencies (e.g. 20Hz - 120Hz)
                let bass_min_bin = (20.0 / freq_per_bin).floor() as usize;
                let bass_max_bin = (120.0 / freq_per_bin).ceil() as usize;
                
                let mut current_bass = 0.0f32;
                let mut count = 0;
                for &val in &bands[bass_min_bin..=bass_max_bin.min(bands.len() - 1)] {
                    current_bass += val;
                    count += 1;
                }
                if count > 0 {
                    current_bass /= count as f32;
                }
                
                // Moving average for a local bass energy threshold (~1 second tracker)
                self.bass_moving_average = self.bass_moving_average * 0.95 + current_bass * 0.05;
                
                // Trigger a beat if the bass spikes significantly above the recent average
                if current_bass > self.bass_moving_average * 1.5 && current_bass > 0.02
                    && self.last_beat_time.elapsed().as_millis() > 200 { // 200ms cooldown prevents double-triggering
                        self.beat_pulse = 1.0;
                        
                        // Add physical velocity to the lyric spring. The harder the bass spike, the bigger the bounce!
                        let spike = (current_bass / self.bass_moving_average.max(0.001)).clamp(1.5, 3.0);
                        self.lyric_bounce_velocity += 15.0 * spike;
                        self.last_beat_time = Instant::now();
                    }

                // --- Smart Treble Detection (Snares / Hi-Hats) ---
                let treble_min_bin = (3000.0 / freq_per_bin).floor() as usize;
                let treble_max_bin = (8000.0 / freq_per_bin).ceil() as usize;
                
                let mut current_treble = 0.0f32;
                let mut t_count = 0;
                for &val in &bands[treble_min_bin..=treble_max_bin.min(bands.len() - 1)] {
                    current_treble += val;
                    t_count += 1;
                }
                if t_count > 0 {
                    current_treble /= t_count as f32;
                }
                
                self.treble_moving_average = self.treble_moving_average * 0.90 + current_treble * 0.10;
                
                if current_treble > self.treble_moving_average * 1.3 && current_treble > 0.01
                    && self.last_treble_time.elapsed().as_millis() > 50 { // Fast 50ms cooldown for rapid 16th-note hi-hats
                        self.treble_pulse = 1.0;
                        self.last_treble_time = Instant::now();
                    }
                
                let min_log = min_freq.log2();
                let max_log = max_freq.log2();

                for (i, current) in self.state.audio_bands.iter_mut().enumerate() {
                    let t_lo = i as f32 / target_len as f32;
                    let t_hi = (i + 1) as f32 / target_len as f32;
                    
                    let freq_lo = (min_log + t_lo * (max_log - min_log)).exp2();
                    let freq_hi = (min_log + t_hi * (max_log - min_log)).exp2();
                    
                    let mut bin_lo = (freq_lo / freq_per_bin).round() as usize;
                    let mut bin_hi = (freq_hi / freq_per_bin).round() as usize;
                    
                    bin_lo = bin_lo.clamp(0, bands.len() - 1);
                    bin_hi = bin_hi.clamp(0, bands.len());
                    
                    if bin_hi <= bin_lo {
                        bin_hi = (bin_lo + 1).min(bands.len());
                    }
                    
                    let max_val = bands[bin_lo..bin_hi].iter().fold(0.0f32, |acc, &val| acc.max(val));
                    
                    // Calculate the geometric mean frequency of the current band
                    let f = (freq_lo * freq_hi).sqrt();
                    let f2 = f * f;
                    let f4 = f2 * f2;
                    
                    // A-Weighting filter curve formula
                    let a_weighting = (12200.0 * 12200.0 * f4) / 
                        ((f2 + 20.6 * 20.6) * 
                         (f2 + 12200.0 * 12200.0) * 
                         ((f2 + 107.7 * 107.7) * (f2 + 737.9 * 737.9)).sqrt());
                         
                    // Normalize to 1.0 at 1 kHz and apply a visual scalar to keep bars punchy
                    let a_weighting_norm = a_weighting * 1.2589;
                    let target = (max_val * a_weighting_norm * 2.5).clamp(0.0, 1.0);
                    
                    if target > *current {
                        *current = *current * 0.2 + target * 0.8;
                    } else {
                        *current = *current * smoothing + target * (1.0 - smoothing);
                    }
                }
                
                if self.state.audio_waveform.len() != target_len {
                    self.state.audio_waveform = vec![0.0; target_len];
                }
                
                let chunk_size = waveform.len() as f32 / target_len as f32;
                for (i, current) in self.state.audio_waveform.iter_mut().enumerate() {
                    let start = (i as f32 * chunk_size) as usize;
                    let end = ((i + 1) as f32 * chunk_size) as usize;
                    
                    let peak = waveform[start..end.min(waveform.len())]
                        .iter()
                        .fold(0.0f32, |max, &val| if val.abs() > max.abs() { val } else { max });

                    *current = *current * smoothing + peak * (1.0 - smoothing);
                }
            }

            Event::WeatherUpdated(weather) => {
                info!(
                    "Weather: {:?} {:.1}°C",
                    weather.condition, weather.temperature_celsius
                );
                self.state.weather = Some(weather);
                self.update_weather_string();
                self.state.begin_transition();
            }
        }
    }

    fn draw_frame(&mut self, wayland_manager: &mut WaylandManager, delta: f32) -> Result<()> {
        let _scene = self.state.scene_description();
        
        let audio_data = match self.state.config.audio.style.as_str() {
            "waveform" => &self.state.audio_waveform,
            _ => &self.state.audio_bands,
        };
        
        let force_weather = self.state.config.mode == super::config::WallpaperMode::Weather;
        let force_vis = self.state.config.mode == super::config::WallpaperMode::AudioVisualiser;
        let force_art = self.state.config.mode == super::config::WallpaperMode::AlbumArt;

        let max_energy = audio_data.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
        let has_audio = (max_energy > 0.001 || force_vis) && !force_weather && !force_art;
        
        let base_energy = if self.state.audio_bands.is_empty() { 0.0 } else {
            (self.state.audio_bands.iter().sum::<f32>() / self.state.audio_bands.len() as f32) * 5.0
        };
        // Combine the base volume energy with our snappy treble pulse, strictly capped to prevent blown out flashing
        let audio_energy = (base_energy * 0.3 + self.treble_pulse * 0.4).clamp(0.0, 1.0);
        let has_art = (self.state.current_track.as_ref().and_then(|t| t.album_art.as_ref()).is_some() || force_art) && !force_weather && !force_vis;
        
        let clear_colour = self.get_clear_colour();
        // Use our new smart audio-reactive beat detector instead of the generic timer
        let pulse = self.beat_pulse;
        let (prev_lyric, current_lyric, next_lyric) = self.state.active_lyrics();

        // --- Dispatch Weather Compute Shader ---
        // We run this once per frame, updating the positions of all 10,000 particles
        let mut wind_x = 0.1f32;
        let mut gravity = 0.5f32;
        
        if let Some(weather) = &self.state.weather {
            use super::event::WeatherCondition;
            match weather.condition {
                WeatherCondition::Rain | WeatherCondition::Thunderstorm => {
                    gravity = 1.2; // Rain falls fast
                    wind_x = 0.2;
                }
                WeatherCondition::Snow => {
                    gravity = 0.2; // Snow drifts slowly
                    wind_x = 0.5;
                }
                _ => {}
            }
        }
        
        let compute_uniforms = [delta, wind_x, gravity, 0.0f32];
        let compute_bytes = unsafe { std::slice::from_raw_parts(compute_uniforms.as_ptr() as *const u8, 16) };
        self.queue.write_buffer(&self.weather_compute_uniform_buffer, 0, compute_bytes);

        let mut compute_encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder"),
        });
        {
            let mut compute_pass = compute_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Weather Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.weather_compute_pipeline);
            compute_pass.set_bind_group(0, &self.weather_compute_bind_group, &[]);
            compute_pass.dispatch_workgroups(157, 1, 1); // 10000 particles / 64 threads = ~157 workgroups
        }
        self.queue.submit(std::iter::once(compute_encoder.finish()));

        if has_audio {
            let bands_bytes = unsafe {
                std::slice::from_raw_parts(
                    audio_data.as_ptr() as *const u8,
                    audio_data.len() * std::mem::size_of::<f32>(),
                )
            };
            self.queue.write_buffer(&self.visualiser_pass.bands_buffer, 0, bands_bytes);
        }

        for (i, gpu_out) in self.outputs.iter_mut().enumerate() {
            if wayland_manager.is_frame_pending(i) {
                continue; // The compositor hasn't shown the last frame yet (e.g., hidden behind a window)
            }

            let output = match gpu_out.surface.get_current_texture() {
                Ok(texture) => texture,
                Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                    warn!("wgpu surface outdated or lost, reconfiguring...");
                    gpu_out.surface.configure(&self.device, &gpu_out.config);
                    continue;
                }
                Err(wgpu::SurfaceError::Timeout) => {
                    warn!("wgpu surface timeout, skipping frame...");
                    continue;
                }
                Err(e) => anyhow::bail!("Failed to get current texture: {:?}", e),
            };

            wayland_manager.mark_frame_rendered(i); // Request the next frame callback

            // 1. Process visualizer uniforms
            if has_audio {
                let get_colors = |palette: Option<&[[f32; 3]]>| -> ([f32; 3], [f32; 3]) {
                    let top = self.theme.visualiser.color_top.or(self.state.config.audio.color_top);
                    let bottom = self.theme.visualiser.color_bottom.or(self.state.config.audio.color_bottom);

                    match palette {
                        _ if top.is_some() && bottom.is_some() => (top.unwrap(), bottom.unwrap()),
                        Some(p) if p.len() >= 2 => (top.unwrap_or(p[0]), bottom.unwrap_or(p[1])),
                        Some(p) if p.len() == 1 => (top.unwrap_or(p[0]), bottom.unwrap_or([p[0][0] * 0.5, p[0][1] * 0.5, p[0][2] * 0.5])),
                        _ => (top.unwrap_or([1.0, 0.2, 0.5]), bottom.unwrap_or([0.2, 0.5, 1.0])),
                    }
                };
                let target_colors = get_colors(self.state.current_track.as_ref().and_then(|t| t.palette.as_deref()));
                let (top_col, bottom_col) = if self.state.transition_progress < 1.0 {
                    let prev_colors = get_colors(self.state.previous_palette.as_deref());
                    let t = self.state.transition_progress;
                    let top_rgb = lerp_colour(prev_colors.0, target_colors.0, t);
                    let bottom_rgb = lerp_colour(prev_colors.1, target_colors.1, t);
                    ([top_rgb[0], top_rgb[1], top_rgb[2], 1.0], [bottom_rgb[0], bottom_rgb[1], bottom_rgb[2], 1.0])
                } else {
                    let top_rgb = target_colors.0;
                    let bottom_rgb = target_colors.1;
                    ([top_rgb[0], top_rgb[1], top_rgb[2], 1.0], [bottom_rgb[0], bottom_rgb[1], bottom_rgb[2], 1.0])
                };

                #[repr(C)]
                struct VisUniforms { 
                    res: [f32; 2], 
                    bands: u32, 
                    pulse: f32, 
                    top: [f32; 4], 
                    bottom: [f32; 4],
                    style: u32,
                    size: f32,
                    position: [f32; 2],
                    rotation: f32,
                    amplitude: f32,
                    padding: [u32; 2], 
                }
                let shape_u32 = match self.theme.visualiser.shape {
                    super::config::VisShape::Circular => 0,
                    super::config::VisShape::Linear => 1,
                };
                let vis_uniforms = VisUniforms {
                    res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                    bands: self.state.config.audio.bands as u32,
                    pulse,
                    top: top_col,
                    bottom: bottom_col,
                    style: shape_u32,
                    size: self.theme.visualiser.size,
                    position: self.theme.visualiser.position,
                    rotation: self.theme.visualiser.rotation.to_radians(),
                    amplitude: self.theme.visualiser.amplitude,
                    padding: [0; 2],
                };
                let vis_bytes = unsafe { std::slice::from_raw_parts(&vis_uniforms as *const _ as *const u8, std::mem::size_of::<VisUniforms>()) };
                self.queue.write_buffer(&self.visualiser_pass.uniform_buffer, 0, vis_bytes);
            }

            // 2. Process album art uniforms
            if has_art {
                if let Some(track) = &self.state.current_track {
                    let target_color = track.palette.as_deref().and_then(|p| p.first()).copied().unwrap_or([0.1, 0.1, 0.1]);
                    let color = if self.state.transition_progress < 1.0 {
                        let prev_color = self.state.previous_palette.as_deref().and_then(|p| p.first()).copied().unwrap_or([0.1, 0.1, 0.1]);
                        lerp_colour(prev_color, target_color, self.state.transition_progress)
                    } else { target_color };

                    let bg_mode = if self.state.config.appearance.disable_blur { 2 } else { 0 };
                    let bg_alpha_val = 1.0 - self.state.transparent_fade;
                    
                    #[repr(C)]
                    struct ArtUniforms { 
                        color_and_transition: [f32; 4], 
                        res: [f32; 2], 
                        art_position: [f32; 2],
                        audio_energy: f32, 
                        mode: u32,
                        bg_alpha: f32,
                        art_size: f32,
                        shape: u32,
                        padding: [u32; 3],
                    }
                    
                    let bg_uniforms = ArtUniforms {
                        color_and_transition: [color[0], color[1], color[2], self.state.transition_progress],
                        res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                        art_position: self.theme.album_art.position,
                        audio_energy,
                        mode: bg_mode,
                        bg_alpha: bg_alpha_val,
                        art_size: self.theme.album_art.size,
                        shape: match self.theme.album_art.shape {
                            super::config::ArtShape::Square => 0,
                            super::config::ArtShape::Circular => 1,
                        },
                        padding: [0; 3],
                    };
                    let bg_bytes = unsafe { std::slice::from_raw_parts(&bg_uniforms as *const _ as *const u8, std::mem::size_of::<ArtUniforms>()) };
                    self.queue.write_buffer(&self.album_art_bg_uniform_buffer, 0, bg_bytes);

                    let fg_uniforms = ArtUniforms {
                        color_and_transition: [color[0], color[1], color[2], self.state.transition_progress],
                        res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                        art_position: self.theme.album_art.position,
                        audio_energy,
                        mode: 1,
                        bg_alpha: 1.0, // The sharp foreground art never fades!
                        art_size: self.theme.album_art.size,
                    shape: match self.theme.album_art.shape {
                        super::config::ArtShape::Square => 0,
                        super::config::ArtShape::Circular => 1,
                    },
                    padding: [0; 3],
                    };
                    let fg_bytes = unsafe { std::slice::from_raw_parts(&fg_uniforms as *const _ as *const u8, std::mem::size_of::<ArtUniforms>()) };
                    self.queue.write_buffer(&self.album_art_fg_uniform_buffer, 0, fg_bytes);
                }
            }

            if self.custom_bg_bind_group.is_some() {
                // 4. Process custom background uniforms
                let bg_mode = if self.state.config.appearance.disable_blur { 2 } else { 0 };
                let bg_alpha_val = 1.0 - self.state.transparent_fade;
                
                #[repr(C)]
                struct ArtUniforms { 
                    color_and_transition: [f32; 4], 
                    res: [f32; 2], 
                    art_position: [f32; 2],
                    audio_energy: f32, 
                    mode: u32,
                    bg_alpha: f32,
                    art_size: f32,
                    shape: u32,
                    padding: [u32; 3],
                }
                
                let target_color = self.state.current_track.as_ref().and_then(|t| t.palette.as_deref()).and_then(|p| p.first()).copied().unwrap_or([0.1, 0.1, 0.1]);
                
                let custom_bg_uniforms = ArtUniforms {
                    color_and_transition: [target_color[0], target_color[1], target_color[2], self.state.transition_progress],
                    res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                    art_position: self.theme.album_art.position,
                    audio_energy,
                    mode: bg_mode,
                    bg_alpha: bg_alpha_val, 
                    art_size: self.theme.album_art.size,
                    shape: match self.theme.album_art.shape {
                        super::config::ArtShape::Square => 0,
                        super::config::ArtShape::Circular => 1,
                    },
                    padding: [0; 3],
                };
                let cbg_bytes = unsafe { std::slice::from_raw_parts(&custom_bg_uniforms as *const _ as *const u8, std::mem::size_of::<ArtUniforms>()) };
                self.queue.write_buffer(&self.custom_bg_uniform_buffer, 0, cbg_bytes);
            } else {
                // 3. Process ambient uniforms
                let elapsed = self.start_time.elapsed().as_secs_f32();
                let mut weather_type = 0u32;
                let sky = time_to_sky_colour(self.state.time_of_day);
                let final_sky = if let Some(weather) = &self.state.weather {
                    use super::event::WeatherCondition;
                    weather_type = match weather.condition {
                        WeatherCondition::Clear | WeatherCondition::PartlyCloudy => 0,
                        WeatherCondition::Cloudy | WeatherCondition::Fog => 1,
                        WeatherCondition::Rain | WeatherCondition::Thunderstorm => 2,
                        WeatherCondition::Snow => 3,
                    };
                    match weather.condition {
                        WeatherCondition::Rain | WeatherCondition::Thunderstorm => lerp_colour(sky, [0.2, 0.2, 0.25], 0.6),
                        WeatherCondition::Snow => lerp_colour(sky, [0.8, 0.85, 0.9], 0.4),
                        _ => sky,
                    }
                } else { sky };
                
                let bg_alpha_val = 1.0 - self.state.transparent_fade;

                #[repr(C)]
                struct AmbUniforms { 
                    res: [f32; 2], 
                    time: f32, 
                    weather: u32, 
                    sky: [f32; 4],
                    bg_alpha: f32,
                    padding: [u32; 7],
                }
                let amb_uniforms = AmbUniforms {
                    res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                    time: elapsed, weather: weather_type, sky: [final_sky[0], final_sky[1], final_sky[2], 1.0],
                    bg_alpha: bg_alpha_val,
                    padding: [0; 7],
                };
                let amb_bytes = unsafe { std::slice::from_raw_parts(&amb_uniforms as *const _ as *const u8, std::mem::size_of::<AmbUniforms>()) };
                self.queue.write_buffer(&self.ambient_uniform_buffer, 0, amb_bytes);
            }

            // --- Construct Text Layouts ---
            let mut owned_sections = Vec::new();

            let width_f = gpu_out.config.width as f32;
            let height_f = gpu_out.config.height as f32;
            
            let map_align = |a: &super::config::TextAlign| match a {
                super::config::TextAlign::Left => HorizontalAlign::Left,
                super::config::TextAlign::Center => HorizontalAlign::Center,
                super::config::TextAlign::Right => HorizontalAlign::Right,
            };
            
            if self.state.config.audio.show_lyrics {
                let base_font_size = (height_f * 0.04).clamp(16.0, 48.0);
                let active_font_size = base_font_size * 1.5;
                let line_spacing = active_font_size * 1.2;
                
                let lx = width_f * self.theme.lyrics.position[0];
                let ly = height_f * self.theme.lyrics.position[1];
                let l_align = map_align(&self.theme.lyrics.align);

                if let Some(text) = prev_lyric {
                    owned_sections.push(Section::default().add_text(Text::new(text).with_scale(base_font_size).with_color([1.0, 1.0, 1.0, 0.35]))
                        .with_screen_position((lx, ly - line_spacing)).with_layout(Layout::default().h_align(l_align)));
                }

                if let Some(text) = current_lyric {
                    let scale = active_font_size + self.lyric_bounce_value * 8.0;
                    let active_y = ly - (self.lyric_bounce_value * 12.0);
                    owned_sections.push(Section::default().add_text(Text::new(text).with_scale(scale).with_color([1.0, 1.0, 1.0, 1.0]))
                        .with_screen_position((lx, active_y)).with_layout(Layout::default().h_align(l_align)));
                }

                if let Some(text) = next_lyric {
                    owned_sections.push(Section::default().add_text(Text::new(text).with_scale(base_font_size).with_color([1.0, 1.0, 1.0, 0.35]))
                        .with_screen_position((lx, ly + line_spacing)).with_layout(Layout::default().h_align(l_align)));
                }
            }

            if self.state.current_track.is_some() {
                let info_scale = (height_f * 0.025).clamp(16.0, 36.0);
                let tx = width_f * self.theme.track_info.position[0];
                let ty = height_f * self.theme.track_info.position[1];
                let t_align = map_align(&self.theme.track_info.align);
                owned_sections.push(Section::default().add_text(Text::new(&self.cached_track_str).with_scale(info_scale).with_color([0.8, 0.8, 0.8, 0.8]))
                    .with_screen_position((tx, ty)).with_layout(Layout::default().h_align(t_align)));
            }
            
            // Optionally draw weather to the top right corner
            if self.state.weather.is_some() {
                let weather_scale = (height_f * 0.02).clamp(14.0, 24.0);
                let wx = width_f * self.theme.weather.position[0];
                let wy = height_f * self.theme.weather.position[1];
                let w_align = map_align(&self.theme.weather.align);
                owned_sections.push(Section::default().add_text(Text::new(&self.cached_weather_str).with_scale(weather_scale).with_color([0.9, 0.9, 0.9, 0.8]))
                    .with_screen_position((wx, wy))
                    .with_layout(Layout::default().h_align(w_align)));
            }

            let text_sections: Vec<&Section> = owned_sections.iter().collect();
            gpu_out.text_brush.queue(&self.device, &self.queue, text_sections).unwrap();

            let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Encoder"),
            });

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Main Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear_colour),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                // 0. Custom Desktop Wallpaper Background
                if let Some(bind_group) = &self.custom_bg_bind_group {
                    if self.state.transparent_fade < 1.0 {
                        render_pass.set_pipeline(&self.album_art_pipeline);
                        render_pass.set_bind_group(0, bind_group, &[]);
                        render_pass.draw(0..3, 0..1);
                    }
                } else {
                    // 1. ALWAYS Draw Ambient Weather Background
                    if self.state.transparent_fade < 1.0 {
                        render_pass.set_pipeline(&self.ambient_pipeline);
                        render_pass.set_bind_group(0, &self.ambient_bind_group, &[]);
                        render_pass.draw(0..3, 0..1);
                    }
                }

                // 1.5 Overlay Weather Particles (Rain / Snow)
                let is_weather_active = self.state.weather.as_ref().is_some_and(|w| {
                    use super::event::WeatherCondition;
                    matches!(w.condition, WeatherCondition::Rain | WeatherCondition::Snow | WeatherCondition::Thunderstorm)
                });
                if is_weather_active && self.state.transparent_fade < 1.0 {
                    render_pass.set_pipeline(&self.weather_render_pipeline);
                    render_pass.set_bind_group(0, &self.weather_render_bind_group, &[]);
                    render_pass.draw(0..6, 0..10000); // 6 vertices per quad, 10000 particles!
                }

                // 2. Overlay Frosted Glass Background (Transparent)
                if has_art && self.state.transparent_fade < 1.0 {
                    if let Some(bind_group) = &self.album_art_bg_bind_group {
                        render_pass.set_pipeline(&self.album_art_pipeline);
                        render_pass.set_bind_group(0, bind_group, &[]);
                        render_pass.draw(0..3, 0..1);
                    }
                }

                // 3. Overlay Visualiser
                if has_audio {
                    render_pass.set_pipeline(&self.visualiser_pass.pipeline);
                    render_pass.set_bind_group(0, &self.visualiser_pass.bind_group, &[]);
                    render_pass.draw(0..3, 0..1);
                }

                // 4. Overlay Foreground Art
                if has_art {
                    if let Some(bind_group) = &self.album_art_fg_bind_group {
                        render_pass.set_pipeline(&self.album_art_pipeline);
                        render_pass.set_bind_group(0, bind_group, &[]);
                        render_pass.draw(0..3, 0..1);
                    }
                }

                gpu_out.text_brush.draw(&mut render_pass);
            }

            self.queue.submit(std::iter::once(encoder.finish()));
            output.present();
        }

        Ok(())
    }

    fn get_clear_colour(&self) -> wgpu::Color {
        if self.state.config.appearance.transparent_background {
            return wgpu::Color::TRANSPARENT;
        }
        
        let scene = match self.state.config.mode {
            super::config::WallpaperMode::Weather => SceneHint::Ambient,
            super::config::WallpaperMode::AlbumArt => SceneHint::AlbumArt,
            super::config::WallpaperMode::AudioVisualiser => SceneHint::AudioVisualiser,
            super::config::WallpaperMode::Auto => self.state.scene_description(),
        };

        match scene {
            SceneHint::Ambient => {
                let sky = time_to_sky_colour(self.state.time_of_day);
                let final_sky = if let Some(weather) = &self.state.weather {
                    use super::event::WeatherCondition;
                    match weather.condition {
                        WeatherCondition::Rain | WeatherCondition::Thunderstorm => {
                            lerp_colour(sky, [0.2, 0.2, 0.25], 0.6)
                        }
                        WeatherCondition::Snow => lerp_colour(sky, [0.8, 0.85, 0.9], 0.4),
                        _ => sky,
                    }
                } else {
                    sky
                };
                wgpu::Color { r: final_sky[0] as f64, g: final_sky[1] as f64, b: final_sky[2] as f64, a: 1.0 }
            }
            SceneHint::AlbumArt => wgpu::Color { r: 0.05, g: 0.05, b: 0.05, a: 1.0 },
            SceneHint::AudioVisualiser => wgpu::Color { r: 0.1, g: 0.1, b: 0.15, a: 1.0 },
        }
    }

    fn update_album_art_texture(&mut self, rgba: &image::RgbaImage) {
        let dimensions = rgba.dimensions();

        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("Album Art Texture"),
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            texture_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.previous_album_view = std::mem::replace(&mut self.current_album_view, view);

        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bg_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.album_art_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.album_art_bg_uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.current_album_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.previous_album_view) },
            ],
            label: Some("Album Art BG Bind Group"),
        });

        let fg_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.album_art_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.album_art_fg_uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.current_album_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.previous_album_view) },
            ],
            label: Some("Album Art FG Bind Group"),
        });

        self.album_art_bg_bind_group = Some(bg_bind_group);
        self.album_art_fg_bind_group = Some(fg_bind_group);
        self.current_album_texture = Some(texture);
    }

    fn update_video_frame(&mut self, rgba: &image::RgbaImage) {
        // Fast-path: If the texture already exists and dimensions match perfectly, 
        // we can copy the raw video frame bytes straight into the GPU's VRAM!
        if let Some(texture) = &self.current_album_texture {
            let dimensions = rgba.dimensions();
            if texture.size().width == dimensions.0 && texture.size().height == dimensions.1 {
                self.queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    rgba,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * dimensions.0),
                        rows_per_image: Some(dimensions.1),
                    },
                    texture.size(),
                );
            }
        }
    }

    pub fn load_custom_background(&mut self, path: Option<&str>) {
        let Some(path) = path else {
            self.custom_bg_bind_group = None;
            return;
        };

        info!("Loading custom background from {}", path);
        let img = match image::open(path) {
            Ok(i) => i.to_rgba8(),
            Err(e) => {
                warn!("Failed to load custom background: {}", e);
                self.custom_bg_bind_group = None;
                return;
            }
        };
        
        let dimensions = img.dimensions();
        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("Custom Background Texture"),
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            texture_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        self.custom_bg_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.album_art_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.custom_bg_uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&view) }, // No previous view needed for static desktop bg
            ],
            label: Some("Custom Background Bind Group"),
        }));
    }

    fn build_a_weighting_curve(band_count: usize) -> Vec<f32> {
        let mut curve = Vec::with_capacity(band_count);
        let min_freq = 40.0f32;
        let max_freq = 16000.0f32;
        let min_log = min_freq.log2();
        let max_log = max_freq.log2();

        for i in 0..band_count {
            let t_lo = i as f32 / band_count as f32;
            let t_hi = (i + 1) as f32 / band_count as f32;

            let freq_lo = (min_log + t_lo * (max_log - min_log)).exp2();
            let freq_hi = (min_log + t_hi * (max_log - min_log)).exp2();

            let f = (freq_lo * freq_hi).sqrt();
            let f2 = f * f;
            let f4 = f2 * f2;

            let a_weighting = (12200.0 * 12200.0 * f4) /
                ((f2 + 20.6 * 20.6) *
                 (f2 + 12200.0 * 12200.0) *
                 ((f2 + 107.7 * 107.7) * (f2 + 737.9 * 737.9)).sqrt());

            curve.push(a_weighting * 1.2589);
        }
        curve
    }

    fn build_frequency_bin_ranges(band_count: usize) -> Vec<(usize, usize)> {
        let mut ranges = Vec::with_capacity(band_count);
        let min_freq = 40.0f32;
        let max_freq = 16000.0f32;
        let sample_rate = 48000.0f32;
        let fft_size = 2048.0f32;
        let freq_per_bin = sample_rate / fft_size;
        let min_log = min_freq.log2();
        let max_log = max_freq.log2();
        let max_bins = (fft_size / 2.0) as usize; // 1024

        for i in 0..band_count {
            let t_lo = i as f32 / band_count as f32;
            let t_hi = (i + 1) as f32 / band_count as f32;

            let freq_lo = (min_log + t_lo * (max_log - min_log)).exp2();
            let freq_hi = (min_log + t_hi * (max_log - min_log)).exp2();

            let mut bin_lo = (freq_lo / freq_per_bin).round() as usize;
            let mut bin_hi = (freq_hi / freq_per_bin).round() as usize;

            bin_lo = bin_lo.clamp(0, max_bins.saturating_sub(1));
            bin_hi = bin_hi.clamp(0, max_bins);
            if bin_hi <= bin_lo { bin_hi = (bin_lo + 1).min(max_bins); }
            ranges.push((bin_lo, bin_hi));
        }
        ranges
    }

    fn build_waveform_bin_ranges(band_count: usize) -> Vec<(usize, usize)> {
        let chunk_size = 2048.0 / band_count as f32;
        (0..band_count).map(|i| {
            let start = (i as f32 * chunk_size) as usize;
            let end = ((i + 1) as f32 * chunk_size) as usize;
            (start, end.min(2048))
        }).collect()
    }

    fn update_weather_string(&mut self) {
        if let Some(weather) = &self.state.weather {
            let mut val = weather.temperature_celsius;
            let mut unit = "C";
            if self.state.config.weather.temperature_unit == super::config::TemperatureUnit::Fahrenheit {
                val = (val * 9.0 / 5.0) + 32.0;
                unit = "F";
            }
            self.cached_weather_str = format!("{:?} {:.1}°{}", weather.condition, val, unit);
        } else {
            self.cached_weather_str.clear();
        }
    }
}
