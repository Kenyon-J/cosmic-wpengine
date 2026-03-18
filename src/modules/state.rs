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
    event::{TrackInfo, WeatherData},
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

    /// The palette of the previously playing track, used for smooth visualizer colour transitions.
    pub previous_palette: Option<Vec<[f32; 3]>>,

    /// The extrapolated playback position of the current track.
    pub playback_position: std::time::Duration,

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
            previous_palette: None,
            playback_position: std::time::Duration::ZERO,
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

        if self.is_playing {
            self.playback_position += std::time::Duration::from_secs_f32(delta_seconds);
        }
    }

    /// Start a new transition (called when track changes, weather changes, etc.)
    pub fn begin_transition(&mut self) {
        self.transition_progress = 0.0;
    }

    /// Update the time-of-day value from the system clock.
    pub fn update_time(&mut self) {
        self.time_of_day = Self::current_time_of_day();
    }

    /// Calculates a visual pulse value (0.0 to 1.0) based on the active lyric.
    /// Creates a sharp hit when a lyric starts, followed by a smooth decay.
    pub fn lyric_pulse(&self) -> f32 {
        let Some(track) = &self.current_track else { return 0.0 };
        let Some(lyrics) = &track.lyrics else { return 0.0 };

        let current_time = self.playback_position.as_secs_f32();

        // LRCLIB files are chronological, use binary search to find the active line quickly (O(log N))
        let idx = lyrics.partition_point(|l| l.start_time_secs <= current_time);

        if idx > 0 {
            let line = &lyrics[idx - 1];
            let elapsed = current_time - line.start_time_secs;
            // 1-second cubic decay for a punchy visual hit
            if elapsed >= 0.0 && elapsed < 1.0 {
                let t = 1.0 - elapsed;
                return t * t * t;
            }
        }

        0.0
    }

    /// Returns the text of the currently active lyric, if any.
    pub fn active_lyric(&self) -> Option<&str> {
        let track = self.current_track.as_ref()?;
        let lyrics = track.lyrics.as_ref()?;
        let current_time = self.playback_position.as_secs_f32();
        
        let idx = lyrics.partition_point(|l| l.start_time_secs <= current_time);
        if idx > 0 {
            return Some(&lyrics[idx - 1].text);
        }
        None
    }

    /// Returns a convenience description of the current scene for the renderer.
    pub fn scene_description(&self) -> SceneHint {
        // If music is playing and we have album art, prioritise that
        if self.is_playing
            && self
                .current_track
                .as_ref()
                .and_then(|t| t.album_art.as_ref())
                .is_some()
        {
            return SceneHint::AlbumArt;
        }

        // If audio is active (even without art), show visualiser
        let audio_energy: f32 =
            self.audio_bands.iter().sum::<f32>() / self.audio_bands.len() as f32;
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
