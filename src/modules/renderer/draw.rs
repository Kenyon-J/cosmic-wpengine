use super::core::{ArtLayer, BackgroundLayer};
use super::frame_params::{get_uv_transform, FrameParams};
use super::text::TextRenderer;
use super::types::{AmbUniforms, ArtUniforms, VisUniforms};
use crate::modules::visualiser_pass::VisualiserPass;
use crate::modules::wayland::WaylandManager;
use anyhow::Result;
use cosmic_text::{Attrs, Family};
use tracing::warn;

/// Writes this frame's uniform buffers (visualiser, album art, background/
/// ambient) for a render target of `width`x`height` - the other half of
/// what the per-output loop below does before calling `encode_frame`, and
/// the dev-only offscreen harness's fixed-scene counterpart to it (real
/// per-monitor uniforms are res/aspect-dependent, so the harness can't just
/// skip this the way it can skip `last_uniform_res`/`last_text_params`
/// dedup - there's no cache to reuse on a first and only frame). Same
/// borrow-checker reasoning as `encode_frame`: individual resources, not
/// `&Renderer`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_frame_uniforms(
    queue: &wgpu::Queue,
    visualiser_uniform_buffer: &wgpu::Buffer,
    art: &ArtLayer,
    background: &BackgroundLayer,
    width: u32,
    height: u32,
    has_audio: bool,
    has_track: bool,
    transition_progress: f32,
    audio_bands: u32,
    beat_pulse_mul: f32,
    top_col: [f32; 4],
    bottom_col: [f32; 4],
    vis_pos_size_rot: [f32; 4],
    visualiser_amplitude: f32,
    vis_shape_u32: u32,
    elapsed: f32,
    vis_align_u32: u32,
    is_waveform_u32: u32,
    show_art_fg: bool,
    show_art_bg: bool,
    show_color_bg: bool,
    art_tint_color: [f32; 3],
    album_art_aspect: f32,
    album_art_bg_mode: u32,
    audio_energy: f32,
    album_art_bg_alpha: f32,
    blur_opacity: f32,
    fg_k1: f32,
    fg_k2: f32,
    fg_k3: f32,
    fg_scale_y: f32,
    fg_offset_y: f32,
    album_art_fg_pos: [f32; 2],
    album_art_fg_size: f32,
    album_art_fg_shape: u32,
    custom_bg_aspect: f32,
    custom_bg_mode: u32,
    custom_bg_alpha: f32,
    sky_color_data: Option<(f32, u32, [f32; 3])>,
    bar_width_ratio: f32,
    cap_radius: f32,
    reflection: f32,
    led_segments: u32,
    peak_hold: bool,
    glow_strength: f32,
) {
    let screen_res_f = [width as f32, height as f32];
    let screen_aspect = screen_res_f[0] / screen_res_f[1];

    // 1. Process visualizer uniforms
    if has_audio {
        let vis_uniforms = VisUniforms {
            res: screen_res_f,
            bands: audio_bands,
            pulse: beat_pulse_mul, // Multiplier guarantees visible beat effects
            top: top_col,
            bottom: bottom_col,
            pos_size_rot: vis_pos_size_rot,
            amplitude: visualiser_amplitude,
            shape: vis_shape_u32,
            time: elapsed,
            align: vis_align_u32,
            is_waveform: is_waveform_u32,
            bar_width_ratio,
            cap_radius,
            reflection,
            led_segments,
            peak_hold: peak_hold as u32,
            glow_strength,
            _padding: 0,
        };
        queue.write_buffer(
            visualiser_uniform_buffer,
            0,
            bytemuck::bytes_of(&vis_uniforms),
        );
    }

    // 2. Process album art uniforms
    if (show_art_fg || show_art_bg || show_color_bg) && has_track {
        let color = art_tint_color;

        let bg_uv_transform = get_uv_transform(0, screen_aspect, album_art_aspect);

        let bg_uniforms = ArtUniforms {
            color_and_transition: [color[0], color[1], color[2], transition_progress],
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
        queue.write_buffer(&art.bg_uniform_buffer, 0, bytemuck::bytes_of(&bg_uniforms));

        // Optimization: Use pre-calculated constants to minimize arithmetic in the monitor loop
        let fg_scale_x = screen_aspect * fg_k1;
        let fg_offset_x = fg_k2 - screen_aspect * fg_k3;
        let fg_uv_transform = [fg_scale_x, fg_scale_y, fg_offset_x, fg_offset_y];

        let fg_uniforms = ArtUniforms {
            color_and_transition: [color[0], color[1], color[2], transition_progress],
            uv_transform: fg_uv_transform,
            art_position: album_art_fg_pos,
            blur_step: [0.0, 0.0], // FG art is never blurred
            audio_energy,
            mode: 1,
            // The sharp foreground art only fades when stale art is
            // being eased out after the fetch grace period expires.
            bg_alpha: art.fade,
            art_size: album_art_fg_size,
            shape: album_art_fg_shape,
            blur_opacity: 1.0,
            screen_aspect,
            _padding: 0,
        };
        queue.write_buffer(&art.fg_uniform_buffer, 0, bytemuck::bytes_of(&fg_uniforms));
    }

    if background.bind_group().is_some() {
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
        queue.write_buffer(
            &background.custom_bg_uniform_buffer,
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
        queue.write_buffer(
            &background.ambient_uniform_buffer,
            0,
            bytemuck::bytes_of(&amb_uniforms),
        );
    }
}

/// Encodes and submits one frame's render pass against `view` - phase 4 of
/// the renderer decomposition (`docs/PLAN-renderer-decomposition.md`).
/// Takes a bare `&wgpu::TextureView` rather than acquiring one from a
/// surface, and every GPU resource it draws with individually rather than
/// `&Renderer` (same borrow-checker reason as `prepare_text_buffer` above:
/// callers inside the per-output loop already hold `renderer.outputs`
/// mutably borrowed via its iterator, so a function taking the whole
/// struct would conflict where one taking its disjoint fields doesn't) -
/// so this same function can drive both the live per-monitor loop below
/// and a future offscreen `--render-frame` harness rendering to a bare
/// texture with no surface at all. Presenting (surface-specific) stays
/// the caller's job; this only encodes and submits.
#[allow(clippy::too_many_arguments)]
pub(crate) fn encode_frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: &wgpu::TextureView,
    width: u32,
    height: u32,
    album_art_pipeline: &wgpu::RenderPipeline,
    art: &ArtLayer,
    background: &BackgroundLayer,
    weather_render_pipeline: &wgpu::RenderPipeline,
    weather_render_bind_group: &wgpu::BindGroup,
    visualiser_pass: &VisualiserPass,
    text_renderer: &TextRenderer,
    clear_colour: wgpu::Color,
    show_art_bg: bool,
    show_color_bg: bool,
    show_art_fg: bool,
    is_weather_active: bool,
    active_particles: u32,
    has_audio: bool,
    visualiser_instance_count: u32,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some(&format!("Frame Encoder {width}x{height}")),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Main Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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
            if let Some(bind_group) = art.bg_bind_group() {
                render_pass.set_pipeline(album_art_pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.draw(0..3, 0..1);
            }
        } else if let Some(bind_group) = background.bind_group() {
            // Custom Desktop Wallpaper Background (Frosted Glass)
            render_pass.set_pipeline(album_art_pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        } else {
            // Ambient Procedural Sky
            render_pass.set_pipeline(&background.ambient_pipeline);
            render_pass.set_bind_group(0, &background.ambient_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        // --- Overlay Layers ---
        if is_weather_active && active_particles > 0 {
            render_pass.set_pipeline(weather_render_pipeline);
            render_pass.set_bind_group(0, weather_render_bind_group, &[]);
            render_pass.draw(0..6, 0..active_particles); // 6 vertices per quad
        }

        if has_audio {
            render_pass.set_pipeline(&visualiser_pass.pipeline);
            render_pass.set_bind_group(0, &visualiser_pass.bind_group, &[]);
            render_pass.draw(0..6, 0..visualiser_instance_count);
        }

        if show_art_fg {
            if let Some(bind_group) = art.fg_bind_group() {
                render_pass.set_pipeline(album_art_pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.draw(0..3, 0..1);
            }
        }

        // --- Text Rendering ---
        render_pass.set_pipeline(&text_renderer.pipeline);
        render_pass.set_bind_group(0, &text_renderer.bind_group, &[]);
        render_pass.set_vertex_buffer(0, text_renderer.vertices.slice(..));
        render_pass.set_index_buffer(text_renderer.indices.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..text_renderer.num_indices, 0, 0..1);
    }

    queue.submit(std::iter::once(encoder.finish()));
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
            &renderer.audio.waveform
        } else {
            &renderer.audio.bands
        };

        if renderer.last_audio_bands != audio_data {
            renderer.last_audio_bands.clear();
            renderer.last_audio_bands.extend_from_slice(audio_data);
            renderer.queue.write_buffer(
                &renderer.visualiser_pass.bands_buffer,
                0,
                bytemuck::cast_slice(audio_data),
            );
        }

        let peaks_data: &[f32] = &renderer.audio.peaks;
        if renderer.last_audio_peaks != peaks_data {
            renderer.last_audio_peaks.clear();
            renderer.last_audio_peaks.extend_from_slice(peaks_data);
            renderer.queue.write_buffer(
                &renderer.visualiser_pass.peaks_buffer,
                0,
                bytemuck::cast_slice(peaks_data),
            );
        }
    }

    // The owned family from FrameParams rebuilt into a borrow-free Attrs:
    // TextSubsystem::prepare() below needs `&mut renderer.text` and `&attrs`
    // in the same call, which a borrow through renderer would reject.
    let family = font_family
        .as_deref()
        .map_or(Family::SansSerif, Family::Name);
    let attrs = Attrs::new().family(family);

    renderer.text.evict_stale_cache();

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

        if last_uniform_res != Some(current_res) {
            write_frame_uniforms(
                &renderer.queue,
                &renderer.visualiser_pass.uniform_buffer,
                &renderer.art,
                &renderer.background,
                current_res.0,
                current_res.1,
                has_audio,
                renderer.state.current_track.is_some(),
                renderer.state.transition_progress,
                renderer.state.config.audio.bands as u32,
                beat_pulse_mul,
                top_col,
                bottom_col,
                vis_pos_size_rot,
                renderer.theme.visualiser.amplitude,
                vis_shape_u32,
                elapsed,
                vis_align_u32,
                is_waveform_u32,
                show_art_fg,
                show_art_bg,
                show_color_bg,
                art_tint_color,
                album_art_aspect,
                album_art_bg_mode,
                audio_energy,
                album_art_bg_alpha,
                blur_opacity,
                fg_k1,
                fg_k2,
                fg_k3,
                fg_scale_y,
                fg_offset_y,
                album_art_fg_pos,
                album_art_fg_size,
                album_art_fg_shape,
                custom_bg_aspect,
                custom_bg_mode,
                custom_bg_alpha,
                sky_color_data,
                renderer.theme.visualiser.bar_width_ratio,
                renderer.theme.visualiser.cap_radius,
                renderer.theme.visualiser.reflection,
                renderer.theme.visualiser.led_segments,
                renderer.theme.visualiser.peak_hold,
                renderer.theme.visualiser.glow_strength,
            );
        }

        last_uniform_res = Some(current_res);
        let screen_res_f = [gpu_out.config.width as f32, gpu_out.config.height as f32];

        // --- Prepare Text for Rendering ---
        let width_f = screen_res_f[0];
        let height_f = screen_res_f[1];
        let scale_factor = wayland_manager
            .app_data
            .windows
            .get(i as usize)
            .map(|w| w.scale_factor as f32)
            .unwrap_or(1.0);

        let current_text_params = (
            gpu_out.config.width,
            gpu_out.config.height,
            scale_factor as u32,
        );

        if last_text_params != Some(current_text_params) {
            // Clone the small (~5-line) active window of lyric text up
            // front, ending the borrow through renderer.state.current_track
            // before renderer.text.prepare() below needs `&mut renderer.text`
            // alongside other renderer fields. This block only runs on
            // resolution/DPI change, so the clones are rare, not a
            // per-frame cost.
            let lyric_window = if renderer.state.config.audio.show_lyrics {
                renderer
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
                            .collect::<Vec<_>>()
                    })
            } else {
                None
            }
            .map(|window| {
                (
                    window,
                    super::core::LyricPhysics {
                        current_lyric_idx: renderer.current_lyric_idx,
                        lyric_scroll_offset: renderer.lyric_scroll_offset,
                        lyric_bounce,
                    },
                )
            });

            let track_text = (renderer.state.current_track.is_some()
                && !renderer.cached_track_str.is_empty())
            .then_some((renderer.cached_track_str.as_str(), track_hash));

            let weather_text = (renderer.state.config.weather.enabled
                && renderer.state.weather.is_some()
                && !renderer.cached_weather_str.is_empty())
            .then_some((renderer.cached_weather_str.as_str(), weather_hash));

            renderer.text.prepare(
                &renderer.device,
                &renderer.queue,
                i,
                width_f,
                height_f,
                scale_factor,
                &attrs,
                lyric_window,
                renderer.theme.lyrics.size,
                renderer.theme.lyrics.position,
                lyrics_align,
                secondary_text,
                text_color_diff,
                track_text,
                renderer.theme.track_info.size,
                renderer.theme.track_info.position,
                track_info_align,
                weather_text,
                renderer.theme.weather.size,
                renderer.theme.weather.position,
                weather_align,
            );
        }

        last_text_params = Some(current_text_params);

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        encode_frame(
            &renderer.device,
            &renderer.queue,
            &view,
            current_res.0,
            current_res.1,
            &renderer.album_art_pipeline,
            &renderer.art,
            &renderer.background,
            &renderer.weather_render_pipeline,
            &renderer.weather_render_bind_group,
            &renderer.visualiser_pass,
            &renderer.text.text_renderer,
            clear_colour,
            show_art_bg,
            show_color_bg,
            show_art_fg,
            is_weather_active,
            active_particles,
            has_audio,
            visualiser_instance_count,
        );

        renderer.queue.present(output);
    }

    renderer.text.recycle_buffers();

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
