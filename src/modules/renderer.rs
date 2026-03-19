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
};

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
    font: wgpu_text::glyph_brush::ab_glyph::FontArc,
    visualiser_pipeline: wgpu::RenderPipeline,
    visualiser_bind_group: wgpu::BindGroup,
    bands_buffer: wgpu::Buffer,
    visualiser_uniform_buffer: wgpu::Buffer,
    album_art_pipeline: wgpu::RenderPipeline,
    album_art_layout: wgpu::BindGroupLayout,
    album_art_bg_uniform_buffer: wgpu::Buffer,
    album_art_fg_uniform_buffer: wgpu::Buffer,
    album_art_bg_bind_group: Option<wgpu::BindGroup>,
    album_art_fg_bind_group: Option<wgpu::BindGroup>,
    previous_album_view: wgpu::TextureView,
    current_album_view: wgpu::TextureView,
    ambient_pipeline: wgpu::RenderPipeline,
    ambient_bind_group: wgpu::BindGroup,
    ambient_uniform_buffer: wgpu::Buffer,
    start_time: Instant,
    state: AppState,
    frame_duration: Duration,
}

impl Renderer {
    pub async fn new(wayland_manager: &WaylandManager, state: AppState) -> Result<Self> {
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
        let font = wgpu_text::glyph_brush::ab_glyph::FontArc::try_from_vec(font_bytes).unwrap();

        let mut outputs = Vec::new();
        for (info, surface) in outputs_info.into_iter().zip(surfaces) {
            let caps = surface.get_capabilities(&adapter);
            let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: info.width,
                height: info.height,
                present_mode: caps.present_modes[0],
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);
            
            let text_brush = wgpu_text::BrushBuilder::using_fonts(vec![font.clone()])
                .build(&device, info.width, info.height, format);
                
            outputs.push(GpuOutput { surface, config, text_brush });
        }

        let config_format = outputs[0].config.format;

        // --- Visualiser Pipeline Setup ---
        let mut uniform_data = Vec::with_capacity(48);
        uniform_data.extend_from_slice(&(outputs[0].config.width as f32).to_ne_bytes());
        uniform_data.extend_from_slice(&(outputs[0].config.height as f32).to_ne_bytes());
        uniform_data.extend_from_slice(&(state.config.audio.bands as u32).to_ne_bytes());
        uniform_data.extend_from_slice(&0.0f32.to_ne_bytes()); // initial lyric pulse
        
        let default_top: [f32; 4] = [1.0, 0.2, 0.5, 1.0];
        let default_bottom: [f32; 4] = [0.2, 0.5, 1.0, 1.0];
        for f in default_top { uniform_data.extend_from_slice(&f.to_ne_bytes()); }
        for f in default_bottom { uniform_data.extend_from_slice(&f.to_ne_bytes()); }

        let visualiser_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Visualiser Uniform Buffer"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&visualiser_uniform_buffer, 0, &uniform_data);

        let bands_size = (state.config.audio.bands * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
        let bands_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Audio Bands Buffer"),
            size: bands_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Visualiser Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("visualiser.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Visualiser Bind Group Layout"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(bands_size),
                    },
                    count: None,
                },
            ],
        });

        let visualiser_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Visualiser Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: visualiser_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: bands_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Visualiser Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let visualiser_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Visualiser Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
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
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let album_art_fg_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Album Art FG Uniform Buffer"),
            size: 32,
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
                        min_binding_size: wgpu::BufferSize::new(32),
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
            size: 32, // resolution(8) + time(4) + weather(4) + sky_color(16)
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
                        min_binding_size: wgpu::BufferSize::new(32),
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

        info!("Renderer initialised at {}fps", fps);

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            outputs,
            font,
            visualiser_pipeline,
            visualiser_bind_group,
            bands_buffer,
            visualiser_uniform_buffer,
            album_art_pipeline,
            album_art_layout,
            album_art_bg_uniform_buffer,
            album_art_fg_uniform_buffer,
            album_art_bg_bind_group: None,
            album_art_fg_bind_group: None,
            previous_album_view,
            current_album_view,
            ambient_pipeline,
            ambient_bind_group,
            ambient_uniform_buffer,
            start_time: Instant::now(),
            state,
            frame_duration: Duration::from_secs_f64(1.0 / fps as f64),
        })
    }

    pub async fn run(
        &mut self,
        mut event_rx: Receiver<Event>,
        mut wayland_manager: WaylandManager,
    ) -> Result<()> {
        let mut last_frame = Instant::now();

        loop {
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

                    let config = wgpu::SurfaceConfiguration {
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        format,
                        width: info.width.max(1),
                        height: info.height.max(1),
                        present_mode: caps.present_modes[0],
                        alpha_mode: caps.alpha_modes[0],
                        view_formats: vec![],
                        desired_maximum_frame_latency: 2,
                    };
                    surface.configure(&self.device, &config);
                    
                    let text_brush = wgpu_text::BrushBuilder::using_fonts(vec![self.font.clone()])
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

            while let Ok(event) = event_rx.try_recv() {
                self.handle_event(event);
            }

            self.state.update_time();

            let now = Instant::now();
            let delta = now.duration_since(last_frame).as_secs_f32();
            self.state.tick_transition(delta);
            last_frame = now;

            self.draw_frame()?;

            let elapsed = last_frame.elapsed();
            if elapsed < self.frame_duration {
                tokio::time::sleep(self.frame_duration - elapsed).await;
            }
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::TrackChanged(track) => {
                info!("Now playing: {} - {}", track.artist, track.title);
                if let Some(art) = &track.album_art {
                    self.update_album_art_texture(art);
                }
                self.state.previous_palette = self.state.current_track.as_ref().and_then(|t| t.palette.clone());
                self.state.current_track = Some(track);
                self.state.is_playing = true;
                self.state.begin_transition();
            }

            Event::PlaybackStopped => {
                self.state.previous_palette = self.state.current_track.as_ref().and_then(|t| t.palette.clone());
                self.state.is_playing = false;
                self.state.begin_transition();
            }

            Event::PlaybackPosition(pos) => {
                self.state.playback_position = pos;
            }

            Event::AudioFrame(bands) => {
                let smoothing = self.state.config.audio.smoothing;
                let target_len = self.state.audio_bands.len();

                for (i, current) in self.state.audio_bands.iter_mut().enumerate() {
                    let src = i as f32 * bands.len() as f32 / target_len as f32;
                    let lo = src.floor() as usize;
                    let hi = (lo + 1).min(bands.len() - 1);
                    let t = src.fract();
                    let target = bands[lo] * (1.0 - t) + bands[hi] * t;
                    
                    *current = *current * smoothing + target * (1.0 - smoothing);
                }
            }

            Event::WeatherUpdated(weather) => {
                info!(
                    "Weather: {:?} {:.1}°C",
                    weather.condition, weather.temperature_celsius
                );
                self.state.weather = Some(weather);
                self.state.begin_transition();
            }
        }
    }

    fn draw_frame(&mut self) -> Result<()> {
        let _scene = self.state.scene_description();
        
        let max_energy = self.state.audio_bands.iter().cloned().fold(0.0f32, f32::max);
        let has_audio = max_energy > 0.001;
        
        let audio_energy = if self.state.audio_bands.is_empty() { 0.0 } else {
            (self.state.audio_bands.iter().sum::<f32>() / self.state.audio_bands.len() as f32) * 5.0
        };
        let has_art = self.state.is_playing && self.state.current_track.as_ref().and_then(|t| t.album_art.as_ref()).is_some();
        
        let clear_colour = self.get_clear_colour();
        let active_lyric = self.state.active_lyric().map(String::from);
        let pulse = self.state.lyric_pulse();
        let (prev_lyric, current_lyric, next_lyric) = self.state.active_lyrics();
        let prev_text = prev_lyric.map(String::from);
        let curr_text = current_lyric.map(String::from);
        let next_text = next_lyric.map(String::from);

        if has_audio {
            let bands_bytes = unsafe {
                std::slice::from_raw_parts(
                    self.state.audio_bands.as_ptr() as *const u8,
                    self.state.audio_bands.len() * std::mem::size_of::<f32>(),
                )
            };
            self.queue.write_buffer(&self.bands_buffer, 0, bands_bytes);
        }

        for gpu_out in &mut self.outputs {
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

            // 1. Process visualizer uniforms
            let get_colors = |palette: Option<&[[f32; 3]]>| -> ([f32; 3], [f32; 3]) {
                match palette {
                    Some(p) if p.len() >= 2 => (p[0], p[1]),
                    Some(p) if p.len() == 1 => (p[0], [p[0][0] * 0.5, p[0][1] * 0.5, p[0][2] * 0.5]),
                    _ => ([1.0, 0.2, 0.5], [0.2, 0.5, 1.0]), // Fallback neon gradient
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
            struct VisUniforms { res: [f32; 2], bands: u32, pulse: f32, top: [f32; 4], bottom: [f32; 4] }
            let vis_uniforms = VisUniforms {
                res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                bands: self.state.config.audio.bands as u32,
                pulse: self.state.lyric_pulse(),
                pulse,
                top: top_col,
                bottom: bottom_col,
            };
            let vis_bytes = unsafe { std::slice::from_raw_parts(&vis_uniforms as *const _ as *const u8, std::mem::size_of::<VisUniforms>()) };
            self.queue.write_buffer(&self.visualiser_uniform_buffer, 0, vis_bytes);

            // 2. Process album art uniforms
            if let Some(track) = &self.state.current_track {
                let target_color = track.palette.as_deref().and_then(|p| p.first()).copied().unwrap_or([0.1, 0.1, 0.1]);
                let color = if self.state.transition_progress < 1.0 {
                    let prev_color = self.state.previous_palette.as_deref().and_then(|p| p.first()).copied().unwrap_or([0.1, 0.1, 0.1]);
                    lerp_colour(prev_color, target_color, self.state.transition_progress)
                } else { target_color };

                #[repr(C)]
                struct ArtUniforms { color_and_transition: [f32; 4], res: [f32; 2], audio_energy: f32, mode: u32 }
                
                let bg_uniforms = ArtUniforms {
                    color_and_transition: [color[0], color[1], color[2], self.state.transition_progress],
                    res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                    audio_energy,
                    mode: 0,
                };
                let bg_bytes = unsafe { std::slice::from_raw_parts(&bg_uniforms as *const _ as *const u8, std::mem::size_of::<ArtUniforms>()) };
                self.queue.write_buffer(&self.album_art_bg_uniform_buffer, 0, bg_bytes);

                let fg_uniforms = ArtUniforms {
                    color_and_transition: [color[0], color[1], color[2], self.state.transition_progress],
                    res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                    audio_energy,
                    mode: 1,
                };
                let fg_bytes = unsafe { std::slice::from_raw_parts(&fg_uniforms as *const _ as *const u8, std::mem::size_of::<ArtUniforms>()) };
                self.queue.write_buffer(&self.album_art_fg_uniform_buffer, 0, fg_bytes);
            }

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

            #[repr(C)]
            struct AmbUniforms { res: [f32; 2], time: f32, weather: u32, sky: [f32; 4] }
            let amb_uniforms = AmbUniforms {
                res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                time: elapsed, weather: weather_type, sky: [final_sky[0], final_sky[1], final_sky[2], 1.0]
            };
            let amb_bytes = unsafe { std::slice::from_raw_parts(&amb_uniforms as *const _ as *const u8, std::mem::size_of::<AmbUniforms>()) };
            self.queue.write_buffer(&self.ambient_uniform_buffer, 0, amb_bytes);

            if let Some(text) = &active_lyric {
                let section = Section::default()
                    .add_text(Text::new(text)
                        .with_scale(64.0)
                        .with_color([1.0, 1.0, 1.0, 1.0]))
                    .with_screen_position((gpu_out.config.width as f32 / 2.0, gpu_out.config.height as f32 - 200.0))
                    .with_layout(Layout::default().h_align(HorizontalAlign::Center));
                
                gpu_out.text_brush.queue(&self.device, &self.queue, vec![&section]).unwrap();
            } else {
                gpu_out.text_brush.queue(&self.device, &self.queue, Vec::<&Section>::new()).unwrap();
            }

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

                // 1. Draw Base Scene (Album Art or fallback Ambient)
                if has_art {
                    if let Some(bind_group) = &self.album_art_bg_bind_group {
                        render_pass.set_pipeline(&self.album_art_pipeline);
                        render_pass.set_bind_group(0, bind_group, &[]);
                        render_pass.draw(0..3, 0..1);
                    }
                } else {
                    render_pass.set_pipeline(&self.ambient_pipeline);
                    render_pass.set_bind_group(0, &self.ambient_bind_group, &[]);
                    render_pass.draw(0..3, 0..1);
                }

                // 2. Overlay Visualiser
                if has_audio {
                    render_pass.set_pipeline(&self.visualiser_pipeline);
                    render_pass.set_bind_group(0, &self.visualiser_bind_group, &[]);
                    render_pass.draw(0..3, 0..1);
                }

                // 3. Overlay Foreground Art
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
        match self.state.scene_description() {
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

    fn update_album_art_texture(&mut self, image: &image::DynamicImage) {
        let rgba = image.to_rgba8();
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
            &rgba,
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
    }
}
