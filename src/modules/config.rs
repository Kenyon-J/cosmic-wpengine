use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::mpsc::Sender;

use super::event::Event;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub mode: WallpaperMode,
    pub fps: u32,
    pub weather: WeatherConfig,
    pub audio: AudioConfig,
    #[serde(default)]
    pub appearance: AppearanceConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum TemperatureUnit {
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WallpaperMode {
    AlbumArt,
    AudioVisualiser,
    Weather,
    Auto,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WeatherConfig {
    pub enabled: bool,
    pub latitude: f64,
    pub longitude: f64,
    pub poll_interval_minutes: u64,
    #[serde(default = "default_temp_unit")]
    pub temperature_unit: TemperatureUnit,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioConfig {
    #[serde(default = "default_audio_style")]
    pub style: String,
    pub bands: usize,
    pub smoothing: f32,
    pub color_top: Option<[f32; 3]>,
    pub color_bottom: Option<[f32; 3]>,
    #[serde(default = "default_show_lyrics")]
    pub show_lyrics: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppearanceConfig {
    #[serde(default)]
    pub disable_blur: bool,
    #[serde(default)]
    pub transparent_background: bool,
    pub custom_background_path: Option<String>,
}

fn default_show_lyrics() -> bool { true }
fn default_audio_style() -> String { "monstercat".to_string() }
fn default_temp_unit() -> TemperatureUnit { TemperatureUnit::Celsius }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThemeLayout {
    #[serde(default = "default_album_art_layout")]
    pub album_art: ArtLayout,
    #[serde(default = "default_track_info_layout")]
    pub track_info: TextLayout,
    #[serde(default = "default_lyrics_layout")]
    pub lyrics: TextLayout,
    #[serde(default = "default_weather_layout")]
    pub weather: TextLayout,
    #[serde(default = "default_visualiser_layout")]
    pub visualiser: VisualiserLayout,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VisualiserLayout {
    #[serde(default = "default_vis_shape")]
    pub shape: VisShape,
    #[serde(default = "default_vis_position")]
    pub position: [f32; 2],
    #[serde(default = "default_vis_size")]
    pub size: f32,
    #[serde(default = "default_vis_rotation")]
    pub rotation: f32,
    #[serde(default = "default_vis_amplitude")]
    pub amplitude: f32,
    pub color_top: Option<[f32; 3]>,
    pub color_bottom: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Copy)]
#[serde(rename_all = "lowercase")]
pub enum VisShape {
    Circular,
    Linear,
}

fn default_vis_shape() -> VisShape { VisShape::Circular }
fn default_vis_position() -> [f32; 2] { [0.5, 0.5] }
fn default_vis_size() -> f32 { 0.25 }
fn default_vis_rotation() -> f32 { 0.0 }
fn default_vis_amplitude() -> f32 { 1.0 }

fn default_visualiser_layout() -> VisualiserLayout {
    VisualiserLayout {
        shape: default_vis_shape(),
        position: default_vis_position(),
        size: default_vis_size(),
        rotation: default_vis_rotation(),
        amplitude: default_vis_amplitude(),
        color_top: None,
        color_bottom: None,
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArtLayout {
    pub position: [f32; 2],
    pub size: f32,
    #[serde(default = "default_art_shape")]
    pub shape: ArtShape,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ArtShape {
    Square,
    Circular,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TextLayout {
    pub position: [f32; 2],
    pub align: TextAlign,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Copy)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

fn default_art_shape() -> ArtShape { ArtShape::Circular }
fn default_album_art_layout() -> ArtLayout { ArtLayout { position: [0.5, 0.5], size: 0.25, shape: default_art_shape() } }
fn default_track_info_layout() -> TextLayout { TextLayout { position: [0.5, 0.08], align: TextAlign::Center } }
fn default_lyrics_layout() -> TextLayout { TextLayout { position: [0.5, 0.82], align: TextAlign::Center } }
fn default_weather_layout() -> TextLayout { TextLayout { position: [0.98, 0.03], align: TextAlign::Right } }

impl AppearanceConfig {
    pub fn resolved_background_path(&self) -> Option<String> {
        if self.custom_background_path.is_some() {
            return self.custom_background_path.clone();
        }

        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_default();
                PathBuf::from(home).join(".config")
            });

        let cosmic_bg_dir = base.join("cosmic/com.system76.CosmicBackground/v1");

        if let Ok(entries) = std::fs::read_dir(cosmic_bg_dir) {
            for entry in entries.flatten() {
                if let Ok(contents) = std::fs::read_to_string(entry.path()) {
                    // COSMIC uses RON format, storing wallpaper paths like: Path("/path/to/img.jpg")
                    if let Some(start_idx) = contents.find("Path(\"") {
                        let path_start = start_idx + 6;
                        if let Some(end_offset) = contents[path_start..].find("\")") {
                            let path = &contents[path_start..path_start + end_offset];
                            if std::path::Path::new(path).exists() {
                                return Some(path.to_string());
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

impl Default for ThemeLayout {
    fn default() -> Self {
        Self {
            album_art: default_album_art_layout(),
            track_info: default_track_info_layout(),
            lyrics: default_lyrics_layout(),
            weather: default_weather_layout(),
            visualiser: default_visualiser_layout(),
        }
    }
}

impl ThemeLayout {
    pub fn load(style: &str) -> Self {
        let path = Config::config_dir().join("shaders").join(format!("{}.toml", style));
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(theme) = toml::from_str(&text) {
                return theme;
            } else {
                tracing::warn!("Failed to parse theme layout at {:?}. Using defaults.", path);
            }
        }
        
        let mut theme = Self::default();
        if style == "monstercat" {
            theme.visualiser.shape = VisShape::Linear;
            theme.visualiser.position = [0.5, 0.5];
            theme.visualiser.size = 1.0;
            theme.visualiser.amplitude = 1.5;
            theme.album_art.position = [0.15, 0.8];
            theme.album_art.size = 0.15;
            theme.album_art.shape = ArtShape::Square;
            theme.track_info.position = [0.28, 0.75];
            theme.track_info.align = TextAlign::Left;
            theme.lyrics.position = [0.28, 0.85];
            theme.lyrics.align = TextAlign::Left;
        } else if style == "waveform" {
            theme.visualiser.shape = VisShape::Circular;
            theme.album_art.shape = ArtShape::Circular;
        }
        theme
    }

    pub fn write_defaults() -> std::io::Result<()> {
        let shaders_dir = Config::config_dir().join("shaders");
        std::fs::create_dir_all(&shaders_dir)?;

        let bars_path = shaders_dir.join("bars.toml");
        if !bars_path.exists() {
            std::fs::write(&bars_path, r#"# ==============================================================================
# Bars Theme (Default)
# ==============================================================================
# A central, circular floating hub that radiates frequency bands outward.

[album_art]
position = [0.5, 0.5]
size = 0.25
shape = "circular"

[track_info]
position = [0.5, 0.08]
align = "center"

[lyrics]
position = [0.5, 0.82]
align = "center"

[weather]
position = [0.98, 0.03]
align = "right"

[visualiser]
shape = "circular"
position = [0.5, 0.5]
size = 0.25
rotation = 0.0
amplitude = 1.0
# color_top = [1.0, 0.2, 0.5]      # Optional fixed colours (RGB 0.0 - 1.0)
# color_bottom = [0.2, 0.5, 1.0]
"#)?;
        }

        let monstercat_path = shaders_dir.join("monstercat.toml");
        if !monstercat_path.exists() {
            std::fs::write(&monstercat_path, r#"# ==============================================================================
# Monstercat Theme
# ==============================================================================
# A sleek, linear audio visualiser layout inspired by Monstercat's videos.

[album_art]
position = [0.15, 0.7]
size = 0.10

[track_info]
position = [0.26, 0.65]
align = "left"

[lyrics]
position = [0.26, 0.75]
align = "left"

[weather]
position = [0.98, 0.03]
align = "right"

[visualiser]
shape = "linear"
position = [0.5, 0.95]
size = 0.85
rotation = 0.0
amplitude = 1.5
# color_top = [1.0, 0.2, 0.5]      # Optional fixed colours (RGB 0.0 - 1.0)
# color_bottom = [0.2, 0.5, 1.0]
"#)?;
        }

        let waveform_path = shaders_dir.join("waveform.toml");
        if !waveform_path.exists() {
            std::fs::write(&waveform_path, r#"# ==============================================================================
# Waveform Theme
# ==============================================================================
# Same layout as "bars", but optimized for the waveform audio style.

[album_art]
position = [0.5, 0.5]
size = 0.25
shape = "circular"

[track_info]
position = [0.5, 0.08]
align = "center"

[lyrics]
position = [0.5, 0.82]
align = "center"

[weather]
position = [0.98, 0.03]
align = "right"

[visualiser]
shape = "circular"
position = [0.5, 0.5]
size = 0.25
rotation = 0.0
amplitude = 1.0
# color_top = [1.0, 0.2, 0.5]      # Optional fixed colours (RGB 0.0 - 1.0)
# color_bottom = [0.2, 0.5, 1.0]
"#)?;
        }

        let waveform_wgsl_path = shaders_dir.join("waveform.wgsl");
        if !waveform_wgsl_path.exists() {
            std::fs::write(&waveform_wgsl_path, r#"struct VisualiserUniforms {
    resolution: vec2<f32>,
    band_count: u32,
    lyric_pulse: f32,
    color_top: vec4<f32>,
    color_bottom: vec4<f32>,
    style: u32,
    size: f32,
    position: vec2<f32>,
    rotation: f32,
    amplitude: f32,
    pad1: u32,
    pad2: u32,
}

@group(0) @binding(0) var<uniform> uniforms: VisualiserUniforms;
@group(0) @binding(1) var<storage, read> bands: array<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((idx << 1u) & 2u);
    let y = f32(idx & 2u);
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let aspect = uniforms.resolution.x / uniforms.resolution.y;

    let p = vec2<f32>((uv.x - uniforms.position.x) * aspect, uv.y - uniforms.position.y);
    let s = sin(uniforms.rotation);
    let c = cos(uniforms.rotation);
    let p_rot = vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
    let angle = atan2(p_rot.y, p_rot.x) + 3.14159; 
    
    let normalized_angle = angle / 6.28318;
    var f_band = normalized_angle * 2.0;
    if f_band > 1.0 { f_band = 2.0 - f_band; }

    let band_idx = min(u32(f_band * f32(uniforms.band_count)), uniforms.band_count - 1u);
    let next_idx = min(band_idx + 1u, uniforms.band_count - 1u);
    let fract_band = fract(f_band * f32(uniforms.band_count));

    let val1 = bands[band_idx];
    let val2 = bands[next_idx];
    let val = mix(val1, val2, fract_band);

    let wave_radius = uniforms.size + (val * uniforms.amplitude * 0.1) + (uniforms.lyric_pulse * 0.02);
    let d = length(p_rot);
    let dist_to_line = abs(d - wave_radius);

    let line_thickness = 0.005;
    if dist_to_line < line_thickness {
        return vec4<f32>(uniforms.color_top.rgb, 1.0);
    }

    let glow = clamp(0.002 / (dist_to_line * dist_to_line + 0.002) - 0.05, 0.0, 1.0);
    return vec4<f32>(uniforms.color_top.rgb * glow * 1.5, min(glow * 1.5, 1.0));
}
"#)?;
        }

        Ok(())
    }
}

impl Config {
    pub fn load_or_default() -> Result<Self> {
        let path = Self::config_path();
        
        // Extract default themes so users can find and edit them!
        let _ = ThemeLayout::write_defaults();

        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&text)?;
            Ok(config)
        } else {
            let config = Config::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, toml::to_string_pretty(&config)?)?;
            tracing::info!("Created default config at {:?}", path);
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn config_dir() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").expect("HOME not set");
                PathBuf::from(home).join(".config")
            });
        base.join("cosmic-wallpaper")
    }

    fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub async fn watch(tx: Sender<Event>) -> Result<()> {
        let path = Self::config_path();
        let parent = path.parent().unwrap_or(std::path::Path::new("")).to_path_buf();
        let path_clone = path.clone();
        let shaders_dir = parent.join("shaders");

        let cosmic_bg_dir = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_default();
                PathBuf::from(home).join(".config")
            })
            .join("cosmic/com.system76.CosmicBackground/v1");

        let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel();

        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = notify_tx.send(res);
        })?;

        watcher.watch(&parent, RecursiveMode::NonRecursive)?;
        if shaders_dir.exists() {
            let _ = watcher.watch(&shaders_dir, RecursiveMode::NonRecursive);
        }
        if cosmic_bg_dir.exists() {
            let _ = watcher.watch(&cosmic_bg_dir, RecursiveMode::NonRecursive);
        }

        while let Some(res) = notify_rx.recv().await {
            if let Ok(event) = res {
                let is_our_config = event.paths.iter().any(|p| p == &path_clone);
                let is_our_shader = event.paths.iter().any(|p| p.starts_with(&shaders_dir));
                let is_cosmic_bg = event.paths.iter().any(|p| p.starts_with(&cosmic_bg_dir));

                if is_our_config || is_our_shader || is_cosmic_bg {
                    // Slight debounce to ensure the text editor has finished writing the file
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    if let Ok(config) = Self::load_or_default() {
                        let _ = tx.send(Event::ConfigUpdated(config)).await;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: WallpaperMode::Auto,
            fps: 30,
            weather: WeatherConfig {
                enabled: false,
                latitude: 51.5,
                longitude: -0.1,
                poll_interval_minutes: 15,
                temperature_unit: TemperatureUnit::Celsius,
            },
            audio: AudioConfig {
                style: "monstercat".to_string(),
                bands: 64,
                smoothing: 0.7,
                color_top: None,
                color_bottom: None,
                show_lyrics: true,
            },
            appearance: AppearanceConfig::default(),
        }
    }
}
