use super::text::{PositionedBuffer, TextRenderer, TextVertex};
use super::types::ArtUniforms;
use crate::modules::colour::{
    ensure_contrast, ensure_contrast_blended, lerp_colour, relative_luminance, time_to_sky_colour,
};
use crate::modules::config::{ArtShape, TextAlign, VisAlign, VisShape, WallpaperMode};
use crate::modules::event::WeatherCondition;
use crate::modules::state::SceneHint;
use crate::modules::wayland::WaylandManager;
use anyhow::Result;
use cosmic_text::{self, Attrs, Family, Metrics, Shaping};
use tracing::warn;

pub(crate) fn draw_frame(
    renderer: &mut super::Renderer,
    wayland_manager: &mut WaylandManager,
    delta: f32,
) -> Result<()> {
    let _scene = renderer.state.scene_description();

    let audio_data = match renderer.state.config.audio.style.as_str() {
        "waveform" => &renderer.state.audio_waveform,
        _ => &renderer.state.audio_bands,
    };

    let force_weather = renderer.state.config.mode == WallpaperMode::Weather;
    let force_vis = renderer.state.config.mode == WallpaperMode::AudioVisualiser;
    let force_art = renderer.state.config.mode == WallpaperMode::AlbumArt;

    let is_waveform_style = renderer.state.config.audio.style == "waveform";

    let max_energy = audio_data
        .iter()
        .fold(0.0f32, |a: f32, &b: &f32| a.max(b.abs()));
    let has_audio = (max_energy > 0.001 || force_vis)
        && !force_weather
        && !force_art
        && (renderer.state.current_track.is_some() || force_vis);

    let base_energy = if renderer.state.audio_bands.is_empty() {
        0.0
    } else {
        (renderer.state.audio_bands.iter().sum::<f32>() / renderer.state.audio_bands.len() as f32)
            * 5.0
    };
    // Combine the base volume energy with our snappy treble pulse, strictly capped to prevent blown out flashing
    let audio_energy = (base_energy * 0.3 + renderer.treble_pulse * 0.4).clamp(0.0, 1.0);

    // --- IMPORTANT FIX ---
    // The old state check can fail due to subtle race conditions.
    // The most robust way to check for media is to see if the GPU resources for it exist.
    let has_media_check_state = renderer.state.has_album_art;
    let has_media_check_gpu = renderer.album_art_fg_bind_group.is_some();
    if has_media_check_gpu && !has_media_check_state {
        warn!("Album art visibility check mismatch! State: false, GPU: true. Using GPU state.");
    }

    // Decouple art visibility from force_vis so you can layer the visualizer AND the album art!
    let show_art_fg =
        (has_media_check_gpu || force_art) && renderer.state.config.appearance.show_album_art;
    let show_art_bg =
        (has_media_check_gpu || force_art) && renderer.state.config.appearance.album_art_background;
    let show_color_bg = (has_media_check_gpu || force_art)
        && renderer.state.config.appearance.album_color_background;

    let clear_colour = get_clear_colour(renderer);
    // Use our new smart audio-reactive beat detector instead of the generic timer
    let pulse = renderer.beat_pulse;

    let is_n7 = renderer.state.config.audio.style == "n7";

    let reaper_tint = if renderer
        .state
        .current_track
        .as_ref()
        .is_some_and(|t| t.album.as_ref().contains("Mass Effect 3"))
    {
        (base_energy * 0.6).clamp(0.0, 0.8)
    } else {
        0.0
    };

    let is_weather_active = renderer.state.config.weather.enabled
        && !renderer.state.config.weather.hide_effects
        && renderer.state.weather.as_ref().is_some_and(|w| {
            matches!(
                w.condition,
                WeatherCondition::Rain | WeatherCondition::Snow | WeatherCondition::Thunderstorm
            )
        });

    let active_particles = if is_weather_active {
        if let Some(weather) = &renderer.state.weather {
            match weather.condition {
                WeatherCondition::Rain => 800,
                WeatherCondition::Thunderstorm => 1500,
                WeatherCondition::Snow => 2500,
                _ => 0,
            }
        } else {
            0
        }
    } else {
        0
    };

    if is_weather_active && active_particles > 0 {
        // --- Dispatch Weather Compute Shader ---
        // Only spend GPU time running particle physics if weather is actually visible!
        let mut wind_x = 0.1f32;
        let mut gravity = 0.5f32;

        if let Some(weather) = &renderer.state.weather {
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
        renderer
            .queue
            .write_buffer(&renderer.weather_compute_uniform_buffer, 0, compute_bytes);

        let mut compute_encoder =
            renderer
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Compute Encoder"),
                });
        {
            let mut compute_pass =
                compute_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Weather Compute Pass"),
                    timestamp_writes: None,
                });
            compute_pass.set_pipeline(&renderer.weather_compute_pipeline);
            compute_pass.set_bind_group(0, &renderer.weather_compute_bind_group, &[]);
            let workgroups = ((active_particles as f32) / 64.0).ceil() as u32;
            if workgroups > 0 {
                compute_pass.dispatch_workgroups(workgroups, 1, 1);
            }
        }
        renderer
            .queue
            .submit(std::iter::once(compute_encoder.finish()));
    }

    if has_audio {
        let bands_bytes = unsafe {
            std::slice::from_raw_parts(
                audio_data.as_ptr() as *const u8,
                audio_data.len() * std::mem::size_of::<f32>(),
            )
        };
        renderer
            .queue
            .write_buffer(&renderer.visualiser_pass.bands_buffer, 0, bands_bytes);
    }

    // Optimization: Pre-calculate common render values out of the monitor loop
    // to avoid redundant calculations for each output.

    // 1. Pre-calculate Visualizer colors
    let (top_col, bottom_col) = if has_audio {
        let get_colors = |palette: Option<&[[f32; 3]]>| -> ([f32; 3], [f32; 3]) {
            if is_n7 {
                return ([0.91, 0.0, 0.0], [1.0, 1.0, 1.0]); // N7 Crimson and stark white
            }
            let top = renderer.theme.visualiser.color_top;
            let bottom = renderer.theme.visualiser.color_bottom;

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
            renderer
                .state
                .current_track
                .as_ref()
                .and_then(|t| t.palette.as_deref()),
        );
        if renderer.state.transition_progress < 1.0 {
            let prev_colors = get_colors(renderer.state.previous_palette.as_deref());
            let t = renderer.state.transition_progress;
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
    let mut art_tint_color = if show_art_fg || show_art_bg || show_color_bg {
        renderer
            .state
            .current_track
            .as_ref()
            .map(|track| {
                let target_color = track
                    .palette
                    .as_deref()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or([0.1, 0.1, 0.1]);
                if renderer.state.transition_progress < 1.0 {
                    let prev_color = renderer
                        .state
                        .previous_palette
                        .as_deref()
                        .and_then(|p| p.first())
                        .copied()
                        .unwrap_or([0.1, 0.1, 0.1]);
                    lerp_colour(prev_color, target_color, renderer.state.transition_progress)
                } else {
                    target_color
                }
            })
            .unwrap_or([0.1, 0.1, 0.1])
    } else {
        [0.1, 0.1, 0.1]
    };

    if reaper_tint > 0.0 {
        art_tint_color = lerp_colour(art_tint_color, [0.8, 0.0, 0.0], reaper_tint);
    }

    let elapsed = renderer.start_time.elapsed().as_secs_f32();

    // 3. Pre-calculate Sky colors
    let sky_color_data = if renderer.custom_bg_bind_group.is_none() {
        let mut weather_type = 0u32;
        let sky = time_to_sky_colour(renderer.state.time_of_day);
        let mut final_sky = if let Some(weather) = &renderer.state.weather {
            if renderer.state.config.weather.enabled {
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
            }
        } else {
            sky
        };

        if reaper_tint > 0.0 {
            final_sky = lerp_colour(final_sky, [0.8, 0.0, 0.0], reaper_tint);
        }
        Some((elapsed, weather_type, final_sky))
    } else {
        None
    };

    let mut last_text_params = None;
    let mut last_uniform_res = None;

    // 4. Pre-calculate Text colors (luminance and tinting)
    let (primary_text, secondary_text) = if is_n7 {
        ([1.0, 1.0, 1.0, 1.0], [0.9, 0.9, 0.9, 0.8]) // Force white text for N7 theme
    } else {
        let text_bg_color = if show_art_bg || show_color_bg {
            art_tint_color
        } else if renderer.custom_bg_bind_group.is_some() {
            [0.1, 0.1, 0.1] // Rely on drop shadow for custom wallpapers
        } else if let Some((_, _, sky)) = sky_color_data {
            sky
        } else {
            [0.1, 0.1, 0.1]
        };

        let text_accent = renderer
            .state
            .current_track
            .as_ref()
            .and_then(|t| t.palette.as_deref())
            .and_then(|p| p.get(1).or_else(|| p.first()))
            .copied()
            .unwrap_or([1.0, 1.0, 1.0]);

        let l_bg = relative_luminance(text_bg_color);
        let base_tint = if l_bg > 0.179 {
            [
                text_accent[0] * 0.3,
                text_accent[1] * 0.3,
                text_accent[2] * 0.3,
            ]
        } else {
            [
                text_accent[0] * 0.3 + 0.7,
                text_accent[1] * 0.3 + 0.7,
                text_accent[2] * 0.3 + 0.7,
            ]
        };

        // WCAG 2.0 AA requires a contrast ratio of at least 4.5:1 for normal text.
        let primary_rgb = ensure_contrast(base_tint, text_bg_color, 4.5);
        // Secondary text uses a 0.7 alpha fade. ensure_contrast_blended guarantees it still hits >= 3.0:1!
        let secondary_rgb = ensure_contrast_blended(base_tint, text_bg_color, 0.7, 3.0);

        (
            [primary_rgb[0], primary_rgb[1], primary_rgb[2], 1.0],
            [secondary_rgb[0], secondary_rgb[1], secondary_rgb[2], 0.7],
        )
    };

    let map_align = |a: &TextAlign| -> cosmic_text::Align {
        match a {
            TextAlign::Left => cosmic_text::Align::Left,
            TextAlign::Center => cosmic_text::Align::Center,
            TextAlign::Right => cosmic_text::Align::Right,
        }
    };

    let family = renderer
        .state
        .config
        .appearance
        .font_family
        .as_deref()
        .map_or(Family::SansSerif, Family::Name);
    let attrs = Attrs::new().family(family);

    for (i, gpu_out) in renderer.outputs.iter_mut().enumerate() {
        if wayland_manager.is_frame_pending(i) {
            continue; // The compositor hasn't shown the last frame yet (e.g., hidden behind a window)
        }

        let output = match gpu_out.surface.get_current_texture() {
            Ok(texture) => texture,
            Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                warn!("wgpu surface outdated or lost, reconfiguring...");
                gpu_out.surface.configure(&renderer.device, &gpu_out.config);
                continue;
            }
            Err(wgpu::SurfaceError::Timeout) => {
                warn!("wgpu surface timeout, skipping frame...");
                continue;
            }
            Err(e) => anyhow::bail!("Failed to get current texture: {:?}", e),
        };

        wayland_manager.mark_frame_rendered(i); // Request the next frame callback

        let current_res = (gpu_out.config.width, gpu_out.config.height);

        // 1. Process visualizer uniforms
        if has_audio && last_uniform_res != Some(current_res) {
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
            let shape_u32 = match renderer.theme.visualiser.shape {
                VisShape::Circular => 0,
                VisShape::Linear => 1,
                VisShape::Square => 2,
            };
            let align_u32 = match renderer.theme.visualiser.align {
                VisAlign::Left => 0,
                VisAlign::Center => 1,
                VisAlign::Right => 2,
            };
            let vis_uniforms = VisUniforms {
                res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                bands: renderer.state.config.audio.bands as u32,
                pulse: pulse * 2.0, // Multiplier guarantees visible beat effects
                top: top_col,
                bottom: bottom_col,
                pos_size_rot: [
                    renderer.theme.visualiser.position[0],
                    renderer.theme.visualiser.position[1],
                    renderer.theme.visualiser.size,
                    renderer.theme.visualiser.rotation.to_radians(),
                ],
                amplitude: renderer.theme.visualiser.amplitude,
                shape: shape_u32,
                time: renderer.start_time.elapsed().as_secs_f32(),
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
            renderer
                .queue
                .write_buffer(&renderer.visualiser_pass.uniform_buffer, 0, vis_bytes);
        }

        // 2. Process album art uniforms
        if (show_art_fg || show_art_bg || show_color_bg) && last_uniform_res != Some(current_res) {
            if let Some(_track) = &renderer.state.current_track {
                let color = art_tint_color;
                let bg_mode = if show_color_bg {
                    3
                } else if renderer.state.config.appearance.disable_blur {
                    2
                } else {
                    0
                };
                // Fade out the album art background completely when transparent background is enabled
                let bg_alpha_val = 1.0 - renderer.state.transparent_fade;

                let bg_uniforms = ArtUniforms {
                    color_and_transition: [
                        color[0],
                        color[1],
                        color[2],
                        renderer.state.transition_progress,
                    ],
                    res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                    art_position: [0.5, 0.5],
                    audio_energy,
                    mode: bg_mode,
                    bg_alpha: bg_alpha_val,
                    art_size: 1.0,
                    shape: 0,
                    blur_opacity: renderer.state.config.appearance.blur_opacity,
                    image_res: [
                        renderer
                            .current_album_texture
                            .as_ref()
                            .map(|t| t.size().width as f32)
                            .unwrap_or(1.0),
                        renderer
                            .current_album_texture
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
                renderer
                    .queue
                    .write_buffer(&renderer.album_art_bg_uniform_buffer, 0, bg_bytes);

                let mut art_position = renderer.theme.album_art.position;
                let mut art_size = renderer.theme.album_art.size;
                let mut art_shape = renderer.theme.album_art.shape;

                // If the circular visualiser is active, dynamically override the album art
                // layout to fit perfectly inside of it.
                if has_audio && renderer.theme.visualiser.shape == VisShape::Circular {
                    art_position = renderer.theme.visualiser.position;
                    art_size = renderer.theme.visualiser.size;
                    art_shape = ArtShape::Circular; // Force circular shape to match
                }

                let fg_uniforms = ArtUniforms {
                    color_and_transition: [
                        color[0],
                        color[1],
                        color[2],
                        renderer.state.transition_progress,
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
                        renderer
                            .current_album_texture
                            .as_ref()
                            .map(|t| t.size().width as f32)
                            .unwrap_or(1.0),
                        renderer
                            .current_album_texture
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
                renderer
                    .queue
                    .write_buffer(&renderer.album_art_fg_uniform_buffer, 0, fg_bytes);
            }
        }

        if last_uniform_res != Some(current_res) {
            if renderer.custom_bg_bind_group.is_some() {
                // 4. Process custom background uniforms
                let bg_mode = if renderer.state.config.appearance.disable_blur {
                    2
                } else {
                    0
                };
                let bg_alpha_val = 1.0 - renderer.state.transparent_fade;

                let mut custom_bg_color = [1.0, 1.0, 1.0];
                if reaper_tint > 0.0 {
                    custom_bg_color = lerp_colour(custom_bg_color, [0.8, 0.0, 0.0], reaper_tint);
                }

                let custom_bg_uniforms = ArtUniforms {
                    color_and_transition: [
                        custom_bg_color[0],
                        custom_bg_color[1],
                        custom_bg_color[2],
                        1.0,
                    ],
                    res: [gpu_out.config.width as f32, gpu_out.config.height as f32],
                    art_position: [0.5, 0.5],
                    audio_energy,
                    mode: bg_mode,
                    bg_alpha: bg_alpha_val,
                    art_size: 1.0,
                    shape: 0,
                    blur_opacity: renderer.state.config.appearance.blur_opacity,
                    image_res: [
                        renderer
                            .current_custom_bg_size
                            .map(|s| s.0 as f32)
                            .unwrap_or(1.0),
                        renderer
                            .current_custom_bg_size
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
                renderer
                    .queue
                    .write_buffer(&renderer.custom_bg_uniform_buffer, 0, cbg_bytes);
            } else if let Some((elapsed, weather_type, final_sky)) = sky_color_data {
                // 3. Process ambient uniforms
                let bg_alpha_val = 1.0 - renderer.state.transparent_fade;

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
                renderer
                    .queue
                    .write_buffer(&renderer.ambient_uniform_buffer, 0, amb_bytes);
            }
        }

        last_uniform_res = Some(current_res);

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

        let current_text_params = (
            gpu_out.config.width,
            gpu_out.config.height,
            scale_factor as u32,
        );

        if last_text_params != Some(current_text_params) {
            for p_buf in renderer.text_buffers.drain(..) {
                if let Some((_, evicted)) = renderer
                    .text_buffer_cache
                    .push(p_buf.text_key, p_buf.buffer)
                {
                    if renderer.text_buffer_pool.len() < 20 {
                        renderer.text_buffer_pool.push(evicted);
                    }
                }
            }

            if renderer.state.config.audio.show_lyrics {
                if let Some(track) = &renderer.state.current_track {
                    if let Some(lyrics) = &track.lyrics {
                        let base_font_size =
                            (logical_height * 0.04).clamp(16.0, 48.0) * scale_factor;
                        let active_font_size = base_font_size * 1.5;
                        let line_spacing = active_font_size * 1.2;

                        let start_idx = renderer.current_lyric_idx.saturating_sub(2);
                        let end_idx = (renderer.current_lyric_idx + 2).min(lyrics.len());

                        for i in start_idx..=end_idx {
                            if i == 0 || i > lyrics.len() {
                                continue;
                            }

                            let lyric_line = &lyrics[i - 1];
                            // Compute exactly how far this string is from the "current active string"
                            let dist = (i as f32)
                                - (renderer.current_lyric_idx as f32)
                                - renderer.lyric_scroll_offset;
                            let abs_dist = dist.abs();

                            if abs_dist > 2.0 {
                                continue;
                            }

                            let center_weight = (1.0 - abs_dist).clamp(0.0, 1.0);

                            let scale = base_font_size
                                + (active_font_size - base_font_size) * center_weight;
                            let final_scale = scale
                                + (renderer.lyric_bounce_value * 8.0 * scale_factor)
                                    * center_weight;

                            let render_scale = final_scale / active_font_size;
                            let bounce_y =
                                (renderer.lyric_bounce_value * 12.0 * scale_factor) * center_weight;
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
                                let text_key = format!("{i}_{}", lyric_line.text);
                                let mut buffer = renderer
                                    .text_buffer_cache
                                    .pop(&text_key)
                                    .unwrap_or_else(|| {
                                        let mut b =
                                            renderer.text_buffer_pool.pop().unwrap_or_else(|| {
                                                cosmic_text::Buffer::new(
                                                    &mut renderer.font_system,
                                                    metrics,
                                                )
                                            });
                                        b.set_metrics(&mut renderer.font_system, metrics);
                                        b.set_size(&mut renderer.font_system, width_f, height_f);
                                        b.set_text(
                                            &mut renderer.font_system,
                                            &lyric_line.text,
                                            attrs,
                                            Shaping::Advanced,
                                        );
                                        b
                                    });
                                buffer.set_metrics(&mut renderer.font_system, metrics);
                                buffer.set_size(&mut renderer.font_system, width_f, height_f);

                                let align = map_align(&renderer.theme.lyrics.align);
                                buffer.lines.iter_mut().for_each(
                                    |line: &mut cosmic_text::BufferLine| {
                                        line.set_align(Some(align));
                                    },
                                );

                                let pos = [
                                    renderer.theme.lyrics.position[0] * width_f,
                                    renderer.theme.lyrics.position[1] * height_f + y_pos,
                                ];

                                renderer.text_buffers.push(PositionedBuffer {
                                    buffer,
                                    text_key,
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

            if renderer.state.current_track.is_some() && !renderer.cached_track_str.is_empty() {
                let info_scale = (logical_height * 0.025).clamp(16.0, 36.0) * scale_factor;
                let metrics = Metrics::new(info_scale, info_scale * 1.2);
                let text_key = format!("{i}_{}", renderer.cached_track_str);
                let mut buffer = renderer
                    .text_buffer_cache
                    .pop(&text_key)
                    .unwrap_or_else(|| {
                        let mut b = renderer.text_buffer_pool.pop().unwrap_or_else(|| {
                            cosmic_text::Buffer::new(&mut renderer.font_system, metrics)
                        });
                        b.set_metrics(&mut renderer.font_system, metrics);
                        b.set_size(&mut renderer.font_system, width_f, height_f);
                        b.set_text(
                            &mut renderer.font_system,
                            &renderer.cached_track_str,
                            attrs,
                            Shaping::Advanced,
                        );
                        b
                    });
                buffer.set_metrics(&mut renderer.font_system, metrics);
                buffer.set_size(&mut renderer.font_system, width_f, height_f);
                let final_color = [
                    secondary_text[0],
                    secondary_text[1],
                    secondary_text[2],
                    secondary_text[3],
                ];
                let align = map_align(&renderer.theme.track_info.align);
                buffer
                    .lines
                    .iter_mut()
                    .for_each(|line: &mut cosmic_text::BufferLine| {
                        line.set_align(Some(align));
                    });
                let pos = [
                    renderer.theme.track_info.position[0] * width_f,
                    renderer.theme.track_info.position[1] * height_f,
                ];
                renderer.text_buffers.push(PositionedBuffer {
                    buffer,
                    text_key,
                    pos,
                    color: final_color,
                    scale: 1.0,
                    align,
                });
            }

            if renderer.state.config.weather.enabled
                && renderer.state.weather.is_some()
                && !renderer.cached_weather_str.is_empty()
            {
                let weather_scale = (logical_height * 0.02).clamp(14.0, 24.0) * scale_factor;
                let metrics = Metrics::new(weather_scale, weather_scale * 1.2);
                let text_key = format!("{i}_{}", renderer.cached_weather_str);
                let mut buffer = renderer
                    .text_buffer_cache
                    .pop(&text_key)
                    .unwrap_or_else(|| {
                        let mut b = renderer.text_buffer_pool.pop().unwrap_or_else(|| {
                            cosmic_text::Buffer::new(&mut renderer.font_system, metrics)
                        });
                        b.set_metrics(&mut renderer.font_system, metrics);
                        b.set_size(&mut renderer.font_system, width_f, height_f);
                        b.set_text(
                            &mut renderer.font_system,
                            &renderer.cached_weather_str,
                            attrs,
                            Shaping::Advanced,
                        );
                        b
                    });
                buffer.set_metrics(&mut renderer.font_system, metrics);
                buffer.set_size(&mut renderer.font_system, width_f, height_f);
                let final_color = [
                    secondary_text[0],
                    secondary_text[1],
                    secondary_text[2],
                    secondary_text[3],
                ];
                let align = map_align(&renderer.theme.weather.align);
                buffer
                    .lines
                    .iter_mut()
                    .for_each(|line: &mut cosmic_text::BufferLine| {
                        line.set_align(Some(align));
                    });
                let pos = [
                    renderer.theme.weather.position[0] * width_f,
                    renderer.theme.weather.position[1] * height_f,
                ];
                renderer.text_buffers.push(PositionedBuffer {
                    buffer,
                    text_key,
                    pos,
                    color: final_color,
                    scale: 1.0,
                    align,
                });
            }

            // Prepare text vertices
            TextRenderer::prepare_text(
                &mut renderer.text_renderer,
                &renderer.queue,
                &mut renderer.font_system,
                &mut renderer.swash_cache,
                renderer.text_buffers.as_mut(),
                width_f,
                height_f,
            );

            let vertices_bytes = unsafe {
                std::slice::from_raw_parts(
                    renderer.text_renderer.cpu_vertices.as_ptr() as *const u8,
                    renderer.text_renderer.cpu_vertices.len() * std::mem::size_of::<TextVertex>(),
                )
            };
            let indices_bytes = unsafe {
                std::slice::from_raw_parts(
                    renderer.text_renderer.cpu_indices.as_ptr() as *const u8,
                    renderer.text_renderer.cpu_indices.len() * std::mem::size_of::<u32>(),
                )
            };

            if renderer.text_renderer.vertex_capacity < renderer.text_renderer.cpu_vertices.len() {
                renderer.text_renderer.vertex_capacity = renderer
                    .text_renderer
                    .cpu_vertices
                    .len()
                    .next_power_of_two();
                renderer.text_renderer.vertices =
                    renderer.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Text Vertex Buffer"),
                        size: (renderer.text_renderer.vertex_capacity
                            * std::mem::size_of::<TextVertex>())
                            as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
            }
            if renderer.text_renderer.index_capacity < renderer.text_renderer.cpu_indices.len() {
                renderer.text_renderer.index_capacity =
                    renderer.text_renderer.cpu_indices.len().next_power_of_two();
                renderer.text_renderer.indices =
                    renderer.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Text Index Buffer"),
                        size: (renderer.text_renderer.index_capacity * std::mem::size_of::<u32>())
                            as u64,
                        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
            }

            renderer
                .queue
                .write_buffer(&renderer.text_renderer.vertices, 0, vertices_bytes);

            renderer
                .queue
                .write_buffer(&renderer.text_renderer.indices, 0, indices_bytes);
            renderer.text_renderer.num_indices = renderer.text_renderer.cpu_indices.len() as u32;
        }

        last_text_params = Some(current_text_params);

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = renderer
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
            if is_n7 {
                // Draw nothing; get_clear_colour is already forcing a pitch black background!
            } else if show_art_bg || show_color_bg {
                if let Some(bind_group) = &renderer.album_art_bg_bind_group {
                    render_pass.set_pipeline(&renderer.album_art_pipeline);
                    render_pass.set_bind_group(0, bind_group, &[]);
                    render_pass.draw(0..3, 0..1);
                }
            } else if let Some(bind_group) = &renderer.custom_bg_bind_group {
                // Custom Desktop Wallpaper Background (Frosted Glass)
                render_pass.set_pipeline(&renderer.album_art_pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.draw(0..3, 0..1);
            } else {
                // Ambient Procedural Sky
                render_pass.set_pipeline(&renderer.ambient_pipeline);
                render_pass.set_bind_group(0, &renderer.ambient_bind_group, &[]);
                render_pass.draw(0..3, 0..1);
            }

            // --- Overlay Layers ---
            if is_weather_active && active_particles > 0 {
                render_pass.set_pipeline(&renderer.weather_render_pipeline);
                render_pass.set_bind_group(0, &renderer.weather_render_bind_group, &[]);
                render_pass.draw(0..6, 0..active_particles); // 6 vertices per quad
            }

            if has_audio {
                render_pass.set_pipeline(&renderer.visualiser_pass.pipeline);
                render_pass.set_bind_group(0, &renderer.visualiser_pass.bind_group, &[]);
                let instance_count = if is_waveform_style {
                    1
                } else if renderer.theme.visualiser.shape == VisShape::Linear {
                    renderer.state.config.audio.bands as u32
                } else {
                    renderer.state.config.audio.bands as u32 * 2
                };
                render_pass.draw(0..6, 0..instance_count);
            }

            if show_art_fg {
                if let Some(bind_group) = &renderer.album_art_fg_bind_group {
                    render_pass.set_pipeline(&renderer.album_art_pipeline);
                    render_pass.set_bind_group(0, bind_group, &[]);
                    render_pass.draw(0..3, 0..1);
                }
            }

            // --- Text Rendering ---
            render_pass.set_pipeline(&renderer.text_renderer.pipeline);
            render_pass.set_bind_group(0, &renderer.text_renderer.bind_group, &[]);
            render_pass.set_vertex_buffer(0, renderer.text_renderer.vertices.slice(..));
            render_pass.set_index_buffer(
                renderer.text_renderer.indices.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.draw_indexed(0..renderer.text_renderer.num_indices, 0, 0..1);
        }

        renderer.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    for p_buf in renderer.text_buffers.drain(..) {
        if let Some((_, evicted)) = renderer
            .text_buffer_cache
            .push(p_buf.text_key, p_buf.buffer)
        {
            if renderer.text_buffer_pool.len() < 20 {
                renderer.text_buffer_pool.push(evicted);
            }
        }
    }

    Ok(())
}

pub(crate) fn get_clear_colour(renderer: &super::Renderer) -> wgpu::Color {
    if renderer.state.config.audio.style == "n7" {
        return wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
    }
    if renderer.state.config.appearance.transparent_background {
        return wgpu::Color::TRANSPARENT;
    }

    let scene = match renderer.state.config.mode {
        WallpaperMode::Weather => SceneHint::Ambient,
        WallpaperMode::AlbumArt => SceneHint::AlbumArt,
        WallpaperMode::AudioVisualiser => SceneHint::AudioVisualiser,
        WallpaperMode::Auto => renderer.state.scene_description(),
    };

    match scene {
        SceneHint::Ambient => {
            let sky = time_to_sky_colour(renderer.state.time_of_day);
            let final_sky = if let Some(weather) = &renderer.state.weather {
                if renderer.state.config.weather.enabled {
                    match weather.condition {
                        WeatherCondition::Rain | WeatherCondition::Thunderstorm => {
                            lerp_colour(sky, [0.2, 0.2, 0.25], 0.6)
                        }
                        WeatherCondition::Snow => lerp_colour(sky, [0.8, 0.85, 0.9], 0.4),
                        _ => sky,
                    }
                } else {
                    sky
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
