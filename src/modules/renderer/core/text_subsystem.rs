//! Text shaping/caching/rendering state, bundled into one owned unit -
//! phase 5 of the renderer decomposition
//! (`docs/PLAN-renderer-decomposition.md`). Replaces the three-field
//! workaround `prepare_text_buffer` used to need (`text_buffer_cache`,
//! `font_system`, and either `text_buffers` or the caller's own locals),
//! documented there as a deliberate borrow-checker dodge: a function taking
//! `&mut Renderer` while the per-output loop already holds
//! `renderer.outputs` mutably borrowed via its iterator would conflict,
//! but one taking `&mut TextSubsystem` (a single disjoint field) doesn't.

use super::super::text::{PositionedBuffer, TextCacheKey, TextRenderer};
use cosmic_text::{Attrs, Buffer, BufferLine, FontSystem, Metrics, Shaping, SwashCache};

pub(crate) struct TextSubsystem {
    pub(crate) font_system: FontSystem,
    pub(crate) swash_cache: SwashCache,
    pub(crate) text_renderer: TextRenderer,
    text_buffer_cache: std::collections::HashMap<TextCacheKey, Buffer, rustc_hash::FxBuildHasher>,
    text_buffers: Vec<PositionedBuffer>,
}

impl TextSubsystem {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        Ok(Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            text_renderer: TextRenderer::new(device, format)?,
            text_buffer_cache: std::collections::HashMap::with_hasher(rustc_hash::FxBuildHasher),
            text_buffers: Vec::new(),
        })
    }

    /// Unconditionally drops every cached shaped buffer - a config or
    /// track change can invalidate them all at once (font family/size
    /// changed, or the lyrics they belong to are gone), unlike
    /// `evict_stale_cache`'s size-triggered trim.
    pub(crate) fn clear_cache(&mut self) {
        self.text_buffer_cache.clear();
    }

    /// Prevents unbound memory growth for weather/ambient setups left
    /// running for days. The cache limit is 500 to accommodate
    /// multi-monitor setups; no `shrink_to_fit()` to avoid expensive
    /// reallocations in what's otherwise a per-frame check.
    pub(crate) fn evict_stale_cache(&mut self) {
        if self.text_buffer_cache.len() > 500 {
            self.text_buffer_cache.clear();
        }
    }

    /// Returns any buffers left over from the last prepare - drawn this
    /// frame but not needed again - to the reuse cache. Called both before
    /// re-shaping (so a resolution/DPI change starts from a full cache) and
    /// once after the whole per-output loop (so the frame's buffers aren't
    /// simply dropped).
    pub(crate) fn recycle_buffers(&mut self) {
        for p_buf in self.text_buffers.drain(..) {
            self.text_buffer_cache.insert(p_buf.text_key, p_buf.buffer);
        }
    }

    /// Builds (or refreshes from cache) a shaped text buffer for one
    /// on-screen text element, applying this frame's alignment.
    ///
    /// `set_align` resets a line's shaped layout as a side effect, so we
    /// track whether it actually changed and only re-shape when it did:
    /// reshaping unconditionally is wasted work, and skipping it after a
    /// real alignment change leaves `layout_runs()` empty and the text
    /// invisible.
    #[allow(clippy::too_many_arguments)]
    fn prepare_text_buffer(
        &mut self,
        text_key: TextCacheKey,
        text: &str,
        attrs: &Attrs,
        metrics: Metrics,
        align: cosmic_text::Align,
        width_f: f32,
        height_f: f32,
    ) -> Buffer {
        let mut buffer = self.text_buffer_cache.remove(&text_key).unwrap_or_else(|| {
            let mut b = Buffer::new(&mut self.font_system, metrics);
            b.set_metrics(&mut self.font_system, metrics);
            b.set_size(&mut self.font_system, Some(width_f), Some(height_f));
            b.set_text(
                &mut self.font_system,
                text,
                attrs,
                Shaping::Advanced,
                Some(align),
            );
            b
        });

        // Re-apply metrics/size even for a cached buffer: a monitor swap can
        // change DPI/resolution without changing the text content or its cache key.
        buffer.set_metrics(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(width_f), Some(height_f));

        let mut realigned = false;
        buffer.lines.iter_mut().for_each(|line: &mut BufferLine| {
            realigned |= line.set_align(Some(align));
        });
        if realigned {
            buffer.shape_until_scroll(&mut self.font_system, false);
        }

        buffer
    }

    #[allow(clippy::too_many_arguments)]
    fn push_text_buffer(
        &mut self,
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
        let buffer =
            self.prepare_text_buffer(text_key, text, attrs, metrics, align, width_f, height_f);

        self.text_buffers.push(PositionedBuffer {
            buffer,
            text_key,
            pos,
            color,
            scale,
            align,
        });
    }

    /// Shapes this output's lyric/track-info/weather text and uploads the
    /// resulting vertices - the per-output "text params changed" half of
    /// what `draw_frame`'s loop does before calling `encode_frame`. Every
    /// input here is resolution/DPI-independent frame state *except*
    /// `width_f`/`height_f`/`scale_factor`/`monitor`, which is exactly why
    /// the caller only re-runs this when those four (bundled as
    /// `last_text_params`) actually change.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        monitor: u32,
        width_f: f32,
        height_f: f32,
        scale_factor: f32,
        attrs: &Attrs,
        lyric_window: Option<LyricWindow>,
        lyrics_size: f32,
        lyrics_position: [f32; 2],
        lyrics_align: cosmic_text::Align,
        secondary_text: [f32; 4],
        text_color_diff: [f32; 4],
        track_text: Option<(&str, u64)>,
        track_info_size: f32,
        track_info_position: [f32; 2],
        track_info_align: cosmic_text::Align,
        weather_text: Option<(&str, u64)>,
        weather_size: f32,
        weather_position: [f32; 2],
        weather_align: cosmic_text::Align,
    ) {
        self.recycle_buffers();

        let logical_height = height_f / scale_factor;

        if let Some((lyric_window, physics)) = lyric_window {
            let base_font_size = (logical_height * 0.04).clamp(16.0, 48.0)
                * scale_factor
                * lyrics_size.clamp(0.25, 4.0);
            let active_font_size = base_font_size * 1.5;
            let inv_active_font_size = 1.0 / active_font_size;
            let line_spacing = active_font_size * 1.2;

            // Bound wrapping to the space actually visible between the
            // lyrics anchor point and this monitor's own edge. Themes like
            // "monstercat" anchor lyrics off-center (x=0.49, left aligned),
            // so shaping against the full monitor width let long lines run
            // past the right edge before cosmic-text ever wrapped them.
            // Each monitor renders as an independent surface here (no
            // cross-monitor spanning), so bounding to this monitor's own
            // visible span is always correct, whether or not another
            // display sits next to it.
            let lyrics_anchor_x = lyrics_position[0] * width_f;
            let lyrics_wrap_width = match lyrics_align {
                cosmic_text::Align::Left => width_f - lyrics_anchor_x,
                cosmic_text::Align::Right => lyrics_anchor_x,
                _ => 2.0 * lyrics_anchor_x.min(width_f - lyrics_anchor_x),
            }
            .max(active_font_size * 3.0);

            // Pre-calculate bounce scalars for the current monitor's scale factor
            let bounce_8_scaled = physics.lyric_bounce * 8.0 * scale_factor;
            let bounce_12_scaled = physics.lyric_bounce * 12.0 * scale_factor;

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
                buffer: Buffer,
            }
            let mut shaped_lines: Vec<ShapedLyric> = Vec::with_capacity(5);

            for (line_idx, text, text_hash) in lyric_window {
                // Compute exactly how far this string is from the "current active string"
                let dist = (line_idx as f32)
                    - (physics.current_lyric_idx as f32)
                    - physics.lyric_scroll_offset;
                let abs_dist = dist.abs();

                if abs_dist > 2.0 {
                    continue;
                }

                let center_weight = (1.0 - abs_dist).clamp(0.0, 1.0);

                let scale = base_font_size + (active_font_size - base_font_size) * center_weight;
                let final_scale = scale + bounce_8_scaled * center_weight;

                // The buffer was shaped/wrapped assuming font_size ==
                // active_font_size (see `metrics` above), so a render_scale
                // above 1.0 - which the beat-reactive bounce can produce,
                // since lyric_bounce_value is an unclamped spring - would
                // draw the active line larger than the box it was just
                // wrapped to fit, spilling back past the screen edge. Clamp
                // to the shaped size; the pulse still reads clearly on the
                // way up to it.
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
                        monitor,
                        line: line_idx as u32,
                        content_hash: text_hash,
                    };
                    let buffer = self.prepare_text_buffer(
                        text_key,
                        &text,
                        attrs,
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
            // The weights keep the layout continuous while lines scroll: a
            // wrapped line pushes everything below it down while it is at
            // or below the active slot (weight fades in over dist -1 -> 0),
            // and pulls itself plus everything above it up once it scrolls
            // above the active slot, so its overflow never collides with
            // the anchored active line.
            let anchor_x = lyrics_position[0] * width_f;
            let anchor_y = lyrics_position[1] * height_f;
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
                self.text_buffers.push(PositionedBuffer {
                    buffer: line.buffer,
                    text_key: line.text_key,
                    pos: [anchor_x, anchor_y + line.y_base + shift],
                    color: line.color,
                    scale: line.render_scale,
                    align: lyrics_align,
                });
            }
        }

        if let Some((text, track_hash)) = track_text {
            let info_scale = (logical_height * 0.025).clamp(16.0, 36.0)
                * scale_factor
                * track_info_size.clamp(0.25, 4.0);
            let metrics = Metrics::new(info_scale, info_scale * 1.2);
            let text_key = TextCacheKey::Track {
                monitor,
                content_hash: track_hash,
            };
            let pos = [
                track_info_position[0] * width_f,
                track_info_position[1] * height_f,
            ];
            self.push_text_buffer(
                text_key,
                text,
                attrs,
                metrics,
                track_info_align,
                width_f,
                height_f,
                pos,
                secondary_text,
                1.0,
            );
        }

        if let Some((text, weather_hash)) = weather_text {
            let weather_scale = (logical_height * 0.02).clamp(14.0, 24.0)
                * scale_factor
                * weather_size.clamp(0.25, 4.0);
            let metrics = Metrics::new(weather_scale, weather_scale * 1.2);
            let text_key = TextCacheKey::Weather {
                monitor,
                content_hash: weather_hash,
            };
            let pos = [
                weather_position[0] * width_f,
                weather_position[1] * height_f,
            ];
            self.push_text_buffer(
                text_key,
                text,
                attrs,
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
            &mut self.text_renderer,
            queue,
            &mut self.font_system,
            &mut self.swash_cache,
            self.text_buffers.as_mut(),
            width_f,
            height_f,
        );

        let vertices_bytes: &[u8] = bytemuck::cast_slice(&self.text_renderer.cpu_vertices);
        let indices_bytes: &[u8] = bytemuck::cast_slice(&self.text_renderer.cpu_indices);

        if self.text_renderer.vertex_capacity < self.text_renderer.cpu_vertices.len() {
            self.text_renderer.vertex_capacity =
                self.text_renderer.cpu_vertices.len().next_power_of_two();
            self.text_renderer.vertices = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Text Vertex Buffer"),
                size: (self.text_renderer.vertex_capacity
                    * std::mem::size_of::<super::super::text::TextVertex>())
                    as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if self.text_renderer.index_capacity < self.text_renderer.cpu_indices.len() {
            self.text_renderer.index_capacity =
                self.text_renderer.cpu_indices.len().next_power_of_two();
            self.text_renderer.indices = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Text Index Buffer"),
                size: (self.text_renderer.index_capacity * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        queue.write_buffer(&self.text_renderer.vertices, 0, vertices_bytes);
        queue.write_buffer(&self.text_renderer.indices, 0, indices_bytes);
        self.text_renderer.num_indices = self.text_renderer.cpu_indices.len() as u32;
    }
}

/// The lyric-scroll/bounce physics state `prepare()` needs, bundled since
/// it's always passed together with the lyric window text.
pub(crate) struct LyricPhysics {
    pub(crate) current_lyric_idx: usize,
    pub(crate) lyric_scroll_offset: f32,
    pub(crate) lyric_bounce: f32,
}

/// The visible lyric window: (line number, text, content hash) for each
/// line within ±2 of the current one, plus the physics driving where they
/// land on screen.
pub(crate) type LyricWindow = (Vec<(usize, Box<str>, u64)>, LyricPhysics);
