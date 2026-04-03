use anyhow::Result;
use cosmic_text::{self, Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Receiver;
use tracing::{info, warn};

use crate::modules::colour::{lerp_colour, time_to_sky_colour};
use crate::modules::event::Event;
use crate::modules::state::{AppState, SceneHint};
use crate::modules::visualiser_pass::VisualiserPass;
use crate::modules::wayland::WaylandManager;

pub const GLYPH_CACHE_WIDTH: u32 = 2048;
pub const GLYPH_CACHE_HEIGHT: u32 = 2048;
use super::text::*;
use super::types::*;

use crate::modules::config::{
    ArtShape, TemperatureUnit, TextAlign, ThemeLayout, VisAlign, VisShape, WallpaperMode,
};
use crate::modules::event::WeatherCondition;
pub struct GpuOutput {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
}

pub struct Renderer {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    outputs: Vec<GpuOutput>,
    font_system: FontSystem,
    swash_cache: SwashCache,
    text_renderer: TextRenderer,
    text_buffer_cache: std::collections::HashMap<String, Buffer>,
    visualiser_pass: VisualiserPass,
    album_art_pipeline: wgpu::RenderPipeline,
    album_art_layout: wgpu::BindGroupLayout,
    album_art_bg_uniform_buffer: wgpu::Buffer,
    album_art_fg_uniform_buffer: wgpu::Buffer,
    album_art_bg_bind_group: Option<wgpu::BindGroup>,
    album_art_fg_bind_group: Option<wgpu::BindGroup>,
    current_album_texture: Option<wgpu::Texture>,
    album_art_sampler: wgpu::Sampler,
    ambient_pipeline: wgpu::RenderPipeline,
    ambient_bind_group: wgpu::BindGroup,
    ambient_uniform_buffer: wgpu::Buffer,
    custom_bg_uniform_buffer: wgpu::Buffer,
    custom_bg_bind_group: Option<wgpu::BindGroup>,
    current_bg_path: Option<String>,
    current_custom_bg_size: Option<(u32, u32)>,
    _particle_buffer: wgpu::Buffer,
    weather_compute_uniform_buffer: wgpu::Buffer,
    weather_compute_bind_group: wgpu::BindGroup,
    weather_compute_pipeline: wgpu::ComputePipeline,
    weather_render_bind_group: wgpu::BindGroup,
    weather_render_pipeline: wgpu::RenderPipeline,
    start_time: Instant,
    state: AppState,
    frame_duration: Duration,
    current_fps: u32,
    show_lyrics_atomic: std::sync::Arc<std::sync::atomic::AtomicBool>,
    bass_moving_average: f32,
    beat_pulse: f32,
    last_beat_time: Instant,
    treble_moving_average: f32,
    treble_pulse: f32,
    last_treble_time: Instant,
    theme: ThemeLayout,
    a_weighting_curve: Vec<f32>,
    frequency_bin_ranges: Vec<(usize, usize)>,
    waveform_bin_ranges: Vec<(usize, usize)>,
    lyric_bounce_value: f32,
    lyric_bounce_velocity: f32,
    cached_track_str: String,
    cached_weather_str: String,
    current_lyric_idx: usize,
    lyric_scroll_offset: f32,
    video_frame_buffer: Vec<u8>,
    album_art_pad_buffer: Vec<u8>,
}

impl Renderer {
    fn create_album_art_pipeline(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config_format: wgpu::TextureFormat,
    ) -> (
        wgpu::RenderPipeline,
        wgpu::BindGroupLayout,
        wgpu::Buffer,
        wgpu::Buffer,
        wgpu::Texture,
        wgpu::Sampler,
    ) {
        // --- Album Art Pipeline Setup ---
        let empty_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
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
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

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
                        min_binding_size: wgpu::BufferSize::new(64),
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
            ],
        });

        let album_art_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Album Art Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../album_art.wgsl").into()),
        });

        let album_art_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
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

        let album_art_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        (
            album_art_pipeline,
            album_art_layout,
            album_art_bg_uniform_buffer,
            album_art_fg_uniform_buffer,
            empty_texture,
            album_art_sampler,
        )
    }

    fn create_ambient_pipeline(
        device: &wgpu::Device,
        config_format: wgpu::TextureFormat,
    ) -> (
        wgpu::RenderPipeline,
        wgpu::BindGroup,
        wgpu::Buffer,
        wgpu::Buffer,
    ) {
        // --- Ambient Pipeline Setup ---
        let ambient_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Ambient Uniform Buffer"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let ambient_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Ambient Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(64),
                    },
                    count: None,
                }],
            });

        let ambient_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ambient Bind Group"),
            layout: &ambient_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ambient_uniform_buffer.as_entire_binding(),
            }],
        });

        let ambient_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Ambient Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../ambient.wgsl").into()),
        });

        let ambient_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
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

        (
            ambient_pipeline,
            ambient_bind_group,
            ambient_uniform_buffer,
            custom_bg_uniform_buffer,
        )
    }

    fn create_weather_pipelines(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config_format: wgpu::TextureFormat,
    ) -> (
        wgpu::Buffer,
        wgpu::Buffer,
        wgpu::BindGroup,
        wgpu::ComputePipeline,
        wgpu::BindGroup,
        wgpu::RenderPipeline,
    ) {
        // --- Weather Compute Pipeline Setup ---
        let max_particles = 2500;
        let mut initial_particles = Vec::with_capacity(max_particles);
        for i in 0..max_particles {
            initial_particles.push(Particle {
                pos: [
                    (i as f32 * 12.9898).sin().fract() * 2.0 - 0.5, // Random X scatter
                    (i as f32 * 7.2345).sin().fract() * 2.4 - 1.2, // Naturally spread across the entire vertical space
                ],
                vel: [0.0, 0.5 + (i as f32 % 5.0) * 0.1], // Base downward velocity
                lifetime: 5.0 + (i as f32 % 5.0),
                scale: 1.0,
            });
        }

        let particle_buffer_size =
            (initial_particles.len() * std::mem::size_of::<Particle>()) as wgpu::BufferAddress;
        let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Storage Buffer"),
            size: particle_buffer_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let particles_bytes = unsafe {
            std::slice::from_raw_parts(
                initial_particles.as_ptr() as *const u8,
                particle_buffer_size as usize,
            )
        };
        queue.write_buffer(&particle_buffer, 0, particles_bytes);

        let weather_compute_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Weather Compute Uniform Buffer"),
            size: 16, // delta_time, wind_x, gravity, padding
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let weather_compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Weather Compute Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        // Storage Buffer (Read/Write)
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(particle_buffer_size),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        // Uniform Buffer (delta_time, physics)
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
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: particle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: weather_compute_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let weather_compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Weather Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../weather_compute.wgsl").into()),
        });

        let weather_compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Weather Compute Pipeline Layout"),
                bind_group_layouts: &[&weather_compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        let weather_compute_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Weather Compute Pipeline"),
                layout: Some(&weather_compute_pipeline_layout),
                module: &weather_compute_shader,
                entry_point: "main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            });

        // --- Weather Render Pipeline Setup ---
        let weather_render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Weather Render Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(particle_buffer_size),
                    },
                    count: None,
                }],
            });

        let weather_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Weather Render Bind Group"),
            layout: &weather_render_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: particle_buffer.as_entire_binding(),
            }],
        });

        let weather_render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Weather Render Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../weather_render.wgsl").into()),
        });

        let weather_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Weather Render Pipeline Layout"),
                bind_group_layouts: &[&weather_render_bind_group_layout],
                push_constant_ranges: &[],
            });

        let weather_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Weather Render Pipeline"),
                layout: Some(&weather_render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &weather_render_shader,
                    entry_point: "vs_main",
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &weather_render_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        (
            particle_buffer,
            weather_compute_uniform_buffer,
            weather_compute_bind_group,
            weather_compute_pipeline,
            weather_render_bind_group,
            weather_render_pipeline,
        )
    }

    pub async fn new(
        wayland_manager: &WaylandManager,
        state: AppState,
        show_lyrics_atomic: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self> {
        let fps = state.config.fps;
        let current_fps = fps;

        info!("Initialising wgpu renderer...");

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });

        let outputs_info: Vec<_> = wayland_manager.outputs().collect();
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

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surfaces[0]),
                force_fallback_adapter: false,
            })
            .await
            .expect("No suitable GPU adapter found");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("COSMIC Wallpaper Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let mut outputs = Vec::new();
        for (info, surface) in outputs_info.into_iter().zip(surfaces) {
            let caps: wgpu::SurfaceCapabilities = surface.get_capabilities(&adapter);
            let format = caps
                .formats
                .iter()
                .copied()
                .find(|f: &wgpu::TextureFormat| f.is_srgb())
                .unwrap_or(caps.formats[0]);

            let alpha_mode = if caps
                .alpha_modes
                .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
            {
                wgpu::CompositeAlphaMode::PreMultiplied
            } else if caps
                .alpha_modes
                .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
            {
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
                desired_maximum_frame_latency: 1, // Enforce Double Buffering to save ~33MB+ VRAM per monitor
            };
            surface.configure(&device, &config);

            outputs.push(GpuOutput { surface, config });
        }

        let config_format = outputs[0].config.format;

        // --- Visualiser Pipeline Setup ---
        let visualiser_pass = VisualiserPass::new(
            &device,
            config_format,
            state.config.audio.bands,
            &state.config.audio.style,
        )
        .await?;

        // --- Text Rendering Setup ---
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let text_renderer = TextRenderer::new(&device, config_format)?;

        let (
            album_art_pipeline,
            album_art_layout,
            album_art_bg_uniform_buffer,
            album_art_fg_uniform_buffer,
            empty_texture,
            album_art_sampler,
        ) = Self::create_album_art_pipeline(&device, &queue, config_format);
        let (
            ambient_pipeline,
            ambient_bind_group,
            ambient_uniform_buffer,
            custom_bg_uniform_buffer,
        ) = Self::create_ambient_pipeline(&device, config_format);
        let (
            particle_buffer,
            weather_compute_uniform_buffer,
            weather_compute_bind_group,
            weather_compute_pipeline,
            weather_render_bind_group,
            weather_render_pipeline,
        ) = Self::create_weather_pipelines(&device, &queue, config_format);
        let theme = ThemeLayout::load(&state.config.audio.style);
        let a_weighting_curve = Self::build_a_weighting_curve(state.config.audio.bands);
        let frequency_bin_ranges = Self::build_frequency_bin_ranges(state.config.audio.bands);
        let waveform_bin_ranges = Self::build_waveform_bin_ranges(state.config.audio.bands);

        let mut renderer = Self {
            instance,
            adapter,
            device,
            queue,
            outputs,
            font_system,
            swash_cache,
            text_renderer,
            text_buffer_cache: std::collections::HashMap::new(),
            visualiser_pass,
            album_art_pipeline,
            album_art_layout,
            album_art_bg_uniform_buffer,
            album_art_fg_uniform_buffer,
            album_art_bg_bind_group: None,
            album_art_fg_bind_group: None,
            current_album_texture: Some(empty_texture),
            album_art_sampler,
            ambient_pipeline,
            ambient_bind_group,
            ambient_uniform_buffer,
            custom_bg_uniform_buffer,
            custom_bg_bind_group: None,
            current_bg_path: None,
            current_custom_bg_size: None,
            _particle_buffer: particle_buffer,
            weather_compute_uniform_buffer,
            weather_compute_bind_group,
            weather_compute_pipeline,
            weather_render_bind_group,
            weather_render_pipeline,
            start_time: Instant::now(),
            state,
            frame_duration: Duration::from_secs_f64(1.0 / fps as f64),
            current_fps,
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
            current_lyric_idx: 0,
            lyric_scroll_offset: 0.0,
            video_frame_buffer: Vec::new(),
            album_art_pad_buffer: Vec::new(),
        };

        let path = renderer
            .state
            .config
            .appearance
            .resolved_background_path()
            .await;
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
        let mut last_config_serial = wayland_manager.app_data.configuration_serial;

        let mut interval = tokio::time::interval(self.frame_duration);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        wayland_manager.update_opaque_regions(self.state.config.appearance.transparent_background);

        loop {
            // --- Dynamic FPS ---
            let target_fps = self.state.config.fps;

            if self.current_fps != target_fps {
                info!("Updating FPS from {} to {}", self.current_fps, target_fps);
                self.current_fps = target_fps;
                self.frame_duration = Duration::from_secs_f64(1.0 / target_fps as f64);
                interval = tokio::time::interval(self.frame_duration);
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            }

            interval.tick().await;

            let occluded = wayland_manager.is_occluded();
            is_visible.store(!occluded, std::sync::atomic::Ordering::Relaxed);

            wayland_manager.dispatch_events()?;

            let current_outputs: Vec<_> = wayland_manager.outputs().collect();
            if wayland_manager.app_data.configuration_serial != last_config_serial {
                last_config_serial = wayland_manager.app_data.configuration_serial;
                info!(
                    "Monitor configuration changed ({} outputs), rebuilding GPU surfaces...",
                    current_outputs.len()
                );

                self.outputs.clear();
                wayland_manager.cleanup_dead_windows();

                for info in &current_outputs {
                    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: info.raw_display_handle(),
                        raw_window_handle: info.raw_window_handle(),
                    };
                    let surface = unsafe { self.instance.create_surface_unsafe(target) }
                        .expect("Failed to recreate surface");

                    let caps = surface.get_capabilities(&self.adapter);
                    let format = caps
                        .formats
                        .iter()
                        .copied()
                        .find(|f: &wgpu::TextureFormat| f.is_srgb())
                        .unwrap_or(caps.formats[0]);

                    let alpha_mode = if caps
                        .alpha_modes
                        .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
                    {
                        wgpu::CompositeAlphaMode::PreMultiplied
                    } else if caps
                        .alpha_modes
                        .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
                    {
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
                        desired_maximum_frame_latency: 1,
                    };
                    surface.configure(&self.device, &config);

                    self.outputs.push(GpuOutput { surface, config });
                }
            }

            for (i, win) in wayland_manager.app_data.windows.iter().enumerate() {
                if let Some(gpu_out) = self.outputs.get_mut(i) {
                    let target_width = win.width * (win.scale_factor as u32);
                    let target_height = win.height * (win.scale_factor as u32);
                    if gpu_out.config.width != target_width
                        || gpu_out.config.height != target_height
                    {
                        info!(
                            "Resizing output {} to {}x{}",
                            i, target_width, target_height
                        );
                        gpu_out.config.width = target_width.max(1);
                        gpu_out.config.height = target_height.max(1);
                    }
                }
            }

            let mut transparent_changed = false;

            while let Ok(event) = event_rx.try_recv() {
                if let Event::ConfigUpdated(ref config) = event {
                    if config.appearance.transparent_background
                        != self.state.config.appearance.transparent_background
                    {
                        transparent_changed = true;
                    }
                }
                self.handle_event(event).await;
            }

            if transparent_changed {
                wayland_manager
                    .update_opaque_regions(self.state.config.appearance.transparent_background);
            }

            self.state.update_time();

            let now = Instant::now();
            // Cap the delta to 100ms to prevent the Explicit Euler physics from exploding after a monitor sleep!
            let delta = now.duration_since(last_frame).as_secs_f32().min(0.1);
            self.state.tick_transition(delta);
            last_frame = now;

            // Exponential decay for the beat pulse so it snaps up and softly falls down
            self.beat_pulse *= (-12.0 * delta).exp();
            // Treble decays slightly faster for snappier, rapid hi-hats
            self.treble_pulse *= (-15.0 * delta).exp();

            // Spring physics for organic lyric bounce (Hooke's Law)
            let stiffness = self.theme.effects.lyric_spring_stiffness;
            let damping = self.theme.effects.lyric_spring_damping;
            let spring_force =
                -stiffness * self.lyric_bounce_value - damping * self.lyric_bounce_velocity;
            self.lyric_bounce_velocity += spring_force * delta;
            self.lyric_bounce_value += self.lyric_bounce_velocity * delta;

            let current_idx = self
                .state
                .current_track
                .as_ref()
                .and_then(|t| t.lyrics.as_ref())
                .map(|l| {
                    l.partition_point(|line| {
                        line.start_time_secs <= self.state.playback_position.as_secs_f32()
                    })
                })
                .unwrap_or(0);

            if current_idx != self.current_lyric_idx {
                if (current_idx as isize - self.current_lyric_idx as isize).abs() > 2 {
                    // Prevent massive scroll jumps on track init or seeking
                    self.current_lyric_idx = current_idx;
                    self.lyric_scroll_offset = 0.0;
                } else {
                    self.lyric_scroll_offset += self.current_lyric_idx as f32 - current_idx as f32;
                    self.current_lyric_idx = current_idx;
                }
            }

            // Smoothly interpolate the scroll offset back to 0
            self.lyric_scroll_offset *= (-12.0 * delta).exp();

            if wayland_manager.any_monitor_ready() {
                self.draw_frame(&mut wayland_manager, delta)?;
            }

            // Tell wgpu to process internal garbage collection.
            // If we don't call this when output.present() is skipped (e.g. monitor asleep or occluded),
            // dropped textures and command buffers will queue up indefinitely and cause an OOM crash!
            self.device.poll(wgpu::Maintain::Poll);
        }
    }

    async fn handle_event(&mut self, event: Event) {
        match event {
            Event::ConfigUpdated(config) => {
                self.show_lyrics_atomic.store(
                    config.audio.show_lyrics,
                    std::sync::atomic::Ordering::Relaxed,
                );

                let new_bg = config.appearance.resolved_background_path().await;
                if new_bg != self.current_bg_path {
                    self.load_custom_background(new_bg.as_deref());
                    self.current_bg_path = new_bg.clone();
                }

                if config.audio.bands != self.state.config.audio.bands {
                    self.state.audio_bands = vec![0.0; config.audio.bands].into_boxed_slice();
                    self.state.audio_waveform = vec![0.0; config.audio.bands].into_boxed_slice();
                    self.a_weighting_curve = Self::build_a_weighting_curve(config.audio.bands);
                    self.frequency_bin_ranges =
                        Self::build_frequency_bin_ranges(config.audio.bands);
                    self.waveform_bin_ranges = Self::build_waveform_bin_ranges(config.audio.bands);
                }

                // Always reload the shader pipeline so live WGSL edits apply instantly!
                let format = self.outputs[0].config.format;
                self.visualiser_pass
                    .reload(
                        &self.device,
                        format,
                        &config.audio.style,
                        config.audio.bands,
                    )
                    .await;

                // Always reload the theme layout so live edits to the .toml apply instantly!
                self.theme = ThemeLayout::load(&config.audio.style);
                self.state.config = *config;
                self.update_weather_string();
                info!("Live settings applied!");
            }
            Event::TrackChanged(mut track) => {
                self.text_buffer_cache.clear(); // Free old shaped lyrics from memory!
                self.text_buffer_cache.shrink_to_fit();
                info!("Now playing: {} - {}", track.artist, track.title);
                let has_art = track.album_art.is_some();
                // take() strips the massive image payload out of TrackInfo so we don't hoard it in RAM permanently!
                if let Some(art) = track.album_art.take() {
                    info!(
                        "Track contains album art ({} bytes raw). Sending to GPU...",
                        (art.len() as wgpu::BufferAddress)
                    );
                    self.update_album_art_texture(&art);
                } else {
                    warn!("Track event received, but album_art payload is None!");
                    self.album_art_bg_bind_group = None;
                    self.album_art_fg_bind_group = None;
                    self.current_album_texture = None;
                }
                self.state.has_album_art = has_art;
                self.cached_track_str =
                    format!("{} — {}\n{}", track.title, track.artist, track.album);
                self.state.previous_palette = self
                    .state
                    .current_track
                    .as_ref()
                    .and_then(|t| t.palette.clone().map(|p| p.into_vec()));
                self.state.current_track = Some(*track);
                self.state.is_playing = true;
                self.current_lyric_idx = 0;
                self.lyric_scroll_offset = 0.0;
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
                let buffer = frame.into_raw();
                if let Ok(mut pool) = crate::modules::video::get_frame_pool().lock() {
                    if pool.len() < 3 {
                        pool.push(buffer);
                    }
                }
            }

            Event::PlayerShutDown => {
                self.cached_track_str.clear();
                self.text_buffer_cache.clear();
                self.text_buffer_cache.shrink_to_fit();
                self.state.previous_palette = self
                    .state
                    .current_track
                    .as_ref()
                    .and_then(|t| t.palette.clone().map(|p| p.into_vec()));
                self.album_art_bg_bind_group = None;
                self.album_art_fg_bind_group = None;
                self.current_album_texture = None;
                self.state.has_album_art = false;
                self.state.current_track = None;
                self.state.is_playing = false;
                self.current_lyric_idx = 0;
                self.lyric_scroll_offset = 0.0;
                self.state.begin_transition();

                // Free the padding buffers back to the OS allocator on idle
                self.video_frame_buffer.clear();
                self.video_frame_buffer.shrink_to_fit();
                self.album_art_pad_buffer.clear();
                self.album_art_pad_buffer.shrink_to_fit();

                if let Ok(mut pool) = crate::modules::video::get_frame_pool().lock() {
                    pool.clear();
                    pool.shrink_to_fit();
                }
            }

            Event::PlaybackPosition(pos) => {
                self.state.playback_position = pos;
            }

            Event::AudioFrame { bands, waveform } => {
                let smoothing = self.state.config.audio.smoothing;
                let target_len = self.state.audio_bands.len();

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
                if current_bass > self.bass_moving_average * 1.3
                    && current_bass > 0.005
                    && self.last_beat_time.elapsed().as_millis() > 200
                {
                    // 200ms cooldown prevents double-triggering
                    self.beat_pulse = 1.0;

                    // Add physical velocity to the lyric spring. The harder the bass spike, the bigger the bounce!
                    let spike =
                        (current_bass / self.bass_moving_average.max(0.001)).clamp(1.2, 3.0);
                    self.lyric_bounce_velocity += (15.0 * spike) * self.theme.effects.lyric_bounce;
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

                self.treble_moving_average =
                    self.treble_moving_average * 0.90 + current_treble * 0.10;

                if current_treble > self.treble_moving_average * 1.2
                    && current_treble > 0.002
                    && self.last_treble_time.elapsed().as_millis() > 50
                {
                    // Fast 50ms cooldown for rapid 16th-note hi-hats
                    self.treble_pulse = 1.0;
                    self.last_treble_time = Instant::now();
                }

                let bands_len = bands.len();
                for (i, current) in self.state.audio_bands.iter_mut().enumerate() {
                    let (bin_lo, bin_hi): (usize, usize) = self.frequency_bin_ranges[i];

                    let max_val =
                        bands
                            .get(bin_lo..bin_hi.min(bands_len))
                            .map_or(0.0, |slice: &[f32]| {
                                slice
                                    .iter()
                                    .fold(0.0f32, |acc, &val| if val > acc { val } else { acc })
                            });

                    let a_weighting_norm = self.a_weighting_curve[i];
                    let target = (max_val * a_weighting_norm * 2.5).clamp(0.0, 1.0);

                    if target > *current {
                        *current = *current * 0.2 + target * 0.8;
                    } else {
                        *current = *current * smoothing + target * (1.0 - smoothing);
                    }
                }

                if self.state.audio_waveform.len() != target_len {
                    self.state.audio_waveform = vec![0.0; target_len].into_boxed_slice();
                }

                let wave_len = waveform.len();
                for (i, current) in self.state.audio_waveform.iter_mut().enumerate() {
                    let (start, end): (usize, usize) = self.waveform_bin_ranges[i];

                    let peak =
                        waveform
                            .get(start..end.min(wave_len))
                            .map_or(0.0, |slice: &[f32]| {
                                let mut peak = 0.0f32;
                                let mut peak_abs = 0.0f32;
                                for &val in slice {
                                    let val_abs: f32 = val.abs();
                                    if val_abs > peak_abs {
                                        peak_abs = val_abs;
                                        peak = val;
                                    }
                                }
                                peak
                            });

                    *current = *current * smoothing + peak * (1.0 - smoothing);
                }
            }

            Event::WeatherUpdated(weather) => {
                info!(
                    "Weather: {:?} {:.1}°C",
                    weather.condition, weather.temperature_celsius
                );
                self.state.weather = Some(*weather);
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

        let force_weather = self.state.config.mode == WallpaperMode::Weather;
        let force_vis = self.state.config.mode == WallpaperMode::AudioVisualiser;
        let force_art = self.state.config.mode == WallpaperMode::AlbumArt;

        let is_waveform_style = self.state.config.audio.style == "waveform";

        let max_energy = audio_data
            .iter()
            .fold(0.0f32, |a: f32, &b: &f32| a.max(b.abs()));
        let has_audio = (max_energy > 0.001 || force_vis)
            && !force_weather
            && !force_art
            && (self.state.current_track.is_some() || force_vis);

        let base_energy = if self.state.audio_bands.is_empty() {
            0.0
        } else {
            (self.state.audio_bands.iter().sum::<f32>() / self.state.audio_bands.len() as f32) * 5.0
        };
        // Combine the base volume energy with our snappy treble pulse, strictly capped to prevent blown out flashing
        let audio_energy = (base_energy * 0.3 + self.treble_pulse * 0.4).clamp(0.0, 1.0);

        // --- IMPORTANT FIX ---
        // The old state check can fail due to subtle race conditions.
        // The most robust way to check for media is to see if the GPU resources for it exist.
        let has_media_check_state = self.state.has_album_art;
        let has_media_check_gpu = self.album_art_fg_bind_group.is_some();
        if has_media_check_gpu && !has_media_check_state {
            warn!("Album art visibility check mismatch! State: false, GPU: true. Using GPU state.");
        }

        // Decouple art visibility from force_vis so you can layer the visualizer AND the album art!
        let show_art_fg =
            (has_media_check_gpu || force_art) && self.state.config.appearance.show_album_art;
        let show_art_bg =
            (has_media_check_gpu || force_art) && self.state.config.appearance.album_art_background;
        let show_color_bg = (has_media_check_gpu || force_art)
            && self.state.config.appearance.album_color_background;

        let clear_colour = self.get_clear_colour();
        // Use our new smart audio-reactive beat detector instead of the generic timer
        let pulse = self.beat_pulse;

        let is_weather_active = self.state.weather.as_ref().is_some_and(|w| {
            matches!(
                w.condition,
                WeatherCondition::Rain | WeatherCondition::Snow | WeatherCondition::Thunderstorm
            )
        });

        let active_particles = if let Some(weather) = &self.state.weather {
            match weather.condition {
                WeatherCondition::Rain => 800,
                WeatherCondition::Thunderstorm => 1500,
                WeatherCondition::Snow => 2500,
                _ => 0,
            }
        } else {
            0
        };

        if is_weather_active && active_particles > 0 {
            // --- Dispatch Weather Compute Shader ---
            // Only spend GPU time running particle physics if weather is actually visible!
            let mut wind_x = 0.1f32;
            let mut gravity = 0.5f32;

            if let Some(weather) = &self.state.weather {
                match weather.condition {
                    WeatherCondition::Rain | WeatherCondition::Thunderstorm => {
                        gravity = 0.85; // Slower, more elegant rain
                        wind_x = 0.15;
                    }
                    WeatherCondition::Snow => {
                        gravity = 0.2; // Snow drifts slowly
                        wind_x = 0.5;
                    }
                    _ => {}
                }
            }

            let compute_uniforms = [delta, wind_x, gravity, 0.0f32];
            let compute_bytes =
                unsafe { std::slice::from_raw_parts(compute_uniforms.as_ptr() as *const u8, 16) };
            self.queue
                .write_buffer(&self.weather_compute_uniform_buffer, 0, compute_bytes);

            let mut compute_encoder =
                self.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Compute Encoder"),
                    });
            {
                let mut compute_pass =
                    compute_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("Weather Compute Pass"),
                        timestamp_writes: None,
                    });
                compute_pass.set_pipeline(&self.weather_compute_pipeline);
                compute_pass.set_bind_group(0, &self.weather_compute_bind_group, &[]);
                let workgroups = ((active_particles as f32) / 64.0).ceil() as u32;
                if workgroups > 0 {
                    compute_pass.dispatch_workgroups(workgroups, 1, 1);
                }
            }
            self.queue.submit(std::iter::once(compute_encoder.finish()));
        }

        if has_audio {
            let bands_bytes = unsafe {
                std::slice::from_raw_parts(
                    audio_data.as_ptr() as *const u8,
                    audio_data.len() * std::mem::size_of::<f32>(),
                )
            };
            self.queue
                .write_buffer(&self.visualiser_pass.bands_buffer, 0, bands_bytes);
        }

        // Optimization: Pre-calculate common render values out of the monitor loop
        // to avoid redundant calculations for each output.

        // 1. Pre-calculate Visualizer colors
        let (top_col, bottom_col) = if has_audio {
            let get_colors = |palette: Option<&[[f32; 3]]>| -> ([f32; 3], [f32; 3]) {
                let top = self.theme.visualiser.color_top;
                let bottom = self.theme.visualiser.color_bottom;

                match palette {
                    _ if top.is_some() && bottom.is_some() => (top.unwrap(), bottom.unwrap()),
                    Some(p) if p.len() >= 2 => (top.unwrap_or(p[0]), bottom.unwrap_or(p[1])),
                    Some(p) if p.len() == 1 => (
                        top.unwrap_or(p[0]),
                        bottom.unwrap_or([p[0][0] * 0.5, p[0][1] * 0.5, p[0][2] * 0.5]),
                    ),
                    _ => (
                        top.unwrap_or([1.0, 0.2, 0.5]),
                        bottom.unwrap_or([0.2, 0.5, 1.0]),
                    ),
                }
            };
            let target_colors = get_colors(
                self.state
                    .current_track
                    .as_ref()
                    .and_then(|t| t.palette.as_deref()),
            );
            if self.state.transition_progress < 1.0 {
                let prev_colors = get_colors(self.state.previous_palette.as_deref());
                let t = self.state.transition_progress;
                let top_rgb = lerp_colour(prev_colors.0, target_colors.0, t);
                let bottom_rgb = lerp_colour(prev_colors.1, target_colors.1, t);
                (
                    [top_rgb[0], top_rgb[1], top_rgb[2], 1.0],
                    [bottom_rgb[0], bottom_rgb[1], bottom_rgb[2], 1.0],
                )
            } else {
                let top_rgb = target_colors.0;
                let bottom_rgb = target_colors.1;
                (
                    [top_rgb[0], top_rgb[1], top_rgb[2], 1.0],
                    [bottom_rgb[0], bottom_rgb[1], bottom_rgb[2], 1.0],
                )
            }
        } else {
            ([0.0; 4], [0.0; 4])
        };

        // 2. Pre-calculate Album Art colors
        let art_tint_color = if show_art_fg || show_art_bg || show_color_bg {
            self.state
                .current_track
                .as_ref()
                .map(|track| {
                    let target_color = track
                        .palette
                        .as_deref()
                        .and_then(|p| p.first())
                        .copied()
                        .unwrap_or([0.1, 0.1, 0.1]);
                    if self.state.transition_progress < 1.0 {
                        let prev_color = self
                            .state
                            .previous_palette
                            .as_deref()
                            .and_then(|p| p.first())
                            .copied()
                            .unwrap_or([0.1, 0.1, 0.1]);
                        lerp_colour(prev_color, target_color, self.state.transition_progress)
                    } else {
                        target_color
                    }
                })
                .unwrap_or([0.1, 0.1, 0.1])
        } else {
            [0.1, 0.1, 0.1]
        };

        // 3. Pre-calculate Sky colors
        let sky_color_data = if self.custom_bg_bind_group.is_none() {
            let elapsed = self.start_time.elapsed().as_secs_f32();
            let mut weather_type = 0u32;
            let sky = time_to_sky_colour(self.state.time_of_day);
            let final_sky = if let Some(weather) = &self.state.weather {
                weather_type = match weather.condition {
                    WeatherCondition::Clear | WeatherCondition::PartlyCloudy => 0,
                    WeatherCondition::Cloudy | WeatherCondition::Fog => 1,
                    WeatherCondition::Rain | WeatherCondition::Thunderstorm => 2,
                    WeatherCondition::Snow => 3,
                };
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
            Some((elapsed, weather_type, final_sky))
        } else {
            None
        };

        // 4. Pre-calculate Text colors (luminance and tinting)
        let (primary_text, secondary_text) = {
            let text_bg_color = self
                .state
                .current_track
                .as_ref()
                .and_then(|t| t.palette.as_deref())
                .and_then(|p| p.first())
                .copied()
                .unwrap_or([0.1, 0.1, 0.1]);
            let text_accent = self
                .state
                .current_track
                .as_ref()
                .and_then(|t| t.palette.as_deref())
                .and_then(|p| p.get(1).or_else(|| p.first()))
                .copied()
                .unwrap_or([1.0, 1.0, 1.0]);

            let luminance =
                0.299 * text_bg_color[0] + 0.587 * text_bg_color[1] + 0.114 * text_bg_color[2];
            if luminance > 0.55 {
                // Dark text for bright backgrounds, tinted with the accent color
                let tint = [
                    text_accent[0] * 0.3,
                    text_accent[1] * 0.3,
                    text_accent[2] * 0.3,
                ];
                (
                    [tint[0], tint[1], tint[2], 1.0],
                    [tint[0], tint[1], tint[2], 0.7],
                )
            } else {
                // Light text for dark backgrounds, lightly tinted with the accent color
                let tint = [
                    text_accent[0] * 0.3 + 0.7,
                    text_accent[1] * 0.3 + 0.7,
                    text_accent[2] * 0.3 + 0.7,
                ];
                (
                    [tint[0], tint[1], tint[2], 1.0],
                    [tint[0], tint[1], tint[2], 0.45],
                )
            }
        };

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
                #[repr(C, align(16))]
                struct VisUniforms {
                    res: [f32; 2],
                    bands: u32,
                    pulse: f32,
                    top: [f32; 4],
                    bottom: [f32; 4],
                    pos_size_rot: [f32; 4],
                    amplitude: f32,
                    shape: u32,
                    time: f32,
                    align: u32,
                    is_waveform: u32,
                    _padding: [u32; 3],
                }
                let shape_u32 = match self.theme.visualiser.shape {
                    VisShape::Circular => 0,
                    VisShape::Linear => 1,
                    VisShape::Square => 2,
                };
                let align_u32 = match self.theme.visualiser.align {
                    VisAlign::Left => 0,
                    VisAlign::Center => 1,
                    VisAlign::Right => 2,
                };
                let vis_uniforms = VisUniforms {
                    res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                    bands: self.state.config.audio.bands as u32,
                    pulse: pulse * 2.0, // Multiplier guarantees visible beat effects
                    top: top_col,
                    bottom: bottom_col,
                    pos_size_rot: [
                        self.theme.visualiser.position[0],
                        self.theme.visualiser.position[1],
                        self.theme.visualiser.size,
                        self.theme.visualiser.rotation.to_radians(),
                    ],
                    amplitude: self.theme.visualiser.amplitude,
                    shape: shape_u32,
                    time: self.start_time.elapsed().as_secs_f32(),
                    align: align_u32,
                    is_waveform: if is_waveform_style { 1 } else { 0 },
                    _padding: [0; 3],
                };
                let vis_bytes = unsafe {
                    std::slice::from_raw_parts(
                        &vis_uniforms as *const _ as *const u8,
                        std::mem::size_of::<VisUniforms>(),
                    )
                };
                self.queue
                    .write_buffer(&self.visualiser_pass.uniform_buffer, 0, vis_bytes);
            }

            // 2. Process album art uniforms
            if show_art_fg || show_art_bg || show_color_bg {
                if let Some(_track) = &self.state.current_track {
                    let color = art_tint_color;
                    let bg_mode = if show_color_bg {
                        3
                    } else if self.state.config.appearance.disable_blur {
                        2
                    } else {
                        0
                    };
                    // Fade out the album art background completely when transparent background is enabled
                    let bg_alpha_val = 1.0 - self.state.transparent_fade;

                    let bg_uniforms = ArtUniforms {
                        color_and_transition: [
                            color[0],
                            color[1],
                            color[2],
                            self.state.transition_progress,
                        ],
                        res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                        art_position: [0.5, 0.5],
                        audio_energy,
                        mode: bg_mode,
                        bg_alpha: bg_alpha_val,
                        art_size: 1.0,
                        shape: 0,
                        blur_opacity: self.state.config.appearance.blur_opacity,
                        image_res: [
                            self.current_album_texture
                                .as_ref()
                                .map(|t| t.size().width as f32)
                                .unwrap_or(1.0),
                            self.current_album_texture
                                .as_ref()
                                .map(|t| t.size().height as f32)
                                .unwrap_or(1.0),
                        ],
                    };
                    let bg_bytes = unsafe {
                        std::slice::from_raw_parts(
                            &bg_uniforms as *const _ as *const u8,
                            std::mem::size_of::<ArtUniforms>(),
                        )
                    };
                    self.queue
                        .write_buffer(&self.album_art_bg_uniform_buffer, 0, bg_bytes);

                    let mut art_position = self.theme.album_art.position;
                    let mut art_size = self.theme.album_art.size;
                    let mut art_shape = self.theme.album_art.shape;

                    // If the circular visualiser is active, dynamically override the album art
                    // layout to fit perfectly inside of it.
                    if has_audio && self.theme.visualiser.shape == VisShape::Circular {
                        art_position = self.theme.visualiser.position;
                        art_size = self.theme.visualiser.size;
                        art_shape = ArtShape::Circular; // Force circular shape to match
                    }

                    let fg_uniforms = ArtUniforms {
                        color_and_transition: [
                            color[0],
                            color[1],
                            color[2],
                            self.state.transition_progress,
                        ],
                        res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                        art_position,
                        audio_energy,
                        mode: 1,
                        bg_alpha: 1.0, // The sharp foreground art never fades!
                        art_size,
                        shape: if art_shape == ArtShape::Circular {
                            1
                        } else {
                            0
                        },
                        blur_opacity: 1.0,
                        image_res: [
                            self.current_album_texture
                                .as_ref()
                                .map(|t| t.size().width as f32)
                                .unwrap_or(1.0),
                            self.current_album_texture
                                .as_ref()
                                .map(|t| t.size().height as f32)
                                .unwrap_or(1.0),
                        ],
                    };
                    let fg_bytes = unsafe {
                        std::slice::from_raw_parts(
                            &fg_uniforms as *const _ as *const u8,
                            std::mem::size_of::<ArtUniforms>(),
                        )
                    };
                    self.queue
                        .write_buffer(&self.album_art_fg_uniform_buffer, 0, fg_bytes);
                }
            }

            if self.custom_bg_bind_group.is_some() {
                // 4. Process custom background uniforms
                let bg_mode = if self.state.config.appearance.disable_blur {
                    2
                } else {
                    0
                };
                let bg_alpha_val = 1.0 - self.state.transparent_fade;

                let custom_bg_uniforms = ArtUniforms {
                    color_and_transition: [1.0, 1.0, 1.0, 1.0], // Don't tint the desktop wallpaper
                    res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                    art_position: [0.5, 0.5],
                    audio_energy,
                    mode: bg_mode,
                    bg_alpha: bg_alpha_val,
                    art_size: 1.0,
                    shape: 0,
                    blur_opacity: self.state.config.appearance.blur_opacity,
                    image_res: [
                        self.current_custom_bg_size
                            .map(|s| s.0 as f32)
                            .unwrap_or(1.0),
                        self.current_custom_bg_size
                            .map(|s| s.1 as f32)
                            .unwrap_or(1.0),
                    ],
                };
                let cbg_bytes = unsafe {
                    std::slice::from_raw_parts(
                        &custom_bg_uniforms as *const _ as *const u8,
                        std::mem::size_of::<ArtUniforms>(),
                    )
                };
                self.queue
                    .write_buffer(&self.custom_bg_uniform_buffer, 0, cbg_bytes);
            } else if let Some((elapsed, weather_type, final_sky)) = sky_color_data {
                // 3. Process ambient uniforms
                let bg_alpha_val = 1.0 - self.state.transparent_fade;

                #[repr(C, align(16))]
                struct AmbUniforms {
                    res: [f32; 2],
                    time: f32,
                    weather: u32,
                    sky: [f32; 4],
                    bg_alpha: f32,
                    // Padding to match std140 layout alignment rules for vec4/arrays
                    _padding: [f32; 3],
                }
                let amb_uniforms = AmbUniforms {
                    res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                    time: elapsed,
                    weather: weather_type,
                    sky: [final_sky[0], final_sky[1], final_sky[2], 1.0],
                    bg_alpha: bg_alpha_val,
                    _padding: [0.0; 3],
                };
                let amb_bytes = unsafe {
                    std::slice::from_raw_parts(
                        &amb_uniforms as *const _ as *const u8,
                        std::mem::size_of::<AmbUniforms>(),
                    )
                };
                self.queue
                    .write_buffer(&self.ambient_uniform_buffer, 0, amb_bytes);
            }

            // --- Prepare Text for Rendering ---
            let width_f = gpu_out.config.width as f32;
            let height_f = gpu_out.config.height as f32;
            let scale_factor = wayland_manager
                .app_data
                .windows
                .get(i)
                .map(|w| w.scale_factor as f32)
                .unwrap_or(1.0);
            let logical_height = height_f / scale_factor;

            let map_align = |a: &TextAlign| -> cosmic_text::Align {
                match a {
                    TextAlign::Left => cosmic_text::Align::Left,
                    TextAlign::Center => cosmic_text::Align::Center,
                    TextAlign::Right => cosmic_text::Align::Right,
                }
            };

            let family = self
                .state
                .config
                .appearance
                .font_family
                .as_deref()
                .map_or(Family::SansSerif, Family::Name);
            let attrs = Attrs::new().family(family);

            let mut text_buffers = Vec::new();

            if self.state.config.audio.show_lyrics {
                if let Some(track) = &self.state.current_track {
                    if let Some(lyrics) = &track.lyrics {
                        let base_font_size =
                            (logical_height * 0.04).clamp(16.0, 48.0) * scale_factor;
                        let active_font_size = base_font_size * 1.5;
                        let line_spacing = active_font_size * 1.2;

                        let start_idx = self.current_lyric_idx.saturating_sub(2);
                        let end_idx = (self.current_lyric_idx + 2).min(lyrics.len());

                        for i in start_idx..=end_idx {
                            if i == 0 || i > lyrics.len() {
                                continue;
                            }

                            let lyric_line = &lyrics[i - 1];
                            // Compute exactly how far this string is from the "current active string"
                            let dist = (i as f32)
                                - (self.current_lyric_idx as f32)
                                - self.lyric_scroll_offset;
                            let abs_dist = dist.abs();

                            if abs_dist > 2.0 {
                                continue;
                            }

                            let center_weight = (1.0 - abs_dist).clamp(0.0, 1.0);

                            let scale = base_font_size
                                + (active_font_size - base_font_size) * center_weight;
                            let final_scale = scale
                                + (self.lyric_bounce_value * 8.0 * scale_factor) * center_weight;

                            let render_scale = final_scale / active_font_size;
                            let bounce_y =
                                (self.lyric_bounce_value * 12.0 * scale_factor) * center_weight;
                            let y_pos = (dist * line_spacing) - bounce_y;

                            let color = [
                                secondary_text[0]
                                    + (primary_text[0] - secondary_text[0]) * center_weight,
                                secondary_text[1]
                                    + (primary_text[1] - secondary_text[1]) * center_weight,
                                secondary_text[2]
                                    + (primary_text[2] - secondary_text[2]) * center_weight,
                                secondary_text[3]
                                    + (primary_text[3] - secondary_text[3]) * center_weight,
                            ];

                            // Fade out gracefully to prevent popping strings at top/bottom
                            let alpha_fade = (1.5 - abs_dist).clamp(0.0, 1.0);
                            let final_color = [color[0], color[1], color[2], color[3] * alpha_fade];

                            if final_color[3] > 0.01 {
                                let metrics =
                                    Metrics::new(active_font_size, active_font_size * 1.2);
                                let mut buffer = self
                                    .text_buffer_cache
                                    .remove(lyric_line.text.as_ref())
                                    .unwrap_or_else(|| {
                                        let mut b = Buffer::new(&mut self.font_system, metrics);
                                        b.set_metrics(&mut self.font_system, metrics);
                                        b.set_size(&mut self.font_system, width_f, height_f);
                                        b.set_text(
                                            &mut self.font_system,
                                            &lyric_line.text,
                                            attrs,
                                            Shaping::Advanced,
                                        );
                                        b
                                    });
                                buffer.set_metrics(&mut self.font_system, metrics);
                                buffer.set_size(&mut self.font_system, width_f, height_f);

                                let align = map_align(&self.theme.lyrics.align);
                                buffer.lines.iter_mut().for_each(|line| {
                                    line.set_align(Some(align));
                                });

                                let pos = [
                                    self.theme.lyrics.position[0] * width_f,
                                    self.theme.lyrics.position[1] * height_f + y_pos,
                                ];

                                text_buffers.push(PositionedBuffer {
                                    buffer,
                                    text_key: lyric_line.text.to_string(),
                                    pos,
                                    color: final_color,
                                    scale: render_scale,
                                    align,
                                });
                            }
                        }
                    }
                }
            }

            if self.state.current_track.is_some() && !self.cached_track_str.is_empty() {
                let info_scale = (logical_height * 0.025).clamp(16.0, 36.0) * scale_factor;
                let metrics = Metrics::new(info_scale, info_scale * 1.2);
                let mut buffer = self
                    .text_buffer_cache
                    .remove(&self.cached_track_str)
                    .unwrap_or_else(|| {
                        let mut b = Buffer::new(&mut self.font_system, metrics);
                        b.set_metrics(&mut self.font_system, metrics);
                        b.set_size(&mut self.font_system, width_f, height_f);
                        b.set_text(
                            &mut self.font_system,
                            &self.cached_track_str,
                            attrs,
                            Shaping::Advanced,
                        );
                        b
                    });
                buffer.set_metrics(&mut self.font_system, metrics);
                buffer.set_size(&mut self.font_system, width_f, height_f);
                let final_color = [
                    secondary_text[0],
                    secondary_text[1],
                    secondary_text[2],
                    secondary_text[3],
                ];
                let align = map_align(&self.theme.track_info.align);
                buffer.lines.iter_mut().for_each(|line| {
                    line.set_align(Some(align));
                });
                let pos = [
                    self.theme.track_info.position[0] * width_f,
                    self.theme.track_info.position[1] * height_f,
                ];
                text_buffers.push(PositionedBuffer {
                    buffer,
                    text_key: self.cached_track_str.clone(),
                    pos,
                    color: final_color,
                    scale: 1.0,
                    align,
                });
            }

            if self.state.weather.is_some() && !self.cached_weather_str.is_empty() {
                let weather_scale = (logical_height * 0.02).clamp(14.0, 24.0) * scale_factor;
                let metrics = Metrics::new(weather_scale, weather_scale * 1.2);
                let mut buffer = self
                    .text_buffer_cache
                    .remove(&self.cached_weather_str)
                    .unwrap_or_else(|| {
                        let mut b = Buffer::new(&mut self.font_system, metrics);
                        b.set_metrics(&mut self.font_system, metrics);
                        b.set_size(&mut self.font_system, width_f, height_f);
                        b.set_text(
                            &mut self.font_system,
                            &self.cached_weather_str,
                            attrs,
                            Shaping::Advanced,
                        );
                        b
                    });
                buffer.set_metrics(&mut self.font_system, metrics);
                buffer.set_size(&mut self.font_system, width_f, height_f);
                let final_color = [
                    secondary_text[0],
                    secondary_text[1],
                    secondary_text[2],
                    secondary_text[3],
                ];
                let align = map_align(&self.theme.weather.align);
                buffer.lines.iter_mut().for_each(|line| {
                    line.set_align(Some(align));
                });
                let pos = [
                    self.theme.weather.position[0] * width_f,
                    self.theme.weather.position[1] * height_f,
                ];
                text_buffers.push(PositionedBuffer {
                    buffer,
                    text_key: self.cached_weather_str.clone(),
                    pos,
                    color: final_color,
                    scale: 1.0,
                    align,
                });
            }

            // Prepare text vertices
            Self::prepare_text(
                &mut self.text_renderer,
                &self.queue,
                &mut self.font_system,
                &mut self.swash_cache,
                text_buffers.as_mut(),
                width_f,
                height_f,
            );

            // Prevent unbound memory growth for weather/ambient setups left running for days
            if self.text_buffer_cache.len() > 100 {
                self.text_buffer_cache.clear();
                self.text_buffer_cache.shrink_to_fit();
            }

            for p_buf in text_buffers {
                self.text_buffer_cache.insert(p_buf.text_key, p_buf.buffer);
            }

            if self.text_renderer.vertex_capacity < self.text_renderer.cpu_vertices.len() {
                self.text_renderer.vertex_capacity =
                    self.text_renderer.cpu_vertices.len().next_power_of_two();
                self.text_renderer.vertices = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Text Vertex Buffer"),
                    size: (self.text_renderer.vertex_capacity * std::mem::size_of::<TextVertex>())
                        as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            if self.text_renderer.index_capacity < self.text_renderer.cpu_indices.len() {
                self.text_renderer.index_capacity =
                    self.text_renderer.cpu_indices.len().next_power_of_two();
                self.text_renderer.indices = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Text Index Buffer"),
                    size: (self.text_renderer.index_capacity * std::mem::size_of::<u32>()) as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }

            let vertices_bytes = unsafe {
                std::slice::from_raw_parts(
                    self.text_renderer.cpu_vertices.as_ptr() as *const u8,
                    self.text_renderer.cpu_vertices.len() * std::mem::size_of::<TextVertex>(),
                )
            };
            self.queue
                .write_buffer(&self.text_renderer.vertices, 0, vertices_bytes);

            let indices_bytes = unsafe {
                std::slice::from_raw_parts(
                    self.text_renderer.cpu_indices.as_ptr() as *const u8,
                    self.text_renderer.cpu_indices.len() * std::mem::size_of::<u32>(),
                )
            };
            self.queue
                .write_buffer(&self.text_renderer.indices, 0, indices_bytes);
            self.text_renderer.num_indices = self.text_renderer.cpu_indices.len() as u32;

            let view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
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

                // --- Background Rendering ---
                // Simplified logic with clear precedence: Album Art > Custom BG > Ambient
                if show_art_bg || show_color_bg {
                    if let Some(bind_group) = &self.album_art_bg_bind_group {
                        render_pass.set_pipeline(&self.album_art_pipeline);
                        render_pass.set_bind_group(0, bind_group, &[]);
                        render_pass.draw(0..3, 0..1);
                    }
                } else if let Some(bind_group) = &self.custom_bg_bind_group {
                    // Custom Desktop Wallpaper Background (Frosted Glass)
                    render_pass.set_pipeline(&self.album_art_pipeline);
                    render_pass.set_bind_group(0, bind_group, &[]);
                    render_pass.draw(0..3, 0..1);
                } else {
                    // Ambient Procedural Sky
                    render_pass.set_pipeline(&self.ambient_pipeline);
                    render_pass.set_bind_group(0, &self.ambient_bind_group, &[]);
                    render_pass.draw(0..3, 0..1);
                }

                // --- Overlay Layers ---
                if is_weather_active && active_particles > 0 {
                    render_pass.set_pipeline(&self.weather_render_pipeline);
                    render_pass.set_bind_group(0, &self.weather_render_bind_group, &[]);
                    render_pass.draw(0..6, 0..active_particles); // 6 vertices per quad
                }

                if has_audio {
                    render_pass.set_pipeline(&self.visualiser_pass.pipeline);
                    render_pass.set_bind_group(0, &self.visualiser_pass.bind_group, &[]);
                    let instance_count = if is_waveform_style {
                        1
                    } else if self.theme.visualiser.shape == VisShape::Linear {
                        self.state.config.audio.bands as u32
                    } else {
                        self.state.config.audio.bands as u32 * 2
                    };
                    render_pass.draw(0..6, 0..instance_count);
                }

                if show_art_fg {
                    if let Some(bind_group) = &self.album_art_fg_bind_group {
                        render_pass.set_pipeline(&self.album_art_pipeline);
                        render_pass.set_bind_group(0, bind_group, &[]);
                        render_pass.draw(0..3, 0..1);
                    }
                }

                // --- Text Rendering ---
                render_pass.set_pipeline(&self.text_renderer.pipeline);
                render_pass.set_bind_group(0, &self.text_renderer.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.text_renderer.vertices.slice(..));
                render_pass.set_index_buffer(
                    self.text_renderer.indices.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                render_pass.draw_indexed(0..self.text_renderer.num_indices, 0, 0..1);
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
            WallpaperMode::Weather => SceneHint::Ambient,
            WallpaperMode::AlbumArt => SceneHint::AlbumArt,
            WallpaperMode::AudioVisualiser => SceneHint::AudioVisualiser,
            WallpaperMode::Auto => self.state.scene_description(),
        };

        match scene {
            SceneHint::Ambient => {
                let sky = time_to_sky_colour(self.state.time_of_day);
                let final_sky = if let Some(weather) = &self.state.weather {
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
                wgpu::Color {
                    r: final_sky[0] as f64,
                    g: final_sky[1] as f64,
                    b: final_sky[2] as f64,
                    a: 1.0,
                }
            }
            SceneHint::AlbumArt => wgpu::Color {
                r: 0.05,
                g: 0.05,
                b: 0.05,
                a: 1.0,
            },
            SceneHint::AudioVisualiser => wgpu::Color {
                r: 0.1,
                g: 0.1,
                b: 0.15,
                a: 1.0,
            },
        }
    }

    fn update_album_art_texture(&mut self, rgba: &image::RgbaImage) {
        let dimensions = rgba.dimensions();
        info!(
            "Creating GPU texture for album art. Dimensions: {}x{}",
            dimensions.0, dimensions.1
        );

        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        // Guarantee dimensions are compatible with wgpu's 256-byte row alignment!
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let unpadded_bytes_per_row = dimensions.0 * 4;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) & !(align - 1);

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

        if unpadded_bytes_per_row == padded_bytes_per_row {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba.as_raw(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(unpadded_bytes_per_row),
                    rows_per_image: Some(dimensions.1),
                },
                texture_size,
            );
        } else {
            let required_size = (padded_bytes_per_row * dimensions.1) as usize;
            if self.album_art_pad_buffer.len() < required_size {
                self.album_art_pad_buffer.resize(required_size, 0);
            }

            let raw_rgba = rgba.as_raw();
            for y in 0..dimensions.1 {
                let src_start = (y * unpadded_bytes_per_row) as usize;
                let src_end = src_start + unpadded_bytes_per_row as usize;
                let dst_start = (y * padded_bytes_per_row) as usize;
                let dst_slice = &mut self.album_art_pad_buffer
                    [dst_start..dst_start + unpadded_bytes_per_row as usize];
                dst_slice.copy_from_slice(&raw_rgba[src_start..src_end]);
            }
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.album_art_pad_buffer[..required_size],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(dimensions.1),
                },
                texture_size,
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bg_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.album_art_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.album_art_bg_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.album_art_sampler),
                },
            ],
            label: Some("Album Art BG Bind Group"),
        });

        let fg_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.album_art_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.album_art_fg_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.album_art_sampler),
                },
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
                let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
                let unpadded_bytes_per_row = dimensions.0 * 4;
                let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) & !(align - 1);

                if unpadded_bytes_per_row == padded_bytes_per_row {
                    self.queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        rgba.as_raw(),
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(unpadded_bytes_per_row),
                            rows_per_image: Some(dimensions.1),
                        },
                        texture.size(),
                    );
                } else {
                    let required_size = (padded_bytes_per_row * dimensions.1) as usize;
                    if self.video_frame_buffer.len() < required_size {
                        self.video_frame_buffer.resize(required_size, 0);
                    }

                    let raw_rgba = rgba.as_raw();
                    for y in 0..dimensions.1 {
                        let src_start = (y * unpadded_bytes_per_row) as usize;
                        let src_end = src_start + unpadded_bytes_per_row as usize;
                        let dst_start = (y * padded_bytes_per_row) as usize;
                        let dst_slice = &mut self.video_frame_buffer
                            [dst_start..dst_start + unpadded_bytes_per_row as usize];
                        dst_slice.copy_from_slice(&raw_rgba[src_start..src_end]);
                    }

                    self.queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &self.video_frame_buffer[..required_size],
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(padded_bytes_per_row),
                            rows_per_image: Some(dimensions.1),
                        },
                        texture.size(),
                    );
                }
                return;
            }
        }

        // Slow-path: If dimensions changed (e.g. switching from square album art to 9:16 Canvas video),
        // this will rebuild the wgpu texture and elegantly crossfade into the video loop!
        self.update_album_art_texture(rgba);
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
        self.current_custom_bg_size = Some(dimensions);
        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        // Guarantee dimensions are compatible with wgpu's 256-byte row alignment!
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let unpadded_bytes_per_row = dimensions.0 * 4;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) & !(align - 1);

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

        if unpadded_bytes_per_row == padded_bytes_per_row {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                img.as_raw(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(unpadded_bytes_per_row),
                    rows_per_image: Some(dimensions.1),
                },
                texture_size,
            );
        } else {
            let required_size = (padded_bytes_per_row * dimensions.1) as usize;
            if self.album_art_pad_buffer.len() < required_size {
                self.album_art_pad_buffer.resize(required_size, 0);
            }

            let raw_rgba = img.as_raw();
            for y in 0..dimensions.1 {
                let src_start = (y * unpadded_bytes_per_row) as usize;
                let src_end = src_start + unpadded_bytes_per_row as usize;
                let dst_start = (y * padded_bytes_per_row) as usize;
                let dst_slice = &mut self.album_art_pad_buffer
                    [dst_start..dst_start + unpadded_bytes_per_row as usize];
                dst_slice.copy_from_slice(&raw_rgba[src_start..src_end]);
            }
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.album_art_pad_buffer[..required_size],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(dimensions.1),
                },
                texture_size,
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.custom_bg_bind_group =
            Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.album_art_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.custom_bg_uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.album_art_sampler),
                    },
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

            let a_weighting = (12200.0 * 12200.0 * f4)
                / ((f2 + 20.6 * 20.6)
                    * (f2 + 12200.0 * 12200.0)
                    * ((f2 + 107.7 * 107.7) * (f2 + 737.9 * 737.9)).sqrt());

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
            if bin_hi <= bin_lo {
                bin_hi = (bin_lo + 1).min(max_bins);
            }
            ranges.push((bin_lo, bin_hi));
        }
        ranges
    }

    fn build_waveform_bin_ranges(band_count: usize) -> Vec<(usize, usize)> {
        let chunk_size = 2048.0 / band_count as f32;
        (0..band_count)
            .map(|i| {
                let start = (i as f32 * chunk_size) as usize;
                let end = ((i + 1) as f32 * chunk_size) as usize;
                (start, end.min(2048))
            })
            .collect()
    }

    fn update_weather_string(&mut self) {
        if let Some(weather) = &self.state.weather {
            let mut val = weather.temperature_celsius;
            let mut unit = "C";
            if self.state.config.weather.temperature_unit == TemperatureUnit::Fahrenheit {
                val = (val * 9.0 / 5.0) + 32.0;
                unit = "F";
            }
            let condition_str = match weather.condition {
                WeatherCondition::Clear => "Clear",
                WeatherCondition::PartlyCloudy => "Partly Cloudy",
                WeatherCondition::Cloudy => "Cloudy",
                WeatherCondition::Rain => "Rain",
                WeatherCondition::Snow => "Snow",
                WeatherCondition::Thunderstorm => "Thunderstorm",
                WeatherCondition::Fog => "Fog",
            };
            self.cached_weather_str = format!("{} {:.1}°{}", condition_str, val, unit);
        } else {
            self.cached_weather_str.clear();
        }
    }

    fn prepare_text(
        text_renderer: &mut TextRenderer,
        queue: &wgpu::Queue,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
        positioned_buffers: &mut [PositionedBuffer],
        width: f32,
        height: f32,
    ) {
        text_renderer.cpu_vertices.clear();
        text_renderer.cpu_indices.clear();

        for p_buf in positioned_buffers.iter_mut() {
            p_buf.buffer.shape_until_scroll(font_system, false);
        }

        for p_buf in positioned_buffers {
            let origin_x = match p_buf.align {
                cosmic_text::Align::Left => 0.0,
                cosmic_text::Align::Right => width,
                _ => width / 2.0,
            };
            let origin_y = p_buf.buffer.metrics().font_size;

            let buffer_offset_x = match p_buf.align {
                cosmic_text::Align::Left => p_buf.pos[0],
                cosmic_text::Align::Right => p_buf.pos[0] - width,
                _ => p_buf.pos[0] - width / 2.0,
            };
            let buffer_offset_y = p_buf.pos[1] - origin_y;

            for run in p_buf.buffer.layout_runs() {
                for glyph in run.glyphs.iter() {
                    // Force subpixel rendering layout to absolute 0.0 offsets. We do the real positioning in the shader!
                    let physical_glyph: cosmic_text::PhysicalGlyph =
                        glyph.physical((0.0, 0.0), 1.0);
                    let cache_key = physical_glyph.cache_key;

                    // Rasterize and pack into texture atlas if not already cached
                    if !text_renderer.glyph_cache.contains_key(&cache_key) {
                        if let Some(image) = swash_cache.get_image(font_system, cache_key) {
                            let img_w = image.placement.width;
                            let img_h = image.placement.height;

                            if img_w == 0 || img_h == 0 {
                                text_renderer.glyph_cache.insert(
                                    cache_key,
                                    CachedGlyph {
                                        uv: [0.0, 0.0, 0.0, 0.0],
                                        offset: [0, 0],
                                        size: [0, 0],
                                    },
                                );
                                continue;
                            }

                            if img_w > GLYPH_CACHE_WIDTH || img_h > GLYPH_CACHE_HEIGHT {
                                tracing::warn!("Glyph ({}x{}) too large for cache!", img_w, img_h);
                                continue;
                            }

                            if text_renderer.cache_x + img_w > GLYPH_CACHE_WIDTH {
                                text_renderer.cache_x = 0;
                                text_renderer.cache_y += text_renderer.cache_row_height;
                                text_renderer.cache_row_height = 0;
                            }

                            if text_renderer.cache_y + img_h > GLYPH_CACHE_HEIGHT {
                                tracing::warn!("Glyph cache full! Clearing and starting fresh.");
                                text_renderer.glyph_cache.clear();
                                text_renderer.cache_x = 0;
                                text_renderer.cache_y = 0;
                                text_renderer.cache_row_height = 0;
                            }

                            let cur_x = text_renderer.cache_x;
                            let cur_y = text_renderer.cache_y;

                            if let cosmic_text::SwashContent::Mask = image.content {
                                queue.write_texture(
                                    wgpu::ImageCopyTexture {
                                        texture: &text_renderer.texture,
                                        mip_level: 0,
                                        origin: wgpu::Origin3d {
                                            x: cur_x,
                                            y: cur_y,
                                            z: 0,
                                        },
                                        aspect: wgpu::TextureAspect::All,
                                    },
                                    &image.data,
                                    wgpu::ImageDataLayout {
                                        offset: 0,
                                        bytes_per_row: Some(img_w),
                                        rows_per_image: Some(img_h),
                                    },
                                    wgpu::Extent3d {
                                        width: img_w,
                                        height: img_h,
                                        depth_or_array_layers: 1,
                                    },
                                );
                            }

                            let u_min = cur_x as f32 / GLYPH_CACHE_WIDTH as f32;
                            let v_min = cur_y as f32 / GLYPH_CACHE_HEIGHT as f32;
                            let u_max = (cur_x + img_w) as f32 / GLYPH_CACHE_WIDTH as f32;
                            let v_max = (cur_y + img_h) as f32 / GLYPH_CACHE_HEIGHT as f32;

                            text_renderer.glyph_cache.insert(
                                cache_key,
                                CachedGlyph {
                                    uv: [u_min, v_min, u_max, v_max],
                                    offset: [image.placement.left, image.placement.top],
                                    size: [img_w, img_h],
                                },
                            );

                            text_renderer.cache_x += img_w + 1; // 1px padding
                            text_renderer.cache_row_height =
                                text_renderer.cache_row_height.max(img_h + 1);
                        }
                    }

                    // Retrieve from cache and build vertex layout
                    if let Some(cached) = text_renderer.glyph_cache.get(&cache_key) {
                        if cached.size[0] == 0 || cached.size[1] == 0 {
                            continue;
                        }

                        let dx = glyph.x - origin_x;
                        let dy = run.line_y + glyph.y - origin_y;

                        let scaled_glyph_x = origin_x + dx * p_buf.scale;
                        let scaled_glyph_y = origin_y + dy * p_buf.scale;

                        let final_x = buffer_offset_x
                            + scaled_glyph_x
                            + cached.offset[0] as f32 * p_buf.scale;
                        let final_y = buffer_offset_y + scaled_glyph_y
                            - cached.offset[1] as f32 * p_buf.scale;

                        let x = final_x / width * 2.0 - 1.0;
                        let y = -(final_y / height * 2.0 - 1.0);

                        let w = (cached.size[0] as f32 * p_buf.scale) / width * 2.0;
                        let h = (cached.size[1] as f32 * p_buf.scale) / height * 2.0;

                        let color = p_buf.color;

                        let base_index = text_renderer.cpu_vertices.len() as u32;
                        let [u_min, v_min, u_max, v_max] = cached.uv;

                        text_renderer.cpu_vertices.push(TextVertex {
                            pos: [x, y],
                            tex_pos: [u_min, v_min],
                            color,
                        });
                        text_renderer.cpu_vertices.push(TextVertex {
                            pos: [x + w, y],
                            tex_pos: [u_max, v_min],
                            color,
                        });
                        text_renderer.cpu_vertices.push(TextVertex {
                            pos: [x, y - h],
                            tex_pos: [u_min, v_max],
                            color,
                        });
                        text_renderer.cpu_vertices.push(TextVertex {
                            pos: [x + w, y - h],
                            tex_pos: [u_max, v_max],
                            color,
                        });

                        text_renderer.cpu_indices.push(base_index);
                        text_renderer.cpu_indices.push(base_index + 1);
                        text_renderer.cpu_indices.push(base_index + 2);
                        text_renderer.cpu_indices.push(base_index + 1);
                        text_renderer.cpu_indices.push(base_index + 3);
                        text_renderer.cpu_indices.push(base_index + 2);
                    }
                }
            }
        }
    }
}
