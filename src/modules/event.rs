// =============================================================================
// modules/event.rs
// =============================================================================
// Defines all the events that subsystems can send to the renderer.
//
// Using a single Event enum means the renderer has one channel to listen on,
// and one match statement to handle everything. Clean and simple.
// =============================================================================

use image::DynamicImage;

/// Every subsystem communicates with the renderer via these event variants.
/// The renderer receives these on its event channel and updates visual state.
#[derive(Debug)]
pub enum Event {
    // --- MPRIS Events ---
    /// A new track started playing. Contains metadata about the track.
    TrackChanged(TrackInfo),

    /// The player paused, stopped, or there's nothing playing.
    PlaybackStopped,

    /// The current playback position in the track.
    PlaybackPosition(std::time::Duration),

    // --- Audio Events ---
    /// A new frame of FFT frequency data is ready for the visualiser.
    /// The Vec<f32> contains amplitude values from low to high frequency,
    /// normalised to the range 0.0–1.0.
    AudioFrame(Vec<f32>),

    // --- Weather Events ---
    /// Fresh weather data has been fetched from the API.
    WeatherUpdated(WeatherData),
}

/// Metadata about the currently playing track, sourced from MPRIS.
#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub title: String,
    pub artist: String,
    pub album: String,

    /// Album art decoded into an image, ready for GPU upload.
    /// It's Option<> because not all tracks have art.
    pub album_art: Option<DynamicImage>,

    /// Dominant colours extracted from the album art.
    /// Used to tint the background even when not showing the full image.
    pub palette: Option<Vec<[f32; 3]>>,

    /// Synced lyrics fetched from LRCLIB.
    pub lyrics: Option<Vec<LyricLine>>,
}

/// A single line of synced lyrics.
#[derive(Debug, Clone)]
pub struct LyricLine {
    pub start_time_secs: f32,
    pub text: String,
}

/// Current weather conditions, fetched from the weather API.
#[derive(Debug, Clone)]
pub struct WeatherData {
    pub condition: WeatherCondition,
    pub temperature_celsius: f32,
    pub location: String,
}

/// Simplified weather condition that maps to a visual scene.
/// We don't need the full detail from the API — just enough to
/// decide what the wallpaper should look like.
#[derive(Debug, Clone, PartialEq)]
pub enum WeatherCondition {
    Clear,
    PartlyCloudy,
    Cloudy,
    Rain,
    Snow,
    Thunderstorm,
    Fog,
}
