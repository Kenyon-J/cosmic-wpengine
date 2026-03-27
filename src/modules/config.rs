use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::mpsc::Sender;

use super::event::Event;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub mode: WallpaperMode,
    pub fps: u32,
    pub weather: WeatherConfig,
    pub audio: AudioConfig,
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
#[serde(default)]
pub struct WeatherConfig {
    pub enabled: bool,
    pub latitude: f64,
    pub longitude: f64,
    pub poll_interval_minutes: u64,
    pub temperature_unit: TemperatureUnit,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            latitude: 51.5,
            longitude: -0.1,
            poll_interval_minutes: 15,
            temperature_unit: TemperatureUnit::Celsius,
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
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            style: "monstercat".to_string(),
            bands: 64,
            smoothing: 0.7,
            show_lyrics: true,
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
    pub custom_background_path: Option<String>,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            disable_blur: false,
            blur_opacity: 0.4,
            transparent_background: false,
            show_album_art: true,
            album_art_background: true,
            custom_background_path: None,
        }
    }
}
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
    #[serde(default)]
    pub effects: EffectsLayout,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EffectsLayout {
    #[serde(default = "default_lyric_bounce")]
    pub lyric_bounce: f32,
    #[serde(default = "default_lyric_spring_stiffness")]
    pub lyric_spring_stiffness: f32,
    #[serde(default = "default_lyric_spring_damping")]
    pub lyric_spring_damping: f32,
    #[serde(default = "default_beat_pulse")]
    pub beat_pulse: f32,
}

impl Default for EffectsLayout {
    fn default() -> Self {
        Self {
            lyric_bounce: 1.0,
            lyric_spring_stiffness: 150.0,
            lyric_spring_damping: 12.0,
            beat_pulse: 1.0,
        }
    }
}

fn default_lyric_bounce() -> f32 {
    1.0
}
fn default_lyric_spring_stiffness() -> f32 {
    150.0
}
fn default_lyric_spring_damping() -> f32 {
    12.0
}
fn default_beat_pulse() -> f32 {
    1.0
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
    #[serde(default = "default_vis_align")]
    pub align: VisAlign,
    pub color_top: Option<[f32; 3]>,
    pub color_bottom: Option<[f32; 3]>,
    pub shader: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Copy)]
#[serde(rename_all = "lowercase")]
pub enum VisShape {
    Circular,
    Linear,
    Square,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Copy)]
#[serde(rename_all = "lowercase")]
pub enum VisAlign {
    Left,
    Center,
    Right,
}

fn default_vis_align() -> VisAlign {
    VisAlign::Left
}
fn default_vis_shape() -> VisShape {
    VisShape::Circular
}
fn default_vis_position() -> [f32; 2] {
    [0.5, 0.5]
}
fn default_vis_size() -> f32 {
    0.25
}
fn default_vis_rotation() -> f32 {
    0.0
}
fn default_vis_amplitude() -> f32 {
    1.0
}

fn default_visualiser_layout() -> VisualiserLayout {
    VisualiserLayout {
        shape: default_vis_shape(),
        position: default_vis_position(),
        size: default_vis_size(),
        rotation: default_vis_rotation(),
        amplitude: default_vis_amplitude(),
        align: default_vis_align(),
        color_top: None,
        color_bottom: None,
        shader: None,
    }
}

fn default_art_position() -> [f32; 2] {
    [0.5, 0.5]
}
fn default_art_size() -> f32 {
    0.25
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArtLayout {
    #[serde(default = "default_art_position")]
    pub position: [f32; 2],
    #[serde(default = "default_art_size")]
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

fn default_text_position() -> [f32; 2] {
    [0.5, 0.5]
}
fn default_text_align() -> TextAlign {
    TextAlign::Center
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TextLayout {
    #[serde(default = "default_text_position")]
    pub position: [f32; 2],
    #[serde(default = "default_text_align")]
    pub align: TextAlign,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Copy)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

fn default_art_shape() -> ArtShape {
    ArtShape::Circular
}
fn default_album_art_layout() -> ArtLayout {
    ArtLayout {
        position: [0.5, 0.5],
        size: 0.25,
        shape: default_art_shape(),
    }
}
fn default_track_info_layout() -> TextLayout {
    TextLayout {
        position: [0.5, 0.08],
        align: TextAlign::Center,
    }
}
fn default_lyrics_layout() -> TextLayout {
    TextLayout {
        position: [0.5, 0.82],
        align: TextAlign::Center,
    }
}
fn default_weather_layout() -> TextLayout {
    TextLayout {
        position: [0.98, 0.03],
        align: TextAlign::Right,
    }
}

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

        let mut latest_path = None;
        let mut latest_time = std::time::SystemTime::UNIX_EPOCH;

        if let Ok(entries) = std::fs::read_dir(cosmic_bg_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if modified > latest_time {
                            if let Ok(contents) = std::fs::read_to_string(entry.path()) {
                                // COSMIC uses RON format, storing wallpaper paths like: Path("/path/to/img.jpg")
                                if let Some(start_idx) = contents.find("Path(\"") {
                                    let path_start = start_idx + 6;
                                    if let Some(end_offset) = contents[path_start..].find("\")") {
                                        let path = &contents[path_start..path_start + end_offset];
                                        if std::path::Path::new(path).exists() {
                                            latest_path = Some(path.to_string());
                                            latest_time = modified;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        latest_path
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
            effects: EffectsLayout::default(),
        }
    }
}

impl ThemeLayout {
    pub fn load(style: &str) -> Self {
        let path = Config::config_dir()
            .join("shaders")
            .join(format!("{}.toml", style));
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(theme) = toml::from_str(&text) {
                return theme;
            } else {
                tracing::warn!(
                    "Failed to parse theme layout at {:?}. Using defaults.",
                    path
                );
            }
        }

        let mut theme = Self::default();
        if style == "monstercat" {
            theme.visualiser.shape = VisShape::Linear;
            theme.visualiser.position = [0.5, 0.5];
            theme.visualiser.size = 0.6;
            theme.visualiser.rotation = 0.0;
            theme.visualiser.amplitude = 1.5;
            theme.album_art.position = [0.15, 0.7];
            theme.album_art.size = 0.10;
            theme.track_info.position = [0.30, 0.51];
            theme.track_info.align = TextAlign::Left;
            theme.lyrics.position = [0.56, 0.56];
            theme.lyrics.align = TextAlign::Left;
        } else if style == "symmetric" {
            theme.visualiser.shape = VisShape::Linear;
            theme.visualiser.position = [0.5, 0.85];
            theme.visualiser.size = 0.8;
            theme.visualiser.align = VisAlign::Center;
            theme.album_art.position = [0.5, 0.3];
            theme.album_art.size = 0.15;
            theme.track_info.position = [0.5, 0.5];
            theme.track_info.align = TextAlign::Center;
            theme.lyrics.position = [0.5, 0.6];
            theme.lyrics.align = TextAlign::Center;
            theme.weather.position = [0.98, 0.03];
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
            std::fs::write(
                &bars_path,
                r#"# ==============================================================================
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
rotation = 0.0 # Visualiser angle in degrees (0.0 to 360.0)
amplitude = 1.0
# shader = "bars.wgsl" # Optional: Path to a custom .wgsl shader in this folder

[effects]
lyric_bounce = 0.5 # Dialed down for cleaner UI
lyric_spring_stiffness = 150.0
lyric_spring_damping = 12.0 # Slightly underdamped for a natural spring overshoot
beat_pulse = 0.5
# color_top = [1.0, 0.2, 0.5]      # Optional fixed colours (RGB 0.0 - 1.0)
# color_bottom = [0.2, 0.5, 1.0]
"#,
            )?;
        }

        let monstercat_path = shaders_dir.join("monstercat.toml");
        if !monstercat_path.exists() {
            std::fs::write(
                &monstercat_path,
                r#"# ==============================================================================
# Monstercat Theme
# ==============================================================================
# A sleek, linear audio visualiser layout inspired by Monstercat's videos.

[album_art]
position = [0.15, 0.7]
size = 0.10

[track_info]
position = [0.30, 0.51]
align = "left"

[lyrics]
position = [0.56, 0.56]
align = "left"

[weather]
position = [0.98, 0.03]
align = "right"

[visualiser]
shape = "linear"
position = [0.5, 0.5]
size = 0.6
rotation = 0
amplitude = 1.5
# color_top = [1.0, 0.2, 0.5]      # Optional fixed colours (RGB 0.0 - 1.0)
# color_bottom = [0.2, 0.5, 1.0]
"#,
            )?;
        }

        let waveform_path = shaders_dir.join("waveform.toml");
        if !waveform_path.exists() {
            std::fs::write(
                &waveform_path,
                r#"# ==============================================================================
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
rotation = 0.0 # Visualiser angle in degrees
amplitude = 1.0
# shader = "waveform.wgsl" # Optional: Path to a custom .wgsl shader in this folder

[effects]
lyric_bounce = 0.5
lyric_spring_stiffness = 150.0
lyric_spring_damping = 12.0
beat_pulse = 0.5
# color_top = [1.0, 0.2, 0.5]      # Optional fixed colours (RGB 0.0 - 1.0)
# color_bottom = [0.2, 0.5, 1.0]
"#,
            )?;
        }

        let waveform_wgsl_path = shaders_dir.join("waveform.wgsl");
        let write_waveform = !waveform_wgsl_path.exists()
            || !std::fs::read_to_string(&waveform_wgsl_path).is_ok_and(|c| c.contains("// v5"));
        if write_waveform {
            std::fs::write(
                &waveform_wgsl_path,
                r#"// v5
struct VisualiserUniforms {
    resolution: vec2<f32>,
    band_count: u32,
    lyric_pulse: f32,
    color_top: vec4<f32>,
    color_bottom: vec4<f32>,
    pos_size_rot: vec4<f32>,
    amplitude: f32,
    style: u32,
    time: f32,
    align: u32,
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

    let p = vec2<f32>((uv.x - uniforms.pos_size_rot.x) * aspect, uv.y - uniforms.pos_size_rot.y);
    let s = sin(uniforms.pos_size_rot.w);
    let c = cos(uniforms.pos_size_rot.w);
    let p_rot = vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
    let angle = atan2(p_rot.y, p_rot.x) + 3.14159; 
    
    let normalized_angle = angle / 6.28318;
    // Double the waveform around the circle for perfect symmetry
    var f_band = normalized_angle * 2.0;
    if f_band > 1.0 { f_band = 2.0 - f_band; }

    let band_idx = min(u32(f_band * f32(uniforms.band_count)), uniforms.band_count - 1u);
    let next_idx = min(band_idx + 1u, uniforms.band_count - 1u);
    let fract_band = fract(f_band * f32(uniforms.band_count));

    // Smooth cubic interpolation for organic curves
    let val1 = bands[band_idx];
    let val2 = bands[next_idx];
    let smooth_fract = fract_band * fract_band * (3.0 - 2.0 * fract_band);
    let val = mix(val1, val2, smooth_fract);

    // Modern symmetrical ribbon with displacement
    let base_radius = uniforms.pos_size_rot.z + (uniforms.lyric_pulse * 0.02);
    let wave_offset = val * uniforms.amplitude * 0.1;
    
    let displaced_radius = base_radius + (wave_offset * 0.5);
    let d = length(p_rot);
    let dist_to_line = abs(d - displaced_radius);
    
    // Thickness expands dynamically with the wave energy
    let thickness = abs(wave_offset * 0.75) + 0.003 + (uniforms.lyric_pulse * 0.005);
    
    // Anti-aliased soft edge
    let edge = smoothstep(thickness + 0.005, thickness - 0.005, dist_to_line);
    
    // Smooth gradient coloring matching the theme bounds
    let gradient_factor = (p_rot.y + uniforms.pos_size_rot.z) / (uniforms.pos_size_rot.z * 2.0);
    let base_color = mix(uniforms.color_bottom.rgb, uniforms.color_top.rgb, clamp(gradient_factor, 0.0, 1.0));
    
    // Bright neon core highlight
    let core = smoothstep(0.005, 0.0, dist_to_line) * 0.6;
    
    // Ethereal outer bloom
    let glow = exp(-dist_to_line * 20.0) * 0.5;
    
    let final_color = base_color * edge + vec3<f32>(core) + (base_color * glow);
    let final_alpha = max(edge, glow);

    if final_alpha < 0.01 { discard; }

    return vec4<f32>(final_color, final_alpha);
}
"#,
            )?;
        }

        let symmetric_path = shaders_dir.join("symmetric.toml");
        if !symmetric_path.exists() {
            std::fs::write(
                &symmetric_path,
                r#"# ==============================================================================
# Symmetric Theme
# ==============================================================================
# A center-aligned visualizer layout that mirrors frequencies perfectly.

[album_art]
position = [0.5, 0.3]
size = 0.15

[track_info]
position = [0.5, 0.5]
align = "center"

[lyrics]
position = [0.5, 0.6]
align = "center"

[visualiser]
shape = "linear"
position = [0.5, 0.85]
size = 0.8
align = "center"
"#,
            )?;
        }

        let monstercat_wgsl_path = shaders_dir.join("monstercat.wgsl");
        let write_monstercat = !monstercat_wgsl_path.exists()
            || !std::fs::read_to_string(&monstercat_wgsl_path).is_ok_and(|c| c.contains("// v5"));
        if write_monstercat {
            std::fs::write(
                &monstercat_wgsl_path,
                r#"// v5
struct VisualiserUniforms {
    resolution: vec2<f32>,
    band_count: u32,
    lyric_pulse: f32,
    color_top: vec4<f32>,
    color_bottom: vec4<f32>,
    pos_size_rot: vec4<f32>,
    amplitude: f32,
    style: u32,
    time: f32,
    align: u32,
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
    
    // Shift origin to configured position
    let shifted = uv - vec2<f32>(uniforms.pos_size_rot.x, uniforms.pos_size_rot.y);
    
    let s = sin(uniforms.pos_size_rot.w);
    let c = cos(uniforms.pos_size_rot.w);
    
    // Apply rotation, compensating for aspect ratio so it doesn't skew!
    let p_rot = vec2<f32>(
        (shifted.x * aspect * c - shifted.y * s) / aspect,
        shifted.x * aspect * s + shifted.y * c
    );
    
    let half_width = uniforms.pos_size_rot.z * 0.5;
    if p_rot.x < -half_width || p_rot.x > half_width { discard; }
    
    let norm_x = (p_rot.x + half_width) / uniforms.pos_size_rot.z;
    
    var mapped_x = norm_x;
    if uniforms.align == 1u { // Center (mirrored out from the middle)
        mapped_x = abs(norm_x - 0.5) * 2.0;
    } else if uniforms.align == 2u { // Right
        mapped_x = 1.0 - norm_x;
    }
    
    let band_idx = min(u32(mapped_x * f32(uniforms.band_count)), uniforms.band_count - 1u);
    let val = bands[band_idx] * uniforms.amplitude;
    
    // Bars grow UPWARDS from the baseline (y = 0)
    let height = (val * 0.25) + 0.005;
    
    // UV Y-axis increases downwards, so 'above' the baseline means negative p_rot.y
    if p_rot.y > 0.0 || p_rot.y < -height { discard; }
    
    let gradient = abs(p_rot.y) / height;
    let color = mix(uniforms.color_bottom.rgb, uniforms.color_top.rgb, gradient);
    
    return vec4<f32>(color, 1.0);
}
"#,
            )?;
        }

        let bars_wgsl_path = shaders_dir.join("bars.wgsl");
        let write_bars = !bars_wgsl_path.exists()
            || !std::fs::read_to_string(&bars_wgsl_path).is_ok_and(|c| c.contains("// v5"));
        if write_bars {
            std::fs::write(
                &bars_wgsl_path,
                r#"// v5
struct VisualiserUniforms {
    resolution: vec2<f32>,
    band_count: u32,
    lyric_pulse: f32,
    color_top: vec4<f32>,
    color_bottom: vec4<f32>,
    pos_size_rot: vec4<f32>,
    amplitude: f32,
    style: u32,
    time: f32,
    align: u32,
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
    
    let p = vec2<f32>((uv.x - uniforms.pos_size_rot.x) * aspect, uv.y - uniforms.pos_size_rot.y);
    let s = sin(uniforms.pos_size_rot.w);
    let c = cos(uniforms.pos_size_rot.w);
    let p_rot = vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
    
    let angle = atan2(p_rot.y, p_rot.x) + 3.14159; 
    let normalized_angle = angle / 6.28318;
    var f_band = normalized_angle * 2.0;
    if f_band > 1.0 { f_band = 2.0 - f_band; }

    let band_idx = min(u32(f_band * f32(uniforms.band_count)), uniforms.band_count - 1u);
    let val = bands[band_idx];

    let d = length(p_rot);
    let inner_radius = uniforms.pos_size_rot.z + (uniforms.lyric_pulse * 0.02);
    let bar_height = val * uniforms.amplitude * 0.2;
    let outer_radius = inner_radius + bar_height;

    if d < inner_radius || d > outer_radius {
        discard;
    }

    let fract_band = fract(f_band * f32(uniforms.band_count));
    if fract_band < 0.1 || fract_band > 0.9 {
        discard;
    }

    let gradient = (d - inner_radius) / bar_height;
    let color = mix(uniforms.color_bottom.rgb, uniforms.color_top.rgb, gradient);
    
    return vec4<f32>(color, 1.0);
}
"#,
            )?;
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
            match toml::from_str(&text) {
                Ok(config) => Ok(config),
                Err(e) => {
                    tracing::error!(
                        "Syntax error in config.toml: {}. Falling back to default configuration!",
                        e
                    );
                    let _ = std::fs::rename(&path, path.with_extension("toml.bak"));
                    let default_config = Config::default();
                    let _ = std::fs::write(&path, toml::to_string_pretty(&default_config)?);
                    Ok(default_config)
                }
            }
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

    #[allow(dead_code)]
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
        let parent = path
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .to_path_buf();
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
                    // FLUSH pending events to prevent GUI sliders from queueing 100+ re-renders and locking up the engine!
                    while notify_rx.try_recv().is_ok() {}

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
                show_lyrics: true,
            },
            appearance: AppearanceConfig::default(),
        }
    }
}
