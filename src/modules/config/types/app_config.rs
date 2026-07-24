use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum TemperatureUnit {
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct WeatherConfig {
    pub enabled: bool,
    pub latitude: f64,
    pub longitude: f64,
    pub poll_interval_minutes: u64,
    pub temperature_unit: TemperatureUnit,
    #[serde(default)]
    pub hide_effects: bool,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            latitude: 51.5,
            longitude: -0.1,
            poll_interval_minutes: 15,
            temperature_unit: TemperatureUnit::Celsius,
            hide_effects: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AudioConfig {
    pub style: String,
    pub bands: usize,
    pub smoothing: f32,
    pub show_lyrics: bool,
    /// Base URL of a Spotify canvas proxy API (e.g. a local
    /// `spotify-canvas-api` instance). Canvas video backgrounds are fetched
    /// only when this is set: any local process can bind a well-known
    /// localhost port, so a hardcoded default would let an unprivileged
    /// process feed the engine attacker-controlled video URLs. Opt-in only.
    #[serde(default)]
    pub canvas_proxy_url: Option<String>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            style: "monstercat".to_string(),
            bands: 64,
            smoothing: 0.7,
            show_lyrics: true,
            canvas_proxy_url: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub disable_blur: bool,
    pub blur_opacity: f32,
    pub transparent_background: bool,
    pub show_album_art: bool,
    pub album_art_background: bool,
    #[serde(default)]
    pub album_color_background: bool,
    #[serde(default)]
    pub font_family: Option<String>,
    pub custom_background_path: Option<String>,
    pub video_background_path: Option<String>,
    /// When the playing track has a Spotify Canvas loop, show it instead of
    /// the configured background.
    #[serde(default = "default_true")]
    pub prefer_canvas: bool,
    /// Fixed sRGB text colour. `None` picks a colour automatically from
    /// whatever is behind the text.
    #[serde(default)]
    pub text_color: Option<[f32; 3]>,
}

pub(super) fn default_true() -> bool {
    true
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            disable_blur: false,
            blur_opacity: 0.4,
            transparent_background: false,
            show_album_art: true,
            album_art_background: false,
            album_color_background: true,
            font_family: None,
            custom_background_path: None,
            video_background_path: None,
            prefer_canvas: true,
            text_color: None,
        }
    }
}
