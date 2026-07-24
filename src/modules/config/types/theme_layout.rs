use super::app_config::default_true;
use crate::modules::config::Config;
use serde::{Deserialize, Serialize};

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
    /// Font family for this style's text. Overridden by the user's global
    /// `appearance.font_family` when that is set.
    #[serde(default)]
    pub font_family: Option<String>,
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
    /// Circular visualisers historically capture the album art into their
    /// ring (art takes the ring's position and size while audio plays).
    /// On by default to preserve that look; off gives the art layout's own
    /// position and size full effect.
    #[serde(default = "default_true")]
    pub dock_art: bool,
    /// Bar width as a fraction of the space allotted to each band (was a
    /// hardcoded 0.85 in the shader; 1.0 butts bars together with no gap).
    #[serde(default = "default_vis_bar_width_ratio")]
    pub bar_width_ratio: f32,
    /// Corner rounding on each bar, 0.0 (hard rectangle) to 1.0 (full
    /// capsule/pill - semicircular caps once the bar is taller than it is
    /// wide).
    #[serde(default = "default_vis_cap_radius")]
    pub cap_radius: f32,
    /// Strength of the mirrored "glass floor" reflection below the
    /// baseline, 0.0 (off) to 1.0 (full strength, still fading with depth).
    #[serde(default = "default_vis_reflection")]
    pub reflection: f32,
    /// Draws a small bright cap that holds each bar's recent peak and falls
    /// back down under gravity, independent of the live smoothed height.
    /// Off by default: tried against the built-in themes and judged not
    /// worth it visually (a floating, oddly-bordered mark) - kept as an
    /// opt-in for anyone who wants it rather than dropped outright.
    #[serde(default)]
    pub peak_hold: bool,
    /// Chops each bar into this many discrete LED-style segments with small
    /// gaps between them. 0 (default) keeps the continuous bar.
    #[serde(default)]
    pub led_segments: u32,
    /// Multiplier on the soft glow above each bar's tip, 0.0 (none, a flat
    /// crisp edge) to 1.0 (full glow). Themes going for a minimal/flat look
    /// (e.g. Monstercat's real-world namesake) want this at 0.
    #[serde(default = "default_vis_glow_strength")]
    pub glow_strength: f32,
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
fn default_vis_bar_width_ratio() -> f32 {
    0.85
}
fn default_vis_cap_radius() -> f32 {
    1.0
}
fn default_vis_reflection() -> f32 {
    0.35
}
fn default_vis_glow_strength() -> f32 {
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
        dock_art: true,
        bar_width_ratio: default_vis_bar_width_ratio(),
        cap_radius: default_vis_cap_radius(),
        reflection: default_vis_reflection(),
        peak_hold: false,
        led_segments: 0,
        glow_strength: default_vis_glow_strength(),
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
fn default_text_size() -> f32 {
    1.0
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TextLayout {
    #[serde(default = "default_text_position")]
    pub position: [f32; 2],
    #[serde(default = "default_text_align")]
    pub align: TextAlign,
    /// Scale multiplier on the element's computed font size (1.0 = default).
    #[serde(default = "default_text_size")]
    pub size: f32,
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
        position: [0.5, 0.10],
        align: TextAlign::Center,
        size: 1.0,
    }
}
fn default_lyrics_layout() -> TextLayout {
    TextLayout {
        position: [0.5, 0.85],
        align: TextAlign::Center,
        size: 1.0,
    }
}
fn default_weather_layout() -> TextLayout {
    TextLayout {
        position: [0.98, 0.05],
        align: TextAlign::Right,
        size: 1.0,
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
            font_family: None,
        }
    }
}

impl ThemeLayout {
    pub fn load(style: &str) -> Self {
        let style_name = std::path::Path::new(style)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("visualiser");
        let path = Config::config_dir()
            .join("shaders")
            .join(format!("{}.toml", style_name));
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

        Self::builtin_default(style)
    }

    /// The hand-tuned look a built-in style name ships with - `bars` (and
    /// any other/unknown name) is just `Self::default()`; `monstercat`,
    /// `symmetric` and `waveform` get their own overrides below. Ignores
    /// any file on disk, unlike `load()`: this is "what does this name
    /// ship with", used both as `load()`'s no-file-yet fallback and as the
    /// theme editor's "Reset to defaults" target, which must restore the
    /// shipped look even after the style's own file has been edited and
    /// autosaved (at which point `load()` itself would just return those
    /// edited values back).
    pub fn builtin_default(style: &str) -> Self {
        let mut theme = Self::default();
        if style == "monstercat" {
            theme.visualiser.shape = VisShape::Linear;
            theme.visualiser.position = [0.5, 0.5];
            theme.visualiser.size = 0.6;
            theme.visualiser.rotation = 0.0;
            theme.visualiser.amplitude = 1.5;
            // Matches the real Monstercat visualiser's namesake look: flat
            // bars, no bloom, tightly packed. Colour is deliberately left
            // at its default (None - adaptive to the wallpaper/album art)
            // rather than pinned to Monstercat's own green, since real
            // Monstercat-style visualisers vary their fill colour by genre;
            // this theme is about the *shape*, not a fixed hue.
            theme.visualiser.cap_radius = 0.0;
            theme.visualiser.reflection = 0.0;
            theme.visualiser.glow_strength = 0.0;
            theme.visualiser.bar_width_ratio = 0.7;
            theme.album_art.position = [0.24, 0.59];
            theme.album_art.size = 0.15;
            theme.album_art.shape = ArtShape::Square;
            theme.track_info.position = [0.29, 0.56];
            theme.track_info.align = TextAlign::Left;
            theme.lyrics.position = [0.49, 0.72];
            theme.lyrics.align = TextAlign::Left;
            theme.font_family = Some("Inter".to_string());
        } else if style == "symmetric" {
            theme.visualiser.shape = VisShape::Linear;
            theme.visualiser.position = [0.5, 0.85];
            theme.visualiser.size = 0.8;
            theme.visualiser.align = VisAlign::Center;
            theme.album_art.position = [0.5, 0.3];
            theme.album_art.size = 0.15;
            theme.track_info.position = [0.5, 0.45];
            theme.track_info.align = TextAlign::Center;
            theme.lyrics.position = [0.5, 0.55];
            theme.lyrics.align = TextAlign::Center;
            theme.weather.position = [0.98, 0.03];
            theme.font_family = Some("Inter".to_string());
        } else if style == "waveform" {
            theme.visualiser.shape = VisShape::Circular;
            theme.album_art.shape = ArtShape::Circular;
            theme.font_family = Some("Fira Sans".to_string());
        }
        theme
    }

    pub fn write_defaults() -> std::io::Result<()> {
        let shaders_dir = Config::config_dir().join("shaders");
        std::fs::create_dir_all(&shaders_dir)?;

        Self::write_if_absent(
            &shaders_dir.join("bars.toml"),
            include_str!("default_themes/bars.toml"),
        )?;
        Self::write_if_absent(
            &shaders_dir.join("monstercat.toml"),
            include_str!("default_themes/monstercat.toml"),
        )?;
        Self::write_if_absent(
            &shaders_dir.join("waveform.toml"),
            include_str!("default_themes/waveform.toml"),
        )?;
        Self::write_if_absent(
            &shaders_dir.join("symmetric.toml"),
            include_str!("default_themes/symmetric.toml"),
        )?;

        // This single, unified shader file is now included directly in the binary
        // and no longer needs to be written to disk. Users can still create their
        // own .wgsl files and point to them from a theme's .toml file.
        let _ = std::fs::remove_file(shaders_dir.join("bars.wgsl"));
        let _ = std::fs::remove_file(shaders_dir.join("monstercat.wgsl"));
        let _ = std::fs::remove_file(shaders_dir.join("waveform.wgsl"));

        // Written purely for discoverability - a starting point for anyone who
        // wants to point a theme's `shader` field at a tweaked copy - and kept
        // byte-identical to the engine's actual compiled-in default via the same
        // `include_str!` source, so it can never silently drift the way a
        // hand-duplicated copy previously did (that copy was frozen at an old
        // "// v20" snapshot while the real shader kept evolving under it -
        // reflection, LED segments, peak-hold, the square shape - none of which
        // ever reached the on-disk file).
        let default_shader_path = shaders_dir.join("visualiser.wgsl");
        let embedded_shader = include_str!("../../visualiser.wgsl");
        let needs_write = std::fs::read_to_string(&default_shader_path)
            .map(|existing| existing != embedded_shader)
            .unwrap_or(true);
        if needs_write {
            std::fs::write(&default_shader_path, embedded_shader)?;
        }

        Ok(())
    }

    /// Writes `contents` to `path` only if nothing is there yet - never
    /// overwrites a file the user may have since edited.
    fn write_if_absent(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(mut f) => {
                use std::io::Write;
                f.write_all(contents.as_bytes())
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(e) => Err(e),
        }
    }
}
