// =============================================================================
// modules/state.rs
// =============================================================================
// AppState is the central store of everything the renderer needs to draw
// the current frame. It gets updated by events from subsystems.
//
// For beginners: think of this as the "current snapshot" of everything
// happening — what's playing, what the audio looks like, what the weather is.
// =============================================================================

use super::{
    config::Config,
    event::{TrackInfo, WeatherData, WeatherCondition},
};

/// The complete visual state of the wallpaper at any given moment.
/// The renderer reads from this each frame to decide what to draw.
pub struct AppState {
    pub config: Config,

    // --- Music / MPRIS state ---

    /// Info about the currently playing track. None if nothing is playing.
    pub current_track: Option<TrackInfo>,

    /// Whether a track is actively playing (affects animation speed/intensity)
    pub is_playing: bool,

    // --- Audio visualiser state ---

    /// The current frequency spectrum, one f32 per band.
    /// Values are 0.0 (silent) to 1.0 (loud).
    /// Smoothed over time to avoid jarring jumps.
    pub audio_bands: Vec<f32>,

    // --- Weather state ---

    /// The latest weather data. None until first poll completes.
    pub weather: Option<WeatherData>,

    // --- Time state ---

    /// The current time of day as a fraction: 0.0 = midnight, 0.5 = noon.
    /// Updated each frame. Used for day/night cycle effects.
    pub time_of_day: f32,

    // --- Transition state ---

    /// When we transition between scenes (e.g. new album art), we blend
    /// from the old state to the new. This tracks how far through we are,
    /// 0.0 = fully old scene, 1.0 = fully new scene.
    pub transition_progress: f32,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let band_count = config.audio.bands;
        Self {
            config,
            current_track: None,
            is_playing: false,
            audio_bands: vec![0.0; band_count],
            weather: None,
            time_of_day: Self::current_time_of_day(),
            transition_progress: 1.0,
        }
    }

    /// Advance the transition animation. Called each frame.
    /// speed controls how fast we blend — 1.0 completes in ~1 second at 60fps.
    pub fn tick_transition(&mut self, delta_seconds: f32) {
        let speed = 1.5;
        self.transition_progress = (self.transition_progress + delta_seconds * speed).min(1.0);
    }

    /// Start a new transition (called when track changes, weather changes, etc.)
    pub fn begin_transition(&mut self) {
        self.transition_progress = 0.0;
    }

    /// Update the time-of-day value from the system clock.
    pub fn update_time(&mut self) {
        self.time_of_day = Self::current_time_of_day();
    }

    /// Returns a convenience description of the current scene for the renderer.
    pub fn scene_description(&self) -> SceneHint {
        // If music is playing and we have album art, prioritise that
        if self.is_playing && self.current_track.as_ref()
            .and_then(|t| t.album_art.as_ref()).is_some()
        {
            return SceneHint::AlbumArt;
        }

        // If audio is active (even without art), show visualiser
        let audio_energy: f32 = self.audio_bands.iter().sum::<f32>()
            / self.audio_bands.len() as f32;
        if audio_energy > 0.05 {
            return SceneHint::AudioVisualiser;
        }

        // Fall back to weather or time-of-day
        SceneHint::Ambient
    }

    fn current_time_of_day() -> f32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Seconds since midnight as a fraction of the day
        (secs % 86400) as f32 / 86400.0
    }
}

/// A hint to the renderer about what kind of scene to draw.
/// The renderer uses this to blend between different shader modes.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SceneHint {
    AlbumArt,
    AudioVisualiser,
    Ambient,
}
