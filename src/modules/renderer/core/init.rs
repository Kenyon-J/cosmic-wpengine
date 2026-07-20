use super::*;
impl Renderer {
    pub async fn new(
        wayland_manager: &WaylandManager,
        state: AppState,
        show_lyrics_tx: tokio::sync::watch::Sender<bool>,
    ) -> Result<Self> {
        // .max(1) mirrors Config::sanitise: fps = 0 would panic in
        // Duration::from_secs_f64(inf) below.
        let fps = state.config.fps.max(1);
        let current_fps = fps;

        info!("Initialising wgpu self...");

        let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_desc.backends = wgpu::Backends::VULKAN | wgpu::Backends::GL;
        let instance = wgpu::Instance::new(instance_desc);

        let outputs_info: Vec<_> = wayland_manager.outputs().collect();
        if outputs_info.is_empty() {
            anyhow::bail!("No Wayland outputs found to render to");
        }

        let mut surfaces = Vec::new();
        for info in &outputs_info {
            let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(info.raw_display_handle()),
                raw_window_handle: info.raw_window_handle(),
            };
            surfaces.push(unsafe { instance.create_surface_unsafe(target) }?);
        }

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surfaces[0]),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|e| anyhow::anyhow!("No suitable GPU adapter found: {}", e))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("COSMIC Wallpaper Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let mut outputs = Vec::new();
        for (info, surface) in outputs_info.into_iter().zip(surfaces) {
            outputs.push(configure_surface(
                surface,
                &adapter,
                &device,
                info.width,
                info.height,
            ));
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
        let kawase_blur = crate::modules::renderer::blur::KawaseBlur::new(&device);
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
        let is_waveform_style = state.config.audio.style == "waveform";
        let audio = crate::modules::renderer::audio_analysis::AudioAnalysis::new(
            state.config.audio.bands,
            state.config.audio.smoothing,
        );

        let mut renderer = Self {
            instance,
            adapter,
            device,
            queue,
            outputs,
            surface_format: config_format,
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
            current_album_size: None,
            album_art_sampler,
            custom_bg_avg_color: None,
            kawase_blur,
            album_blur_chain: None,
            custom_bg_blur_chain: None,
            ambient_pipeline,
            ambient_bind_group,
            ambient_uniform_buffer,
            custom_bg_uniform_buffer,
            custom_bg_bind_group: None,
            current_custom_bg_texture: None,
            current_bg: None,
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
            audio,
            theme,
            lyric_bounce_value: 0.0,
            lyric_bounce_velocity: 0.0,
            pending_art_deadline: None,
            art_fade: 1.0,
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
            text_color_diff: [0.0, 0.0, 0.0, 0.3],
            active_particles: 0,
            weather_gravity: 0.5,
            weather_wind_x: 0.1,
            weather_type: 0,
            is_weather_active: false,
            is_waveform_style,
            vis_target_colors: ([1.0, 0.2, 0.5], [0.2, 0.5, 1.0]),
            vis_prev_colors: ([1.0, 0.2, 0.5], [0.2, 0.5, 1.0]),
            art_target_color: [0.1, 0.1, 0.1],
            art_prev_color: [0.1, 0.1, 0.1],
            album_art_aspect: 1.0,
            custom_bg_aspect: 1.0,
            last_occluded: None,
        };

        let bg = renderer.state.config.appearance.resolved_background().await;
        renderer.load_resolved_background(bg.as_ref());
        renderer.current_bg = bg;
        renderer.update_theme_colors();
        renderer.update_weather_state();

        info!("Renderer initialised at {}fps", fps);
        Ok(renderer)
    }
}
