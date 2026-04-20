use anyhow::Result;
use cosmic_text::{self, Buffer, FontSystem, SwashCache};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Receiver;
use tracing::{info, warn};

use crate::modules::event::Event;
use crate::modules::state::AppState;
use crate::modules::visualiser_pass::VisualiserPass;
use crate::modules::wayland::{WaylandManager, WaylandOutput};

pub const GLYPH_CACHE_WIDTH: u32 = 2048;
pub const GLYPH_CACHE_HEIGHT: u32 = 2048;
use super::text::{PositionedBuffer, TextCacheKey, TextRenderer};

use crate::modules::config::{TemperatureUnit, ThemeLayout};
use crate::modules::event::WeatherCondition;
pub struct GpuOutput {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
}

pub struct Renderer {
    pub(crate) instance: wgpu::Instance,
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) outputs: Vec<GpuOutput>,
    pub(crate) font_system: FontSystem,
    pub(crate) swash_cache: SwashCache,
    pub(crate) text_renderer: TextRenderer,
    pub(crate) text_buffer_cache: std::collections::HashMap<TextCacheKey, Buffer>,
    pub(crate) text_buffers: Vec<PositionedBuffer>,
    pub(crate) current_outputs_cache: Vec<WaylandOutput>,
    pub(crate) visualiser_pass: VisualiserPass,
    pub(crate) album_art_pipeline: wgpu::RenderPipeline,
    pub(crate) album_art_layout: wgpu::BindGroupLayout,
    pub(crate) album_art_bg_uniform_buffer: wgpu::Buffer,
    pub(crate) album_art_fg_uniform_buffer: wgpu::Buffer,
    pub(crate) album_art_bg_bind_group: Option<wgpu::BindGroup>,
    pub(crate) album_art_fg_bind_group: Option<wgpu::BindGroup>,
    pub(crate) current_album_texture: Option<wgpu::Texture>,
    pub(crate) album_art_sampler: wgpu::Sampler,
    pub(crate) ambient_pipeline: wgpu::RenderPipeline,
    pub(crate) ambient_bind_group: wgpu::BindGroup,
    pub(crate) ambient_uniform_buffer: wgpu::Buffer,
    pub(crate) custom_bg_uniform_buffer: wgpu::Buffer,
    pub(crate) custom_bg_bind_group: Option<wgpu::BindGroup>,
    pub(crate) current_custom_bg_texture: Option<wgpu::Texture>,
    pub(crate) current_bg_path: Option<String>,
    pub(crate) current_custom_bg_size: Option<(u32, u32)>,
    pub(crate) _particle_buffer: wgpu::Buffer,
    pub(crate) weather_compute_uniform_buffer: wgpu::Buffer,
    pub(crate) weather_compute_bind_group: wgpu::BindGroup,
    pub(crate) weather_compute_pipeline: wgpu::ComputePipeline,
    pub(crate) weather_render_bind_group: wgpu::BindGroup,
    pub(crate) weather_render_pipeline: wgpu::RenderPipeline,
    pub(crate) start_time: Instant,
    pub(crate) state: AppState,
    pub(crate) frame_duration: Duration,
    pub(crate) current_fps: u32,
    pub(crate) show_lyrics_tx: tokio::sync::watch::Sender<bool>,
    pub(crate) bass_moving_average: f32,
    pub(crate) beat_pulse: f32,
    pub(crate) last_beat_time: Instant,
    pub(crate) treble_moving_average: f32,
    pub(crate) treble_pulse: f32,
    pub(crate) last_treble_time: Instant,
    pub(crate) theme: ThemeLayout,
    pub(crate) a_weighting_curve: Vec<f32>,
    pub(crate) frequency_bin_ranges: Vec<(usize, usize)>,
    pub(crate) waveform_bin_ranges: Vec<(usize, usize)>,
    pub(crate) lyric_bounce_value: f32,
    pub(crate) lyric_bounce_velocity: f32,
    pub(crate) cached_track_str: String,
    pub(crate) cached_track_hash: u64,
    pub(crate) cached_weather_str: String,
    pub(crate) cached_weather_hash: u64,
    pub(crate) current_lyric_idx: usize,
    pub(crate) lyric_scroll_offset: f32,
    pub(crate) video_frame_buffer: Vec<u8>,
    pub(crate) album_art_pad_buffer: Vec<u8>,
    // --- Cached Performance Values ---
    pub(crate) primary_text_color: [f32; 4],
    pub(crate) secondary_text_color: [f32; 4],
    pub(crate) audio_max_energy: f32,
    pub(crate) audio_base_energy: f32,
    pub(crate) is_waveform_style: bool,
    pub(crate) bass_bin_range: (usize, usize),
    pub(crate) treble_bin_range: (usize, usize),
    pub(crate) vis_target_colors: ([f32; 3], [f32; 3]),
    pub(crate) vis_prev_colors: ([f32; 3], [f32; 3]),
    pub(crate) art_target_color: [f32; 3],
    pub(crate) art_prev_color: [f32; 3],
}

impl Renderer {
    pub async fn new(
        wayland_manager: &WaylandManager,
        state: AppState,
        show_lyrics_tx: tokio::sync::watch::Sender<bool>,
    ) -> Result<Self> {
        let fps = state.config.fps;
        let current_fps = fps;

        info!("Initialising wgpu self...");

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
            .ok_or_else(|| anyhow::anyhow!("No suitable GPU adapter found"))?;

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
        ) = super::pipelines::create_album_art_pipeline(&device, &queue, config_format);
        let (
            ambient_pipeline,
            ambient_bind_group,
            ambient_uniform_buffer,
            custom_bg_uniform_buffer,
        ) = super::pipelines::create_ambient_pipeline(&device, config_format);
        let (
            particle_buffer,
            weather_compute_uniform_buffer,
            weather_compute_bind_group,
            weather_compute_pipeline,
            weather_render_bind_group,
            weather_render_pipeline,
        ) = super::pipelines::create_weather_pipelines(&device, &queue, config_format);
        let theme = ThemeLayout::load(&state.config.audio.style);
        let a_weighting_curve = super::utils::build_a_weighting_curve(state.config.audio.bands);
        let frequency_bin_ranges =
            super::utils::build_frequency_bin_ranges(state.config.audio.bands);
        let waveform_bin_ranges = super::utils::build_waveform_bin_ranges(state.config.audio.bands);
        let is_waveform_style = state.config.audio.style == "waveform";

        // Optimization: Pre-calculate the FFT bin ranges for beat and treble detection
        // to avoid redundant math on every single audio frame (typically 60-100 times per second).
        let sample_rate = 48000.0f32;
        let fft_size = 2048.0f32;
        let freq_per_bin = sample_rate / fft_size;

        let bass_bin_range = (
            (20.0 / freq_per_bin).floor() as usize,
            (120.0 / freq_per_bin).ceil() as usize,
        );
        let treble_bin_range = (
            (3000.0 / freq_per_bin).floor() as usize,
            (8000.0 / freq_per_bin).ceil() as usize,
        );

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
            text_buffers: Vec::new(),
            current_outputs_cache: Vec::new(),
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
            current_custom_bg_texture: None,
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
            show_lyrics_tx,
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
            cached_track_hash: 0,
            cached_weather_str: String::new(),
            cached_weather_hash: 0,
            current_lyric_idx: 0,
            lyric_scroll_offset: 0.0,
            video_frame_buffer: Vec::new(),
            album_art_pad_buffer: Vec::new(),
            primary_text_color: [1.0, 1.0, 1.0, 1.0],
            secondary_text_color: [1.0, 1.0, 1.0, 0.7],
            audio_max_energy: 0.0,
            audio_base_energy: 0.0,
            is_waveform_style,
            bass_bin_range,
            treble_bin_range,
            vis_target_colors: ([1.0, 0.2, 0.5], [0.2, 0.5, 1.0]),
            vis_prev_colors: ([1.0, 0.2, 0.5], [0.2, 0.5, 1.0]),
            art_target_color: [0.1, 0.1, 0.1],
            art_prev_color: [0.1, 0.1, 0.1],
        };

        let path = renderer
            .state
            .config
            .appearance
            .resolved_background_path()
            .await;
        renderer.current_bg_path = path.clone();
        renderer.load_custom_background(path.as_deref());
        renderer.update_theme_colors();

        info!("Renderer initialised at {}fps", fps);
        Ok(renderer)
    }

    pub async fn run(
        &mut self,
        mut event_rx: Receiver<Event>,
        mut wayland_manager: WaylandManager,
        is_visible_tx: tokio::sync::watch::Sender<bool>,
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
            let _ = is_visible_tx.send(!occluded);

            wayland_manager.dispatch_events()?;

            self.current_outputs_cache.clear();
            self.current_outputs_cache.extend(wayland_manager.outputs());
            let current_outputs = &self.current_outputs_cache;
            if wayland_manager.app_data.configuration_serial != last_config_serial {
                last_config_serial = wayland_manager.app_data.configuration_serial;
                info!(
                    "Monitor configuration changed ({} outputs), rebuilding GPU surfaces...",
                    current_outputs.len()
                );

                self.outputs.clear();
                wayland_manager.cleanup_dead_windows();

                for info in current_outputs {
                    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: info.raw_display_handle(),
                        raw_window_handle: info.raw_window_handle(),
                    };
                    let surface = unsafe { self.instance.create_surface_unsafe(target) }
                        .map_err(|e| anyhow::anyhow!("Failed to recreate surface: {}", e))?;

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
                if let Event::ConfigUpdated(ref config, _) = event {
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
                super::draw::draw_frame(self, &mut wayland_manager, delta)?;
            }

            // Tell wgpu to process internal garbage collection.
            // If we don't call this when output.present() is skipped (e.g. monitor asleep or occluded),
            // dropped textures and command buffers will queue up indefinitely and cause an OOM crash!
            self.device.poll(wgpu::Maintain::Poll);
        }
    }

    async fn handle_event(&mut self, event: Event) {
        use super::utils::hash_str;
        match event {
            Event::ConfigUpdated(config, theme_layout) => {
                let _ = self.show_lyrics_tx.send(config.audio.show_lyrics);

                let new_bg = config.appearance.resolved_background_path().await;
                if new_bg != self.current_bg_path {
                    self.load_custom_background(new_bg.as_deref());
                    self.current_bg_path = new_bg.clone();
                }

                if config.audio.bands != self.state.config.audio.bands {
                    self.state.audio_bands = vec![0.0; config.audio.bands].into_boxed_slice();
                    self.state.audio_waveform = vec![0.0; config.audio.bands].into_boxed_slice();
                    self.a_weighting_curve =
                        super::utils::build_a_weighting_curve(config.audio.bands);
                    self.frequency_bin_ranges =
                        super::utils::build_frequency_bin_ranges(config.audio.bands);
                    self.waveform_bin_ranges =
                        super::utils::build_waveform_bin_ranges(config.audio.bands);
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
                self.theme = *theme_layout;
                self.state.config = *config;
                self.update_theme_colors();

                // Optimization: Clear and shrink the text buffer cache on config updates to ensure
                // changes like font family or size are applied immediately and memory is reclaimed.
                self.text_buffer_cache.clear();
                self.text_buffer_cache.shrink_to_fit();

                self.is_waveform_style = self.state.config.audio.style == "waveform";
                self.update_weather_string();
                info!("Live settings applied!");
            }
            Event::TrackChanged(mut track) => {
                self.text_buffer_cache.clear(); // Free old shaped lyrics from memory!
                self.text_buffer_cache.shrink_to_fit();

                // Optimization: Don't shrink staging buffers to fit on track changes;
                // keep the allocations ready for the next track's album art or video loops.
                // Recreate SwashCache to flush its internal rasterized glyph memory
                self.swash_cache = SwashCache::new();
                self.text_renderer.glyph_cache.clear();
                self.text_renderer.glyph_cache.shrink_to_fit();
                self.text_renderer.cache_x = 0;
                self.text_renderer.cache_y = 0;
                self.text_renderer.cache_row_height = 0;

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
                self.cached_track_hash = hash_str(&self.cached_track_str);
                self.state.previous_palette = self
                    .state
                    .current_track
                    .as_ref()
                    .and_then(|t| t.palette.clone());
                self.state.current_track = Some(*track);
                self.update_theme_colors();
                self.update_text_colors();
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

            Event::BackgroundVideoFrame(frame) => {
                self.update_background_video_frame(&frame);
            }

            Event::CanvasVideoFrame(frame) => {
                self.update_canvas_video_frame(&frame);
            }

            Event::PlayerShutDown => {
                self.cached_track_str.clear();
                self.cached_track_hash = 0;
                self.text_buffer_cache.clear();
                self.text_buffer_cache.shrink_to_fit();
                self.state.previous_palette = self
                    .state
                    .current_track
                    .as_ref()
                    .and_then(|t| t.palette.clone());
                self.album_art_bg_bind_group = None;
                self.album_art_fg_bind_group = None;
                self.current_album_texture = None;
                self.state.has_album_art = false;
                self.state.current_track = None;
                self.update_theme_colors();
                self.update_text_colors();
                self.state.is_playing = false;
                self.current_lyric_idx = 0;
                self.lyric_scroll_offset = 0.0;
                self.state.begin_transition();

                // Free the padding buffers back to the OS allocator on idle
                self.video_frame_buffer.clear();
                self.video_frame_buffer.shrink_to_fit();
                self.album_art_pad_buffer.clear();
                self.album_art_pad_buffer.shrink_to_fit();
            }

            Event::PlaybackPosition(pos) => {
                self.state.playback_position = pos;
            }

            Event::AudioFrame { bands, waveform } => {
                let smoothing = self.state.config.audio.smoothing;
                let inv_smoothing = 1.0 - smoothing;
                let target_len = self.state.audio_bands.len();

                let bands_len = bands.len();

                // --- Smart Beat Detection ---
                // We focus strictly on the low-end frequencies (e.g. 20Hz - 120Hz)
                // Using pre-calculated ranges to avoid redundant math.
                let (bass_min, bass_max) = self.bass_bin_range;
                let bass_slice = &bands[bass_min..=bass_max.min(bands_len.saturating_sub(1))];

                let current_bass = if !bass_slice.is_empty() {
                    bass_slice.iter().sum::<f32>() / bass_slice.len() as f32
                } else {
                    0.0
                };

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
                let (treble_min, treble_max) = self.treble_bin_range;
                let treble_slice = &bands[treble_min..=treble_max.min(bands_len.saturating_sub(1))];

                let current_treble = if !treble_slice.is_empty() {
                    treble_slice.iter().sum::<f32>() / treble_slice.len() as f32
                } else {
                    0.0
                };

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

                let mut total_energy = 0.0;
                // Optimization: Use zipped iterators instead of manual indexing
                // to eliminate bounds checking and enable auto-vectorization.
                for (current, (&(bin_lo, bin_hi), &a_weighting_norm)) in
                    self.state.audio_bands.iter_mut().zip(
                        self.frequency_bin_ranges
                            .iter()
                            .zip(&self.a_weighting_curve),
                    )
                {
                    let max_val =
                        bands
                            .get(bin_lo..bin_hi.min(bands_len))
                            .map_or(0.0, |slice: &[f32]| {
                                slice
                                    .iter()
                                    .fold(0.0f32, |acc, &val| if val > acc { val } else { acc })
                            });

                    let target = (max_val * a_weighting_norm * 2.5).clamp(0.0, 1.0);

                    // Optimization: Use more efficient lerp formula a + (b - a) * t
                    // and use pre-calculated inv_smoothing.
                    let diff = target - *current;
                    if target > *current {
                        *current += diff * 0.8;
                    } else {
                        *current += diff * inv_smoothing;
                    }
                    total_energy += *current;
                }

                // Optimization: Calculate audio_base_energy during the bands loop to avoid a second pass.
                self.state.audio_energy = if target_len > 0 {
                    total_energy / target_len as f32
                } else {
                    0.0
                };
                self.audio_base_energy = self.state.audio_energy * 5.0;

                if self.state.audio_waveform.len() != target_len {
                    self.state.audio_waveform = vec![0.0; target_len].into_boxed_slice();
                }

                let wave_len = waveform.len();
                let mut max_energy = 0.0f32;
                // Optimization: Use zipped iterators for the waveform smoothing loop.
                for (current, &(start, end)) in self
                    .state
                    .audio_waveform
                    .iter_mut()
                    .zip(self.waveform_bin_ranges.iter())
                {
                    let mut peak = 0.0f32;
                    let mut peak_abs = 0.0f32;
                    if let Some(slice) = waveform.get(start..end.min(wave_len)) {
                        for &val in slice {
                            let val_abs: f32 = val.abs();
                            if val_abs > peak_abs {
                                peak_abs = val_abs;
                                peak = val;
                            }
                        }
                    }

                    // Optimization: Track max absolute energy during the waveform loop to avoid a separate pass.
                    if peak_abs > max_energy {
                        max_energy = peak_abs;
                    }

                    *current += (peak - *current) * inv_smoothing;
                }
                self.audio_max_energy = max_energy;
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
            // Optimization: Re-use the existing buffer if possible. resize(..., 0) only zero-fills
            // newly-allocated space, so by skipping .clear() at the end of the previous frame,
            // we avoid zeroing the entire buffer every single frame.
            if self.album_art_pad_buffer.len() < required_size {
                self.album_art_pad_buffer.resize(required_size, 0);
            }

            // Optimization: Use exact chunks and zip to eliminate manual bounds checking
            // and index arithmetic, allowing LLVM to auto-vectorize the memory copy.
            for (dst_row, src_row) in self.album_art_pad_buffer[..required_size]
                .chunks_exact_mut(padded_bytes_per_row as usize)
                .zip(rgba.as_raw().chunks_exact(unpadded_bytes_per_row as usize))
            {
                dst_row[..unpadded_bytes_per_row as usize].copy_from_slice(src_row);
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

    fn update_canvas_video_frame(&mut self, rgba: &image::RgbaImage) {
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
                    // Optimization: Skip .clear() to avoid redundant zero-filling by .resize()
                    if self.video_frame_buffer.len() < required_size {
                        self.video_frame_buffer.resize(required_size, 0);
                    }

                    // Optimization: Use exact chunks and zip to eliminate manual bounds checking
                    // and index arithmetic, allowing LLVM to auto-vectorize the memory copy.
                    for (dst_row, src_row) in self.video_frame_buffer[..required_size]
                        .chunks_exact_mut(padded_bytes_per_row as usize)
                        .zip(rgba.as_raw().chunks_exact(unpadded_bytes_per_row as usize))
                    {
                        dst_row[..unpadded_bytes_per_row as usize].copy_from_slice(src_row);
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

    fn update_background_video_frame(&mut self, rgba: &image::RgbaImage) {
        if let Some(texture) = &self.current_custom_bg_texture {
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
                    // Optimization: Skip .clear() to avoid redundant zero-filling by .resize()
                    if self.video_frame_buffer.len() < required_size {
                        self.video_frame_buffer.resize(required_size, 0);
                    }

                    // Optimization: Use exact chunks and zip to eliminate manual bounds checking
                    // and index arithmetic, allowing LLVM to auto-vectorize the memory copy.
                    for (dst_row, src_row) in self.video_frame_buffer[..required_size]
                        .chunks_exact_mut(padded_bytes_per_row as usize)
                        .zip(rgba.as_raw().chunks_exact(unpadded_bytes_per_row as usize))
                    {
                        dst_row[..unpadded_bytes_per_row as usize].copy_from_slice(src_row);
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
        self.load_custom_background_from_image(rgba);
    }

    /// Optimization: Recalculate theme-derived colors only when specific events occur
    /// (track change, config update) instead of on every frame in the rendering loop.
    fn update_theme_colors(&mut self) {
        let get_vis_colors =
            |palette: Option<&[[f32; 3]]>, theme: &ThemeLayout| -> ([f32; 3], [f32; 3]) {
                let top = theme.visualiser.color_top;
                let bottom = theme.visualiser.color_bottom;

                if let (Some(top_val), Some(bottom_val)) = (top, bottom) {
                    (top_val, bottom_val)
                } else {
                    match palette {
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
                }
            };

        let get_art_color = |palette: Option<&[[f32; 3]]>| -> [f32; 3] {
            palette
                .and_then(|p| p.first())
                .copied()
                .unwrap_or([0.1, 0.1, 0.1])
        };

        // Update Visualizer colors
        self.vis_prev_colors = get_vis_colors(self.state.previous_palette.as_deref(), &self.theme);
        self.vis_target_colors = get_vis_colors(
            self.state
                .current_track
                .as_ref()
                .and_then(|t| t.palette.as_deref()),
            &self.theme,
        );

        // Update Album Art colors
        self.art_prev_color = get_art_color(self.state.previous_palette.as_deref());
        self.art_target_color = get_art_color(
            self.state
                .current_track
                .as_ref()
                .and_then(|t| t.palette.as_deref()),
        );
    }

    pub fn load_custom_background(&mut self, path: Option<&str>) {
        let Some(path) = path else {
            self.custom_bg_bind_group = None;
            self.current_custom_bg_texture = None;
            return;
        };

        info!("Loading custom background from {}", path);
        let img = match image::open(path) {
            Ok(i) => i.to_rgba8(),
            Err(e) => {
                warn!("Failed to load custom background: {}", e);
                self.custom_bg_bind_group = None;
                self.current_custom_bg_texture = None;
                return;
            }
        };

        self.load_custom_background_from_image(&img);
    }

    pub fn load_custom_background_from_image(&mut self, img: &image::RgbaImage) {
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
            // Optimization: Avoid redundant zero-filling by reuse of the pad buffer
            if self.album_art_pad_buffer.len() < required_size {
                self.album_art_pad_buffer.resize(required_size, 0);
            }

            // Optimization: Use exact chunks and zip to eliminate manual bounds checking
            // and index arithmetic, allowing LLVM to auto-vectorize the memory copy.
            for (dst_row, src_row) in self.album_art_pad_buffer[..required_size]
                .chunks_exact_mut(padded_bytes_per_row as usize)
                .zip(img.as_raw().chunks_exact(unpadded_bytes_per_row as usize))
            {
                dst_row[..unpadded_bytes_per_row as usize].copy_from_slice(src_row);
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
        self.current_custom_bg_texture = Some(texture);
    }

    fn update_text_colors(&mut self) {
        let palette = self
            .state
            .current_track
            .as_ref()
            .and_then(|t| t.palette.as_deref());

        let text_bg_color = palette
            .and_then(|p| p.first())
            .copied()
            .unwrap_or([0.1, 0.1, 0.1]);
        let text_accent = palette
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
            self.primary_text_color = [tint[0], tint[1], tint[2], 1.0];
            self.secondary_text_color = [tint[0], tint[1], tint[2], 0.7];
        } else {
            // Light text for dark backgrounds, lightly tinted with the accent color
            let tint = [
                text_accent[0] * 0.3 + 0.7,
                text_accent[1] * 0.3 + 0.7,
                text_accent[2] * 0.3 + 0.7,
            ];
            self.primary_text_color = [tint[0], tint[1], tint[2], 1.0];
            self.secondary_text_color = [tint[0], tint[1], tint[2], 0.45];
        }
    }

    fn update_weather_string(&mut self) {
        use super::utils::hash_str;
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
            self.cached_weather_hash = hash_str(&self.cached_weather_str);
        } else {
            self.cached_weather_str.clear();
            self.cached_weather_hash = 0;
        }
    }
}
