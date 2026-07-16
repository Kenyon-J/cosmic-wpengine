mod events;
mod init;
mod updates;
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

/// While the scene is static, redraws pause and this heartbeat keeps the
/// screen fresh instead: slow drifts (the time-of-day sky colour) still
/// appear, and any missed invalidation self-heals within a second rather
/// than leaving a permanently stale frame.
const STATIC_SCENE_HEARTBEAT: Duration = Duration::from_secs(1);

/// Single home for surface format/alpha/present-mode selection and the
/// initial configure, shared by first-time init (`init.rs`) and the
/// monitor-hotplug rebuild path above. These two blocks were previously
/// duplicated and had already drifted (only one clamped width/height to 1,
/// which wgpu requires - a disabled/zero-sized output would panic the other).
pub(crate) fn configure_surface(
    surface: wgpu::Surface<'static>,
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> GpuOutput {
    let caps: wgpu::SurfaceCapabilities = surface.get_capabilities(adapter);
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
        width: width.max(1),
        height: height.max(1),
        present_mode,
        alpha_mode,
        view_formats: vec![],
        desired_maximum_frame_latency: 1, // Enforce double buffering to save ~33MB+ VRAM per monitor
        color_space: wgpu::SurfaceColorSpace::Auto,
    };
    surface.configure(device, &config);

    GpuOutput { surface, config }
}

pub struct Renderer {
    pub(crate) instance: wgpu::Instance,
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) outputs: Vec<GpuOutput>,
    /// Render-target format shared by the shader pipelines. Tracked separately
    /// from `outputs` because config reloads can arrive while every monitor is
    /// disconnected (`outputs` empty), and the visualiser reload still needs a
    /// format to rebuild against.
    pub(crate) surface_format: wgpu::TextureFormat,
    pub(crate) font_system: FontSystem,
    pub(crate) swash_cache: SwashCache,
    pub(crate) text_renderer: TextRenderer,
    pub(crate) text_buffer_cache:
        std::collections::HashMap<TextCacheKey, Buffer, rustc_hash::FxBuildHasher>,
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
    pub(crate) current_album_size: Option<(u32, u32)>,
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
    pub(crate) audio_processing_bins: Vec<(usize, usize, f32)>,
    pub(crate) inv_smoothing: f32,
    pub(crate) inv_target_len: f32,
    pub(crate) waveform_bin_ranges: Vec<(usize, usize)>,
    pub(crate) lyric_bounce_value: f32,
    pub(crate) lyric_bounce_velocity: f32,
    /// While a new track's art is still being fetched in the background, the
    /// previous track's art/palette stay on screen. If nothing has arrived by
    /// this deadline, they fade out rather than lingering stale forever.
    pub(crate) pending_art_deadline: Option<Instant>,
    /// Opacity multiplier for the album art (bg + fg). Normally 1.0; eases to
    /// 0.0 once `pending_art_deadline` expires, after which the art is dropped.
    pub(crate) art_fade: f32,
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
    pub(crate) text_color_diff: [f32; 4],
    pub(crate) active_particles: u32,
    pub(crate) weather_gravity: f32,
    pub(crate) weather_wind_x: f32,
    pub(crate) weather_type: u32,
    pub(crate) is_weather_active: bool,
    pub(crate) audio_max_energy: f32,
    pub(crate) audio_base_energy: f32,
    pub(crate) is_waveform_style: bool,
    pub(crate) bass_bin_range: (usize, usize),
    pub(crate) treble_bin_range: (usize, usize),
    pub(crate) vis_target_colors: ([f32; 3], [f32; 3]),
    pub(crate) vis_prev_colors: ([f32; 3], [f32; 3]),
    pub(crate) art_target_color: [f32; 3],
    pub(crate) art_prev_color: [f32; 3],
    pub(crate) album_art_aspect: f32,
    pub(crate) custom_bg_aspect: f32,
    pub(crate) last_occluded: Option<bool>,
}

impl Renderer {
    pub async fn run(
        &mut self,
        mut event_rx: Receiver<Event>,
        mut wayland_manager: WaylandManager,
        is_visible_tx: tokio::sync::watch::Sender<bool>,
    ) -> Result<()> {
        let mut last_frame = Instant::now();
        let mut last_config_serial = wayland_manager.app_data.configuration_serial;

        // Static-scene redraw pausing: when nothing on screen is in motion,
        // encoding and presenting an identical frame at target fps only
        // costs battery. Track when we last presented (for the heartbeat),
        // whether anything has been presented at all (never gate the very
        // first frame), and the previous animation state (to draw one final
        // "settle" frame when motion stops, so decayed-out elements don't
        // linger until the heartbeat).
        let mut last_present = Instant::now();
        let mut has_presented = false;
        let mut was_animating = true;

        let mut interval = tokio::time::interval(self.frame_duration);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        wayland_manager.update_opaque_regions(self.state.config.appearance.transparent_background);

        loop {
            // --- Dynamic FPS ---
            // .max(1) mirrors Config::sanitise: fps = 0 would panic below in
            // Duration::from_secs_f64(inf).
            let target_fps = self.state.config.fps.max(1);

            if self.current_fps != target_fps {
                info!("Updating FPS from {} to {}", self.current_fps, target_fps);
                self.current_fps = target_fps;
                self.frame_duration = Duration::from_secs_f64(1.0 / target_fps as f64);
                interval = tokio::time::interval(self.frame_duration);
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            }

            interval.tick().await;

            // Optimization: Consolidate time measurements by calling Instant::now() once per frame
            // and propagating this timestamp to all temporal logic (occlusion, physics, drawing).
            // This reduces syscall overhead in the high-frequency rendering hot path.
            let now = Instant::now();

            // One-shot reasons this frame's content differs from the last
            // presented one; continuous motion is covered by
            // scene_is_animating() at the draw gate below.
            let mut scene_dirty = false;

            let occluded = wayland_manager.is_occluded(now);
            if self.last_occluded != Some(occluded) {
                let _ = is_visible_tx.send(!occluded);
                self.last_occluded = Some(occluded);
                // Coming back into view needs a fresh frame no matter how
                // long the scene has been static.
                scene_dirty = true;
            }

            wayland_manager.dispatch_events()?;

            if wayland_manager.app_data.configuration_serial != last_config_serial {
                last_config_serial = wayland_manager.app_data.configuration_serial;

                self.current_outputs_cache.clear();
                self.current_outputs_cache.extend(wayland_manager.outputs());
                let current_outputs = &self.current_outputs_cache;

                info!(
                    "Monitor configuration changed ({} outputs), rebuilding GPU surfaces...",
                    current_outputs.len()
                );

                self.outputs.clear();
                wayland_manager.cleanup_dead_windows();

                for info in current_outputs {
                    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: Some(info.raw_display_handle()),
                        raw_window_handle: info.raw_window_handle(),
                    };
                    let surface = unsafe { self.instance.create_surface_unsafe(target) }
                        .map_err(|e| anyhow::anyhow!("Failed to recreate surface: {}", e))?;

                    self.outputs.push(configure_surface(
                        surface,
                        &self.adapter,
                        &self.device,
                        info.width,
                        info.height,
                    ));
                }

                if let Some(out) = self.outputs.first() {
                    self.surface_format = out.config.format;
                }

                scene_dirty = true;
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
                        scene_dirty = true;
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
                // Every event can change what's on screen (new track, new
                // video frame, weather, config...) EXCEPT audio buffers:
                // those arrive continuously - silence included - and their
                // visual effect is already captured by the energy terms in
                // scene_is_animating(). Blanket-marking them dirty would
                // keep redrawing a silent, motionless scene forever.
                scene_dirty |= !matches!(event, Event::AudioFrame { .. });
                self.handle_event(event, now).await;
            }

            if transparent_changed {
                wayland_manager
                    .update_opaque_regions(self.state.config.appearance.transparent_background);
            }

            // Cap the delta to 100ms to prevent the Explicit Euler physics from exploding after a monitor sleep!
            let delta = now
                .saturating_duration_since(last_frame)
                .as_secs_f32()
                .min(0.1);
            self.state.update_time(delta);
            self.state.tick_transition(delta);
            last_frame = now;

            // Pre-calculate shared exponential decay factors once per frame to reduce redundant math.
            let decay_12 = (-12.0 * delta).exp();
            let decay_15 = (-15.0 * delta).exp();

            // Exponential decay for the beat pulse so it snaps up and softly falls down
            self.beat_pulse *= decay_12;
            // Treble decays slightly faster for snappier, rapid hi-hats
            self.treble_pulse *= decay_15;

            // Prevent subnormal float degradation which causes massive CPU slowdowns
            if self.beat_pulse.abs() < 1e-5 {
                self.beat_pulse = 0.0;
            }
            if self.treble_pulse.abs() < 1e-5 {
                self.treble_pulse = 0.0;
            }

            // Spring physics for organic lyric bounce (Hooke's Law)
            let stiffness = self.theme.effects.lyric_spring_stiffness;
            let damping = self.theme.effects.lyric_spring_damping;
            let spring_force =
                -stiffness * self.lyric_bounce_value - damping * self.lyric_bounce_velocity;
            self.lyric_bounce_velocity += spring_force * delta;
            self.lyric_bounce_value += self.lyric_bounce_velocity * delta;

            // Prevent subnormal float degradation for spring physics
            if self.lyric_bounce_velocity.abs() < 1e-5 {
                self.lyric_bounce_velocity = 0.0;
            }
            if self.lyric_bounce_value.abs() < 1e-5 {
                self.lyric_bounce_value = 0.0;
            }

            // The grace period for keeping the previous track's art on screen
            // expired without replacement art arriving: ease its opacity out
            // over ~1.5s, then drop it, rather than showing stale art
            // indefinitely or popping it off screen in a single frame.
            if self
                .pending_art_deadline
                .is_some_and(|deadline| now >= deadline)
            {
                if self.art_fade >= 1.0 {
                    tracing::info!("No album art arrived within the grace period; fading out");
                }
                self.art_fade = (self.art_fade - delta / 1.5).max(0.0);
                if self.art_fade == 0.0 {
                    self.pending_art_deadline = None;
                    self.art_fade = 1.0;
                    self.album_art_bg_bind_group = None;
                    self.album_art_fg_bind_group = None;
                    self.current_album_texture = None;
                    self.current_album_size = None;
                    self.state.has_album_art = false;
                    if let Some(track) = self.state.current_track.as_mut() {
                        self.state.previous_palette = track.palette.take();
                    }
                    self.update_theme_colors();
                    self.update_text_colors();
                    self.state.begin_transition();
                }
            }

            let playback_pos = self.state.playback_position.as_secs_f32();

            // Optimization: Only perform the O(log N) partition_point search if the playback position
            // has actually moved past the current lyric line or jumped significantly (e.g. seeking).
            // This reduces the search to O(1) for the vast majority of frames.
            let lyrics = self
                .state
                .current_track
                .as_ref()
                .and_then(|t| t.lyrics.as_ref());
            let current_idx = if let Some(l) = lyrics {
                let current_idx_base = self.current_lyric_idx;

                let is_in_bounds = if current_idx_base == 0 {
                    // We are currently before the first lyric line
                    l.first()
                        .is_none_or(|first| playback_pos < first.start_time_secs)
                } else {
                    // We are at or after the first lyric line
                    if let Some(curr_line) = l.get(current_idx_base - 1) {
                        if playback_pos < curr_line.start_time_secs {
                            false // Seek backwards
                        } else {
                            // playback_pos >= curr_line.start_time_secs, check if it's before the next line
                            l.get(current_idx_base)
                                .is_none_or(|next| playback_pos < next.start_time_secs)
                        }
                    } else {
                        false // Array bounds changed or index invalid
                    }
                };

                if is_in_bounds {
                    current_idx_base
                } else {
                    l.partition_point(|line| line.start_time_secs <= playback_pos)
                }
            } else {
                0
            };

            if current_idx != self.current_lyric_idx {
                if (current_idx as isize - self.current_lyric_idx as isize).abs() > 2 {
                    // Prevent massive scroll jumps on track init or seeking
                    self.current_lyric_idx = current_idx;
                    self.lyric_scroll_offset = 0.0;
                } else {
                    self.lyric_scroll_offset += self.current_lyric_idx as f32 - current_idx as f32;
                    self.current_lyric_idx = current_idx;
                }
                scene_dirty = true;
            }

            // Smoothly interpolate the scroll offset back to 0
            self.lyric_scroll_offset *= decay_12;
            if self.lyric_scroll_offset.abs() < 1e-5 {
                self.lyric_scroll_offset = 0.0;
            }

            let scene_animating = self.scene_is_animating();
            // The frame where motion just stopped still needs to be drawn:
            // the previous present happened while elements (bars, lyric
            // scroll) were mid-decay, so without this "settle" frame their
            // residue would linger on screen until the heartbeat.
            let settle_frame = was_animating && !scene_animating;
            if settle_frame {
                tracing::debug!("Scene settled; pausing redraws (1s heartbeat)");
            } else if scene_animating && !was_animating {
                tracing::debug!("Scene active again; resuming per-frame redraws");
            }
            was_animating = scene_animating;

            let needs_draw = scene_dirty
                || scene_animating
                || settle_frame
                || !has_presented
                || now.saturating_duration_since(last_present) >= STATIC_SCENE_HEARTBEAT;

            if needs_draw && wayland_manager.any_monitor_ready() {
                super::draw::draw_frame(self, &mut wayland_manager, delta, now)?;
                last_present = now;
                has_presented = true;
            }

            // Tell wgpu to process internal garbage collection.
            // If we don't call this when output.present() is skipped (e.g. monitor asleep or occluded),
            // dropped textures and command buffers will queue up indefinitely and cause an OOM crash!
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
    }

    /// True while any on-screen element is in motion and therefore needs
    /// per-frame redraws. When this is false (and nothing marked the scene
    /// dirty), the render loop skips encode/present entirely - the dominant
    /// idle cost of a wallpaper that would otherwise repaint an identical
    /// frame at target fps.
    ///
    /// Every term must reach an exact resting value, not an asymptote:
    /// the pulses/spring/scroll fields are flushed to 0.0 when they drop
    /// below 1e-5 (also protecting against subnormals), the transition and
    /// fade values clamp to their endpoints, and the audio energies decay
    /// geometrically through the 0.001 threshold that also gates whether
    /// the visualiser is drawn at all.
    fn scene_is_animating(&self) -> bool {
        // Visualiser bars/waveform moving, or beat/treble flashes decaying.
        let audio_active = self.audio_max_energy > 0.001
            || self.state.audio_energy > 0.001
            || self.beat_pulse != 0.0
            || self.treble_pulse != 0.0;

        // Lyric scroll interpolation or spring bounce still settling.
        let lyrics_moving = self.lyric_scroll_offset != 0.0
            || self.lyric_bounce_value != 0.0
            || self.lyric_bounce_velocity != 0.0;

        // Track/art crossfade, transparency fade, or the art grace-period
        // fade-out (pending_art_deadline drives art_fade in the loop above).
        let target_fade = if self.state.config.appearance.transparent_background {
            1.0
        } else {
            0.0
        };
        let fading = self.state.transition_progress < 1.0
            || self.state.transparent_fade != target_fade
            || self.pending_art_deadline.is_some();

        // Weather particle physics runs a compute pass every frame.
        let weather_active = self.is_weather_active && self.active_particles > 0;

        // The procedural sky shader animates with time (cloud drift, rain
        // streaks). It can be on screen whenever no custom background
        // texture exists; conservatively treat that as always-animating
        // rather than replicating draw_frame's exact layering rules here.
        let ambient_possible = self.custom_bg_bind_group.is_none();

        audio_active || lyrics_moving || fading || weather_active || ambient_possible
    }
}
