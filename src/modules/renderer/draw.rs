use super::frame_params::{get_uv_transform, FrameParams};
use super::text::{PositionedBuffer, TextCacheKey, TextRenderer, TextVertex};
use super::types::{AmbUniforms, ArtUniforms, VisUniforms};
use crate::modules::wayland::WaylandManager;
use anyhow::Result;
use cosmic_text::{self, Attrs, Family, Metrics, Shaping};
use tracing::warn;

/// Builds (or refreshes from cache) a shaped text buffer for one on-screen
/// text element, applies this frame's alignment, and appends the result to
/// `renderer.text_buffers`. Shared by the lyric, track-info, and weather
/// blocks in `draw_frame`, which differ only in font metrics, position, and
/// source text.
///
/// `set_align` resets a line's shaped layout as a side effect, so we track
/// whether it actually changed and only re-shape when it did: reshaping
/// unconditionally is wasted work, and skipping it after a real alignment
/// change leaves `layout_runs()` empty and the text invisible.
/// Takes the three `Renderer` fields it needs individually, rather than
/// `&mut Renderer`, so callers inside the per-output loop (which already
/// holds `renderer.outputs` mutably borrowed via its iterator) can still
/// call this: borrowing disjoint fields is fine, but a function taking the
/// whole struct is opaque to the borrow checker and would conflict.
#[allow(clippy::too_many_arguments)]
fn prepare_text_buffer(
    text_buffer_cache: &mut std::collections::HashMap<
        TextCacheKey,
        cosmic_text::Buffer,
        rustc_hash::FxBuildHasher,
    >,
    font_system: &mut cosmic_text::FontSystem,
    text_key: TextCacheKey,
    text: &str,
    attrs: &Attrs,
    metrics: Metrics,
    align: cosmic_text::Align,
    width_f: f32,
    height_f: f32,
) -> cosmic_text::Buffer {
    let mut buffer = text_buffer_cache.remove(&text_key).unwrap_or_else(|| {
        let mut b = cosmic_text::Buffer::new(font_system, metrics);
        b.set_metrics(font_system, metrics);
        b.set_size(font_system, Some(width_f), Some(height_f));
        b.set_text(font_system, text, attrs, Shaping::Advanced, Some(align));
        b
    });

    // Re-apply metrics/size even for a cached buffer: a monitor swap can
    // change DPI/resolution without changing the text content or its cache key.
    buffer.set_metrics(font_system, metrics);
    buffer.set_size(font_system, Some(width_f), Some(height_f));

    let mut realigned = false;
    buffer
        .lines
        .iter_mut()
        .for_each(|line: &mut cosmic_text::BufferLine| {
            realigned |= line.set_align(Some(align));
        });
    if realigned {
        buffer.shape_until_scroll(font_system, false);
    }

    buffer
}

#[expect(clippy::too_many_arguments)]
fn push_text_buffer(
    text_buffer_cache: &mut std::collections::HashMap<
        TextCacheKey,
        cosmic_text::Buffer,
        rustc_hash::FxBuildHasher,
    >,
    font_system: &mut cosmic_text::FontSystem,
    text_buffers: &mut Vec<PositionedBuffer>,
    text_key: TextCacheKey,
    text: &str,
    attrs: &Attrs,
    metrics: Metrics,
    align: cosmic_text::Align,
    width_f: f32,
    height_f: f32,
    pos: [f32; 2],
    color: [f32; 4],
    scale: f32,
) {
    let buffer = prepare_text_buffer(
        text_buffer_cache,
        font_system,
        text_key,
        text,
        attrs,
        metrics,
        align,
        width_f,
        height_f,
    );

    text_buffers.push(PositionedBuffer {
        buffer,
        text_key,
        pos,
        color,
        scale,
        align,
    });
}

pub(crate) fn draw_frame(
    renderer: &mut super::Renderer,
    wayland_manager: &mut WaylandManager,
    delta: f32,
) -> Result<()> {
    // One pass of pure derivation over &renderer (phase 1 of the renderer
    // decomposition); the loop below is then free to take mutable borrows.
    // Destructured into same-named locals so the per-output code reads
    // exactly as it did when these were computed inline.
    let FrameParams {
        has_audio,
        audio_energy,
        show_art_fg,
        show_art_bg,
        show_color_bg,
        clear_colour,
        is_weather_active,
        active_particles,
        top_col,
        bottom_col,
        art_tint_color,
        elapsed,
        track_hash,
        weather_hash,
        sky_color_data,
        vis_shape_u32,
        vis_align_u32,
        vis_pos_size_rot,
        is_waveform_u32,
        album_art_bg_mode,
        album_art_bg_alpha,
        album_art_aspect,
        album_art_fg_pos,
        album_art_fg_size,
        album_art_fg_shape,
        custom_bg_mode,
        custom_bg_alpha,
        custom_bg_aspect,
        blur_opacity,
        visualiser_instance_count,
        lyric_start_idx,
        lyric_end_idx,
        fg_k1,
        fg_k2,
        fg_k3,
        fg_scale_y,
        fg_offset_y,
        secondary_text,
        text_color_diff,
        lyrics_align,
        track_info_align,
        weather_align,
        font_family,
        lyric_bounce,
        beat_pulse_mul,
    } = FrameParams::compute(renderer);

    if is_weather_active && active_particles > 0 {
        // --- Dispatch Weather Compute Shader ---
        // Only spend GPU time running particle physics if weather is actually visible!
        let wind_x = renderer.weather_wind_x;
        let gravity = renderer.weather_gravity;

        let compute_uniforms = [delta, wind_x, gravity, 0.0f32];
        renderer.queue.write_buffer(
            &renderer.weather_compute_uniform_buffer,
            0,
            bytemuck::bytes_of(&compute_uniforms),
        );

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
        let audio_data: &[f32] = if renderer.is_waveform_style {
            &renderer.state.audio_waveform
        } else {
            &renderer.state.audio_bands
        };
        renderer.queue.write_buffer(
            &renderer.visualiser_pass.bands_buffer,
            0,
            bytemuck::cast_slice(audio_data),
        );
    }

    // The owned family from FrameParams rebuilt into a borrow-free Attrs:
    // push_text_buffer() below needs `&mut renderer` and `&attrs` in the
    // same call, which a borrow through renderer would reject.
    let family = font_family
        .as_deref()
        .map_or(Family::SansSerif, Family::Name);
    let attrs = Attrs::new().family(family);

    // Prevent unbound memory growth for weather/ambient setups left running for days.
    // The cache limit is 500 to accommodate multi-monitor setups; no
    // shrink_to_fit() in the hot path to avoid expensive reallocations.
    if renderer.text_buffer_cache.len() > 500 {
        renderer.text_buffer_cache.clear();
    }

    let mut last_text_params = None;
    let mut last_uniform_res = None;

    for (i_idx, gpu_out) in renderer.outputs.iter_mut().enumerate() {
        let i = i_idx as u32;
        if wayland_manager.is_frame_pending(i as usize) {
            continue; // The compositor hasn't shown the last frame yet (e.g., hidden behind a window)
        }

        let output = match gpu_out.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                warn!("wgpu surface outdated or lost, reconfiguring...");
                gpu_out.surface.configure(&renderer.device, &gpu_out.config);
                continue;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                warn!("wgpu surface timeout or occluded, skipping frame...");
                continue;
            }
            e => anyhow::bail!("Failed to get current texture: {:?}", e),
        };

        wayland_manager.mark_frame_rendered(i as usize); // Request the next frame callback

        let current_res = (gpu_out.config.width, gpu_out.config.height);
        let screen_res_f = [gpu_out.config.width as f32, gpu_out.config.height as f32];
        let screen_aspect = screen_res_f[0] / screen_res_f[1];

        // 1. Process visualizer uniforms
        if has_audio && last_uniform_res != Some(current_res) {
            let vis_uniforms = VisUniforms {
                res: screen_res_f,
                bands: renderer.state.config.audio.bands as u32,
                pulse: beat_pulse_mul, // Multiplier guarantees visible beat effects
                top: top_col,
                bottom: bottom_col,
                pos_size_rot: vis_pos_size_rot,
                amplitude: renderer.theme.visualiser.amplitude,
                shape: vis_shape_u32,
                time: elapsed,
                align: vis_align_u32,
                is_waveform: is_waveform_u32,
                _padding: [0; 3],
            };
            renderer.queue.write_buffer(
                &renderer.visualiser_pass.uniform_buffer,
                0,
                bytemuck::bytes_of(&vis_uniforms),
            );
        }

        // 2. Process album art uniforms
        if (show_art_fg || show_art_bg || show_color_bg) && last_uniform_res != Some(current_res) {
            if let Some(_track) = &renderer.state.current_track {
                let color = art_tint_color;

                let bg_uv_transform = get_uv_transform(0, screen_aspect, album_art_aspect);

                let bg_uniforms = ArtUniforms {
                    color_and_transition: [
                        color[0],
                        color[1],
                        color[2],
                        renderer.state.transition_progress,
                    ],
                    uv_transform: bg_uv_transform,
                    art_position: [0.5, 0.5],
                    blur_step: [0.0, 0.0], // retired: the blur is pre-rendered offscreen
                    audio_energy,
                    mode: album_art_bg_mode,
                    bg_alpha: album_art_bg_alpha,
                    art_size: 1.0,
                    shape: 0,
                    blur_opacity,
                    screen_aspect,
                    _padding: 0,
                };
                renderer.queue.write_buffer(
                    &renderer.album_art_bg_uniform_buffer,
                    0,
                    bytemuck::bytes_of(&bg_uniforms),
                );

                // Optimization: Use pre-calculated constants to minimize arithmetic in the monitor loop
                let fg_scale_x = screen_aspect * fg_k1;
                let fg_offset_x = fg_k2 - screen_aspect * fg_k3;
                let fg_uv_transform = [fg_scale_x, fg_scale_y, fg_offset_x, fg_offset_y];

                let fg_uniforms = ArtUniforms {
                    color_and_transition: [
                        color[0],
                        color[1],
                        color[2],
                        renderer.state.transition_progress,
                    ],
                    uv_transform: fg_uv_transform,
                    art_position: album_art_fg_pos,
                    blur_step: [0.0, 0.0], // FG art is never blurred
                    audio_energy,
                    mode: 1,
                    // The sharp foreground art only fades when stale art is
                    // being eased out after the fetch grace period expires.
                    bg_alpha: renderer.art_fade,
                    art_size: album_art_fg_size,
                    shape: album_art_fg_shape,
                    blur_opacity: 1.0,
                    screen_aspect,
                    _padding: 0,
                };
                renderer.queue.write_buffer(
                    &renderer.album_art_fg_uniform_buffer,
                    0,
                    bytemuck::bytes_of(&fg_uniforms),
                );
            }
        }

        if last_uniform_res != Some(current_res) {
            if renderer.custom_bg_bind_group.is_some() {
                let bg_uv_transform = get_uv_transform(0, screen_aspect, custom_bg_aspect);

                // 4. Process custom background uniforms
                let custom_bg_uniforms = ArtUniforms {
                    color_and_transition: [1.0, 1.0, 1.0, 1.0], // Don't tint the desktop wallpaper
                    uv_transform: bg_uv_transform,
                    art_position: [0.5, 0.5],
                    blur_step: [0.0, 0.0], // retired: the blur is pre-rendered offscreen
                    audio_energy,
                    mode: custom_bg_mode,
                    bg_alpha: custom_bg_alpha,
                    art_size: 1.0,
                    shape: 0,
                    blur_opacity,
                    screen_aspect,
                    _padding: 0,
                };
                renderer.queue.write_buffer(
                    &renderer.custom_bg_uniform_buffer,
                    0,
                    bytemuck::bytes_of(&custom_bg_uniforms),
                );
            } else if let Some((elapsed, weather_type, final_sky)) = sky_color_data {
                // 3. Process ambient uniforms
                let amb_uniforms = AmbUniforms {
                    res: screen_res_f,
                    time: elapsed,
                    weather: weather_type,
                    sky: [final_sky[0], final_sky[1], final_sky[2], 1.0],
                    bg_alpha: custom_bg_alpha, // Can reuse the same bg_alpha logic
                    _padding: [0.0; 3],
                };
                renderer.queue.write_buffer(
                    &renderer.ambient_uniform_buffer,
                    0,
                    bytemuck::bytes_of(&amb_uniforms),
                );
            }
        }

        last_uniform_res = Some(current_res);

        // --- Prepare Text for Rendering ---
        let width_f = screen_res_f[0];
        let height_f = screen_res_f[1];
        let scale_factor = wayland_manager
            .app_data
            .windows
            .get(i as usize)
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
                renderer
                    .text_buffer_cache
                    .insert(p_buf.text_key, p_buf.buffer);
            }

            if renderer.state.config.audio.show_lyrics {
                // Clone the small (~5-line) active window of lyric text up front,
                // ending the borrow through renderer.state.current_track before
                // push_text_buffer() below needs `&mut renderer`. This block only
                // runs on resolution/DPI change, so the clones are rare, not a
                // per-frame cost.
                let lyric_window: Option<Vec<(usize, Box<str>, u64)>> = renderer
                    .state
                    .current_track
                    .as_ref()
                    .and_then(|t| t.lyrics.as_ref())
                    .map(|lyrics| {
                        (lyric_start_idx..=lyric_end_idx)
                            .map(|line_idx| {
                                let l = &lyrics[line_idx - 1];
                                (line_idx, l.text.clone(), l.text_hash)
                            })
                            .collect()
                    });

                if let Some(lyric_window) = lyric_window {
                    let base_font_size = (logical_height * 0.04).clamp(16.0, 48.0)
                        * scale_factor
                        * renderer.theme.lyrics.size.clamp(0.25, 4.0);
                    let active_font_size = base_font_size * 1.5;
                    let inv_active_font_size = 1.0 / active_font_size;
                    let line_spacing = active_font_size * 1.2;

                    // Bound wrapping to the space actually visible between the
                    // lyrics anchor point and this monitor's own edge. Themes
                    // like "monstercat" anchor lyrics off-center (x=0.49, left
                    // aligned), so shaping against the full monitor width let
                    // long lines run past the right edge before cosmic-text
                    // ever wrapped them. Each monitor renders as an
                    // independent surface here (no cross-monitor spanning),
                    // so bounding to this monitor's own visible span is
                    // always correct, whether or not another display sits
                    // next to it.
                    let lyrics_anchor_x = renderer.theme.lyrics.position[0] * width_f;
                    let lyrics_wrap_width = match lyrics_align {
                        cosmic_text::Align::Left => width_f - lyrics_anchor_x,
                        cosmic_text::Align::Right => lyrics_anchor_x,
                        _ => 2.0 * lyrics_anchor_x.min(width_f - lyrics_anchor_x),
                    }
                    .max(active_font_size * 3.0);

                    // Pre-calculate bounce scalars for the current monitor's scale factor
                    let bounce_8_scaled = lyric_bounce * 8.0 * scale_factor;
                    let bounce_12_scaled = lyric_bounce * 12.0 * scale_factor;

                    let metrics = Metrics::new(active_font_size, active_font_size * 1.2);

                    // First pass: shape (or fetch from cache) each visible line and
                    // measure how many rows it wrapped to. A wrapped line occupies
                    // more than its one line_spacing slot, so the second pass shifts
                    // its neighbours apart to keep rows from overlapping.
                    struct ShapedLyric {
                        dist: f32,
                        render_scale: f32,
                        color: [f32; 4],
                        y_base: f32,
                        /// Vertical overflow beyond the line's nominal slot:
                        /// (rows - 1) * line height at this line's drawn scale.
                        extra: f32,
                        text_key: TextCacheKey,
                        buffer: cosmic_text::Buffer,
                    }
                    let mut shaped_lines: Vec<ShapedLyric> = Vec::with_capacity(5);

                    for (line_idx, text, text_hash) in lyric_window {
                        // Compute exactly how far this string is from the "current active string"
                        let dist = (line_idx as f32)
                            - (renderer.current_lyric_idx as f32)
                            - renderer.lyric_scroll_offset;
                        let abs_dist = dist.abs();

                        if abs_dist > 2.0 {
                            continue;
                        }

                        let center_weight = (1.0 - abs_dist).clamp(0.0, 1.0);

                        let scale =
                            base_font_size + (active_font_size - base_font_size) * center_weight;
                        let final_scale = scale + bounce_8_scaled * center_weight;

                        // The buffer was shaped/wrapped assuming font_size == active_font_size
                        // (see `metrics` above), so a render_scale above 1.0 - which the
                        // beat-reactive bounce can produce, since lyric_bounce_value is an
                        // unclamped spring - would draw the active line larger than the box it
                        // was just wrapped to fit, spilling back past the screen edge. Clamp to
                        // the shaped size; the pulse still reads clearly on the way up to it.
                        let render_scale = (final_scale * inv_active_font_size).min(1.0);
                        let bounce_y = bounce_12_scaled * center_weight;
                        let y_base = (dist * line_spacing) - bounce_y;

                        // Optimization: Use cached color difference to replace 4 subtractions with 1 multiply-add
                        let color = [
                            secondary_text[0] + text_color_diff[0] * center_weight,
                            secondary_text[1] + text_color_diff[1] * center_weight,
                            secondary_text[2] + text_color_diff[2] * center_weight,
                            secondary_text[3] + text_color_diff[3] * center_weight,
                        ];

                        // Fade out gracefully to prevent popping strings at top/bottom
                        let alpha_fade = (1.5 - abs_dist).clamp(0.0, 1.0);
                        let final_color = [color[0], color[1], color[2], color[3] * alpha_fade];

                        if final_color[3] > 0.01 {
                            let text_key = TextCacheKey::Lyric {
                                monitor: i,
                                line: line_idx as u32,
                                content_hash: text_hash,
                            };
                            let buffer = prepare_text_buffer(
                                &mut renderer.text_buffer_cache,
                                &mut renderer.font_system,
                                text_key,
                                &text,
                                &attrs,
                                metrics,
                                lyrics_align,
                                lyrics_wrap_width,
                                height_f,
                            );
                            let rows = buffer.layout_runs().count().max(1);
                            let extra = (rows - 1) as f32 * line_spacing * render_scale;
                            shaped_lines.push(ShapedLyric {
                                dist,
                                render_scale,
                                color: final_color,
                                y_base,
                                extra,
                                text_key,
                                buffer,
                            });
                        }
                    }

                    // Second pass: place the lines (already in top-to-bottom order).
                    // The weights keep the layout continuous while lines scroll:
                    // a wrapped line pushes everything below it down while it is at
                    // or below the active slot (weight fades in over dist -1 -> 0),
                    // and pulls itself plus everything above it up once it scrolls
                    // above the active slot, so its overflow never collides with
                    // the anchored active line.
                    let anchor_x = renderer.theme.lyrics.position[0] * width_f;
                    let anchor_y = renderer.theme.lyrics.position[1] * height_f;
                    let shifts: Vec<f32> = (0..shaped_lines.len())
                        .map(|idx| {
                            let mut shift = 0.0;
                            for line in &shaped_lines[..idx] {
                                shift += line.extra * (line.dist + 1.0).clamp(0.0, 1.0);
                            }
                            for line in &shaped_lines[idx..] {
                                shift -= line.extra * (-line.dist).clamp(0.0, 1.0);
                            }
                            shift
                        })
                        .collect();

                    for (line, shift) in shaped_lines.into_iter().zip(shifts) {
                        renderer.text_buffers.push(PositionedBuffer {
                            buffer: line.buffer,
                            text_key: line.text_key,
                            pos: [anchor_x, anchor_y + line.y_base + shift],
                            color: line.color,
                            scale: line.render_scale,
                            align: lyrics_align,
                        });
                    }
                }
            }

            if renderer.state.current_track.is_some() && !renderer.cached_track_str.is_empty() {
                let text = renderer.cached_track_str.clone();
                let info_scale = (logical_height * 0.025).clamp(16.0, 36.0)
                    * scale_factor
                    * renderer.theme.track_info.size.clamp(0.25, 4.0);
                let metrics = Metrics::new(info_scale, info_scale * 1.2);
                let text_key = TextCacheKey::Track {
                    monitor: i,
                    content_hash: track_hash,
                };
                let pos = [
                    renderer.theme.track_info.position[0] * width_f,
                    renderer.theme.track_info.position[1] * height_f,
                ];
                push_text_buffer(
                    &mut renderer.text_buffer_cache,
                    &mut renderer.font_system,
                    &mut renderer.text_buffers,
                    text_key,
                    &text,
                    &attrs,
                    metrics,
                    track_info_align,
                    width_f,
                    height_f,
                    pos,
                    secondary_text,
                    1.0,
                );
            }

            if renderer.state.config.weather.enabled
                && renderer.state.weather.is_some()
                && !renderer.cached_weather_str.is_empty()
            {
                let text = renderer.cached_weather_str.clone();
                let weather_scale = (logical_height * 0.02).clamp(14.0, 24.0)
                    * scale_factor
                    * renderer.theme.weather.size.clamp(0.25, 4.0);
                let metrics = Metrics::new(weather_scale, weather_scale * 1.2);
                let text_key = TextCacheKey::Weather {
                    monitor: i,
                    content_hash: weather_hash,
                };
                let pos = [
                    renderer.theme.weather.position[0] * width_f,
                    renderer.theme.weather.position[1] * height_f,
                ];
                push_text_buffer(
                    &mut renderer.text_buffer_cache,
                    &mut renderer.font_system,
                    &mut renderer.text_buffers,
                    text_key,
                    &text,
                    &attrs,
                    metrics,
                    weather_align,
                    width_f,
                    height_f,
                    pos,
                    secondary_text,
                    1.0,
                );
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

            let vertices_bytes: &[u8] = bytemuck::cast_slice(&renderer.text_renderer.cpu_vertices);
            let indices_bytes: &[u8] = bytemuck::cast_slice(&renderer.text_renderer.cpu_indices);

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
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_colour),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // --- Background Rendering ---
            // Simplified logic with clear precedence: Album Art > Custom BG > Ambient
            if show_art_bg || show_color_bg {
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
                render_pass.draw(0..6, 0..visualiser_instance_count);
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
        renderer.queue.present(output);
    }

    for p_buf in renderer.text_buffers.drain(..) {
        renderer
            .text_buffer_cache
            .insert(p_buf.text_key, p_buf.buffer);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};

    /// Regression test for long lyric lines spilling past the monitor edge
    /// under themes that anchor lyrics off-center (e.g. "monstercat", which
    /// uses position=[0.49, 0.72] with left alignment). Exercises the real
    /// cosmic-text shaping/wrapping the renderer relies on: shaping against
    /// the full monitor width lets a long line's shaped width exceed the
    /// space actually visible between the anchor and the screen edge,
    /// whereas shaping against that visible span (as draw_frame now does
    /// via `lyrics_wrap_width`) wraps the line instead.
    #[test]
    fn long_lyric_line_wraps_within_visible_span_not_full_monitor_width() {
        let mut font_system = FontSystem::new();
        let width_f = 2560.0_f32;
        let anchor_x = 0.49 * width_f;
        let available = width_f - anchor_x; // left-aligned: space to the right edge

        let font_size = 33.0_f32;
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let attrs = Attrs::new().family(Family::SansSerif);
        let text = "This is a deliberately long lyric line that would previously run past the right edge of the monitor before wrapping";

        let mut old_buf = Buffer::new(&mut font_system, metrics);
        old_buf.set_size(&mut font_system, Some(width_f), Some(1000.0));
        old_buf.set_text(&mut font_system, text, &attrs, Shaping::Advanced, None);
        let old_max_line_w = old_buf
            .layout_runs()
            .map(|r| r.line_w)
            .fold(0.0_f32, f32::max);
        assert!(
            old_max_line_w > available,
            "expected the pre-fix wrap width ({old_max_line_w}) to exceed the visible \
             span ({available}) so the bug this test guards actually reproduces"
        );

        let mut new_buf = Buffer::new(&mut font_system, metrics);
        new_buf.set_size(&mut font_system, Some(available), Some(1000.0));
        new_buf.set_text(&mut font_system, text, &attrs, Shaping::Advanced, None);
        let new_max_line_w = new_buf
            .layout_runs()
            .map(|r| r.line_w)
            .fold(0.0_f32, f32::max);
        let run_count = new_buf.layout_runs().count();

        assert!(
            new_max_line_w <= available + 1.0,
            "line width {new_max_line_w} still exceeds the visible span {available}"
        );
        assert!(
            run_count >= 2,
            "expected the long line to wrap onto multiple lines, got {run_count}"
        );
    }
}
