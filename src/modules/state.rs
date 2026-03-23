use super::{
    config::Config,
    event::{TrackInfo, WeatherData},
};

pub struct AppState {
    pub config: Config,

    pub current_track: Option<TrackInfo>,
    pub is_playing: bool,
    pub previous_palette: Option<Vec<[f32; 3]>>,
    pub playback_position: std::time::Duration,

    pub audio_bands: Vec<f32>,
    pub audio_waveform: Vec<f32>,

    pub weather: Option<WeatherData>,

    pub time_of_day: f32,

    pub transition_progress: f32,
    pub transparent_fade: f32,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let band_count = config.audio.bands;
        let initial_fade = if config.appearance.transparent_background { 1.0 } else { 0.0 };
        Self {
            config,
            current_track: None,
            is_playing: false,
            previous_palette: None,
            playback_position: std::time::Duration::ZERO,
            audio_bands: vec![0.0; band_count],
            audio_waveform: vec![0.0; band_count],
            weather: None,
            time_of_day: Self::current_time_of_day(),
            transition_progress: 1.0,
            transparent_fade: initial_fade,
        }
    }

    pub fn tick_transition(&mut self, delta_seconds: f32) {
        let speed = 1.5;
        self.transition_progress = (self.transition_progress + delta_seconds * speed).min(1.0);

        let target_fade = if self.config.appearance.transparent_background { 1.0 } else { 0.0 };
        if self.transparent_fade < target_fade {
            self.transparent_fade = (self.transparent_fade + delta_seconds * 3.0).min(1.0);
        } else if self.transparent_fade > target_fade {
            self.transparent_fade = (self.transparent_fade - delta_seconds * 3.0).max(0.0);
        }

        if self.is_playing {
            self.playback_position += std::time::Duration::from_secs_f32(delta_seconds);
        }
    }

    pub fn begin_transition(&mut self) {
        self.transition_progress = 0.0;
    }

    pub fn update_time(&mut self) {
        self.time_of_day = Self::current_time_of_day();
    }

    pub fn active_lyrics(&self) -> (Option<&str>, Option<&str>, Option<&str>) {
        let Some(track) = self.current_track.as_ref() else { return (None, None, None); };
        let Some(lyrics) = track.lyrics.as_ref() else { return (None, None, None); };
        let current_time = self.playback_position.as_secs_f32();
        
        let idx = lyrics.partition_point(|l| l.start_time_secs <= current_time);
        
        let prev = if idx > 1 { Some(lyrics[idx - 2].text.as_str()) } else { None };
        let current = if idx > 0 { Some(lyrics[idx - 1].text.as_str()) } else { None };
        let next = if idx < lyrics.len() { Some(lyrics[idx].text.as_str()) } else { None };
        
        (prev, current, next)
    }

    pub fn scene_description(&self) -> SceneHint {
        if self
                .current_track
                .as_ref()
                .and_then(|t| t.album_art.as_ref())
                .is_some()
        {
            return SceneHint::AlbumArt;
        }

        let audio_energy: f32 =
            self.audio_bands.iter().sum::<f32>() / self.audio_bands.len() as f32;
        if audio_energy > 0.05 {
            return SceneHint::AudioVisualiser;
        }

        SceneHint::Ambient
    }

    fn current_time_of_day() -> f32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        (secs % 86400) as f32 / 86400.0
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SceneHint {
    AlbumArt,
    AudioVisualiser,
    Ambient,
}
