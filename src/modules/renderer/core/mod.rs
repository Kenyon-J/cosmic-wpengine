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
}
