#[derive(Debug)]
pub enum Event {
    ConfigUpdated(Box<super::config::Config>),
    TrackChanged(Box<TrackInfo>),
    PlaybackStopped,
    PlaybackResumed,
    PlayerShutDown,
    PlaybackPosition(std::time::Duration),
    AudioFrame {
        bands: Box<[f32]>,
        waveform: Box<[f32]>,
    },
    VideoFrame(super::video::PooledImage),
    WeatherUpdated(Box<WeatherData>),
}

#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub title: Box<str>,
    pub artist: Box<str>,
    pub album: Box<str>,
    pub album_art: Option<image::RgbaImage>,
    pub palette: Option<Box<[[f32; 3]]>>,
    pub lyrics: Option<Box<[LyricLine]>>,
    pub video_url: Option<Box<str>>,
}

#[derive(Debug, Clone)]
pub struct LyricLine {
    pub start_time_secs: f32,
    pub text: Box<str>,
}

#[derive(Debug, Clone)]
pub struct WeatherData {
    pub condition: WeatherCondition,
    pub temperature_celsius: f32,
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
