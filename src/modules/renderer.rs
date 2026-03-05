// =============================================================================
// modules/renderer.rs
// =============================================================================
// The renderer is the heart of the wallpaper engine. It:
//   1. Owns the wgpu device, queue, and swap chain
//   2. Receives events and updates AppState accordingly
//   3. Runs the frame loop at the configured FPS
//   4. Decides which scene to draw based on AppState
//   5. Dispatches to the appropriate drawing routine
//
// For beginners: wgpu is Rust's GPU abstraction. It lets us write shaders
// (small programs that run on the GPU) to draw complex visuals efficiently.
// =============================================================================

use anyhow::Result;
use tokio::sync::mpsc::Receiver;
use tracing::info;
use std::time::{Instant, Duration};

use super::{
    event::Event,
    state::{AppState, SceneHint},
    wayland::WaylandSurface,
    colour::{time_to_sky_colour, lerp_colour},
};

pub struct Renderer {
    // wgpu core objects
    // In the full implementation these would be:
    //   device: wgpu::Device,
    //   queue: wgpu::Queue,
    //   surface: wgpu::Surface,
    //   config: wgpu::SurfaceConfiguration,

    state: AppState,
    frame_duration: Duration,
}

impl Renderer {
    pub async fn new(_surface: &WaylandSurface, state: AppState) -> Result<Self> {
        let fps = state.config.fps;

        info!("Initialising wgpu renderer...");

        // Full wgpu initialisation would be:
        //
        // let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        //     backends: wgpu::Backends::VULKAN, // Wayland uses Vulkan or EGL
        //     ..Default::default()
        // });
        //
        // let wgpu_surface = unsafe {
        //     instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(...))
        // }?;
        //
        // let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
        //     power_preference: wgpu::PowerPreference::LowPower, // save battery
        //     compatible_surface: Some(&wgpu_surface),
        //     ..Default::default()
        // }).await.expect("No suitable GPU adapter found");
        //
        // let (device, queue) = adapter.request_device(
        //     &wgpu::DeviceDescriptor::default(), None
        // ).await?;

        info!("Renderer initialised at {}fps", fps);

        Ok(Self {
            state,
            frame_duration: Duration::from_secs_f64(1.0 / fps as f64),
        })
    }

    /// The main render loop. Runs until the application exits.
    pub async fn run(
        &mut self,
        mut event_rx: Receiver<Event>,
        surface: WaylandSurface,
    ) -> Result<()> {
        let mut last_frame = Instant::now();

        loop {
            // --- Process all pending events (non-blocking) ---
            // We drain all available events before drawing to avoid
            // rendering stale state when many events arrive at once.
            loop {
                match event_rx.try_recv() {
                    Ok(event) => self.handle_event(event),
                    Err(_) => break, // no more events queued
                }
            }

            // --- Update time-based state ---
            self.state.update_time();

            let now = Instant::now();
            let delta = now.duration_since(last_frame).as_secs_f32();
            self.state.tick_transition(delta);
            last_frame = now;

            // --- Draw the current frame ---
            self.draw_frame(&surface)?;

            // --- Sleep to hit target FPS ---
            let elapsed = last_frame.elapsed();
            if elapsed < self.frame_duration {
                tokio::time::sleep(self.frame_duration - elapsed).await;
            }
        }
    }

    /// Handle an incoming event by updating AppState.
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::TrackChanged(track) => {
                info!("Now playing: {} - {}", track.artist, track.title);
                self.state.current_track = Some(track);
                self.state.is_playing = true;
                self.state.begin_transition(); // smoothly blend to new art
            }

            Event::PlaybackStopped => {
                self.state.is_playing = false;
                self.state.begin_transition();
            }

            Event::AudioFrame(bands) => {
                // Smooth the incoming bands with the previous frame.
                // This prevents jarring jumps between frames.
                let smoothing = self.state.config.audio.smoothing;
                let target_len = self.state.audio_bands.len();

                // Downsample or upsample bands to match our configured band count
                let resampled = Self::resample_bands(&bands, target_len);

                for (current, target) in self.state.audio_bands.iter_mut()
                    .zip(resampled.iter())
                {
                    *current = *current * smoothing + *target * (1.0 - smoothing);
                }
            }

            Event::WeatherUpdated(weather) => {
                info!("Weather: {:?} {:.1}°C", weather.condition, weather.temperature_celsius);
                self.state.weather = Some(weather);
                self.state.begin_transition();
            }
        }
    }

    /// Draw a single frame based on current AppState.
    fn draw_frame(&self, _surface: &WaylandSurface) -> Result<()> {
        // In the real implementation:
        //   let output = self.wgpu_surface.get_current_texture()?;
        //   let view = output.texture.create_view(&Default::default());
        //   let mut encoder = self.device.create_command_encoder(...);
        //
        // Then dispatch to the appropriate scene renderer:

        match self.state.scene_description() {
            SceneHint::AlbumArt => {
                // Upload album art as a texture, render with a blur+colour grade shader
                self.render_album_art_scene();
            }
            SceneHint::AudioVisualiser => {
                // Render frequency bars using audio_bands data
                self.render_visualiser_scene();
            }
            SceneHint::Ambient => {
                // Time-of-day sky gradient, optionally with weather effects
                self.render_ambient_scene();
            }
        }

        //   self.queue.submit([encoder.finish()]);
        //   output.present();

        Ok(())
    }

    /// Render the album art wallpaper scene.
    /// The art is blurred, darkened slightly, and colour-graded using
    /// the extracted palette colours.
    fn render_album_art_scene(&self) {
        if let Some(track) = &self.state.current_track {
            // Shader would:
            //   1. Sample the album art texture
            //   2. Apply gaussian blur (several passes)
            //   3. Overlay a subtle colour wash from the palette
            //   4. Blend with the previous scene based on transition_progress
            let _palette = track.palette.as_deref().unwrap_or(&[]);
            // dispatch to album_art render pipeline...
        }
    }

    /// Render the audio visualiser scene.
    /// Shows frequency bands as animated bars or a waveform.
    fn render_visualiser_scene(&self) {
        // Shader would:
        //   1. Take audio_bands as a uniform buffer
        //   2. Render bars at each band position
        //   3. Colour bars using the current track's palette if available
        //   4. Add a glow effect using additive blending
        let _bands = &self.state.audio_bands;
        // dispatch to visualiser render pipeline...
    }

    /// Render the ambient time/weather scene.
    fn render_ambient_scene(&self) {
        let sky = time_to_sky_colour(self.state.time_of_day);

        // If we have weather, modify the sky colour
        if let Some(weather) = &self.state.weather {
            use super::event::WeatherCondition;
            let _modified_sky = match weather.condition {
                WeatherCondition::Rain | WeatherCondition::Thunderstorm => {
                    // Darken and desaturate for rainy weather
                    lerp_colour(sky, [0.2, 0.2, 0.25], 0.6)
                }
                WeatherCondition::Snow => {
                    lerp_colour(sky, [0.8, 0.85, 0.9], 0.4)
                }
                _ => sky,
            };
            // Add weather particle effects (rain, snow) via a particle shader...
        }

        // dispatch to ambient render pipeline with sky colour...
    }

    /// Resample a band array to a different length.
    /// Used to normalise FFT output to our configured band count.
    fn resample_bands(input: &[f32], target_len: usize) -> Vec<f32> {
        if input.len() == target_len {
            return input.to_vec();
        }
        (0..target_len).map(|i| {
            let src = i as f32 * input.len() as f32 / target_len as f32;
            let lo = src.floor() as usize;
            let hi = (lo + 1).min(input.len() - 1);
            let t = src.fract();
            input[lo] * (1.0 - t) + input[hi] * t
        }).collect()
    }
}
