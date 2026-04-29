use super::*;
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
        ) = crate::modules::renderer::pipelines::create_album_art_pipeline(
            &device,
            &queue,
            config_format,
        );
        let (
            ambient_pipeline,
            ambient_bind_group,
            ambient_uniform_buffer,
            custom_bg_uniform_buffer,
        ) = crate::modules::renderer::pipelines::create_ambient_pipeline(&device, config_format);
        let (
            particle_buffer,
            weather_compute_uniform_buffer,
            weather_compute_bind_group,
            weather_compute_pipeline,
            weather_render_bind_group,
            weather_render_pipeline,
        ) = crate::modules::renderer::pipelines::create_weather_pipelines(
            &device,
            &queue,
            config_format,
        );
        let theme = ThemeLayout::load(&state.config.audio.style);
        let a_weighting_curve =
            crate::modules::renderer::utils::build_a_weighting_curve(state.config.audio.bands);
        let frequency_bin_ranges =
            crate::modules::renderer::utils::build_frequency_bin_ranges(state.config.audio.bands);
        let waveform_bin_ranges =
            crate::modules::renderer::utils::build_waveform_bin_ranges(state.config.audio.bands);
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
            text_buffer_cache: std::collections::HashMap::with_hasher(rustc_hash::FxBuildHasher),
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
}
