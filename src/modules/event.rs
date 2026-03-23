#[derive(Debug)]
pub enum Event {
    ConfigUpdated(super::config::Config),
    TrackChanged(TrackInfo),
    PlaybackStopped,
    PlaybackResumed,
    PlayerShutDown,
    PlaybackPosition(std::time::Duration),
    AudioFrame {
        bands: Vec<f32>,
        waveform: Vec<f32>,
    },
    VideoFrame(image::RgbaImage),
    WeatherUpdated(WeatherData),
}

#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub title: String,
    pub artist: String,
    #[allow(dead_code)]
    pub album: String,
    pub album_art: Option<image::RgbaImage>,
    pub palette: Option<Vec<[f32; 3]>>,
    pub lyrics: Option<Vec<LyricLine>>,
    pub video_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LyricLine {
    pub start_time_secs: f32,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct WeatherData {
    pub condition: WeatherCondition,
    pub temperature_celsius: f32,
    #[allow(dead_code)]
    pub location: String,
}

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
