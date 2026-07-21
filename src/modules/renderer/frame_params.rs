//! Per-frame derived parameters for `draw_frame` (phase 1 of
//! PLAN-renderer-decomposition.md).
//!
//! Everything the per-output render loop reads that is invariant across
//! outputs within one frame - visibility gates, colour lerps, uniform
//! inputs, layout constants - derived here in one pass over `Renderer`.
//! Computing a `FrameParams` performs no GPU work and takes `&Renderer`,
//! so the loop that follows is free to take its mutable borrows.

use crate::modules::colour::{lerp_colour, time_to_sky_colour};
use crate::modules::config::{
    ArtShape, TextAlign, VisAlign, VisShape, VisualiserLayout, WallpaperMode,
};
use crate::modules::event::WeatherCondition;
use crate::modules::state::SceneHint;
use tracing::warn;

pub(crate) struct FrameParams {
    pub(crate) has_audio: bool,
    /// Base volume energy combined with the treble pulse, capped to
    /// prevent blown-out flashing.
    pub(crate) audio_energy: f32,
    pub(crate) show_art_fg: bool,
    pub(crate) show_art_bg: bool,
    pub(crate) show_color_bg: bool,
    pub(crate) clear_colour: wgpu::Color,
    pub(crate) is_weather_active: bool,
    pub(crate) active_particles: u32,
    pub(crate) top_col: [f32; 4],
    pub(crate) bottom_col: [f32; 4],
    pub(crate) art_tint_color: [f32; 3],
    pub(crate) elapsed: f32,
    pub(crate) track_hash: u64,
    pub(crate) weather_hash: u64,
    /// `Some` only when the procedural sky can be on screen (no custom
    /// background bound): (elapsed, weather type, sky colour).
    pub(crate) sky_color_data: Option<(f32, u32, [f32; 3])>,
    pub(crate) vis_shape_u32: u32,
    pub(crate) vis_align_u32: u32,
    pub(crate) vis_pos_size_rot: [f32; 4],
    pub(crate) is_waveform_u32: u32,
    pub(crate) album_art_bg_mode: u32,
    pub(crate) album_art_bg_alpha: f32,
    pub(crate) album_art_aspect: f32,
    pub(crate) album_art_fg_pos: [f32; 2],
    pub(crate) album_art_fg_size: f32,
    pub(crate) album_art_fg_shape: u32,
    pub(crate) custom_bg_mode: u32,
    pub(crate) custom_bg_alpha: f32,
    pub(crate) custom_bg_aspect: f32,
    pub(crate) blur_opacity: f32,
    pub(crate) visualiser_instance_count: u32,
    /// Inclusive 1-based bounds of the visible lyric window; an empty
    /// range (start > end) when there are no lyrics.
    pub(crate) lyric_start_idx: usize,
    pub(crate) lyric_end_idx: usize,
    /// Screen-invariant foreground art transform constants; per output,
    /// only the aspect-dependent terms remain to be applied.
    pub(crate) fg_k1: f32,
    pub(crate) fg_k2: f32,
    pub(crate) fg_k3: f32,
    pub(crate) fg_scale_y: f32,
    pub(crate) fg_offset_y: f32,
    pub(crate) secondary_text: [f32; 4],
    pub(crate) text_color_diff: [f32; 4],
    pub(crate) lyrics_align: cosmic_text::Align,
    pub(crate) track_info_align: cosmic_text::Align,
    pub(crate) weather_align: cosmic_text::Align,
    /// Owned (not borrowed from renderer state) so the loop can build a
    /// borrow-free `Attrs` while holding `&mut Renderer` - see the borrow
    /// note on `prepare_text_buffer`.
    pub(crate) font_family: Option<String>,
    pub(crate) lyric_bounce: f32,
    /// `beat_pulse * 2.0` - the multiplier guarantees visible beat effects.
    pub(crate) beat_pulse_mul: f32,
}

/// Audio-reactive elements draw only while something is actually audible
/// (or the visualiser is forced on), never in the forced weather/art modes,
/// and - unless forced - only while a track exists to attribute the audio to.
fn has_audio_active(audio_max_energy: f32, mode: &WallpaperMode, has_track: bool) -> bool {
    let force_vis = *mode == WallpaperMode::AudioVisualiser;
    (audio_max_energy > 0.001 || force_vis)
        && *mode != WallpaperMode::Weather
        && *mode != WallpaperMode::AlbumArt
        && (has_track || force_vis)
}

/// Circular visualisers capture the album art into their ring while audio
/// plays, unless the theme opts out via `dock_art = false`.
fn dock_art_active(has_audio: bool, vis: &VisualiserLayout) -> bool {
    has_audio && vis.shape == VisShape::Circular && vis.dock_art
}

/// Inclusive 1-based bounds of the ±2-line lyric window around the current
/// line. `None` (no lyrics) yields the empty range (1, 0).
fn lyric_window_bounds(current_lyric_idx: usize, lyrics_len: Option<usize>) -> (usize, usize) {
    match lyrics_len {
        Some(len) => (
            current_lyric_idx.saturating_sub(2).max(1),
            (current_lyric_idx + 2).min(len),
        ),
        None => (1, 0),
    }
}

impl FrameParams {
    pub(crate) fn compute(renderer: &super::Renderer) -> Self {
        let force_art = renderer.state.config.mode == WallpaperMode::AlbumArt;

        let has_audio = has_audio_active(
            renderer.audio.max_energy,
            &renderer.state.config.mode,
            renderer.state.current_track.is_some(),
        );

        // Combine the pre-calculated base volume energy with the snappy
        // treble pulse, strictly capped to prevent blown out flashing.
        let audio_energy =
            (renderer.audio.base_energy * 0.3 + renderer.audio.treble_pulse * 0.4).clamp(0.0, 1.0);

        // The state flag and the GPU resources can disagree for a frame
        // around track changes; the GPU resources are what actually draw,
        // so they are the authority (and the mismatch is logged).
        let has_media_check_state = renderer.state.has_album_art;
        let has_media_check_gpu = renderer.art.fg_bind_group().is_some();
        if has_media_check_gpu && !has_media_check_state {
            warn!("Album art visibility check mismatch! State: false, GPU: true. Using GPU state.");
        }

        // Art visibility is decoupled from force_vis so the visualiser and
        // the album art can layer.
        let appearance = &renderer.state.config.appearance;
        let show_art_fg = (has_media_check_gpu || force_art) && appearance.show_album_art;
        let show_art_bg = (has_media_check_gpu || force_art) && appearance.album_art_background;
        let show_color_bg = (has_media_check_gpu || force_art) && appearance.album_color_background;

        let weather_type = renderer.weather_type;
        let final_sky = get_final_sky_color(renderer);
        let clear_colour = get_clear_colour_from_sky(renderer, final_sky);

        // Visualiser colours, lerped through an active palette transition.
        let (top_col, bottom_col) = if has_audio {
            let (top_rgb, bottom_rgb) = if renderer.state.transition_progress < 1.0 {
                let t = renderer.state.transition_progress;
                (
                    lerp_colour(renderer.vis_prev_colors.0, renderer.vis_target_colors.0, t),
                    lerp_colour(renderer.vis_prev_colors.1, renderer.vis_target_colors.1, t),
                )
            } else {
                (renderer.vis_target_colors.0, renderer.vis_target_colors.1)
            };
            (
                [top_rgb[0], top_rgb[1], top_rgb[2], 1.0],
                [bottom_rgb[0], bottom_rgb[1], bottom_rgb[2], 1.0],
            )
        } else {
            ([0.0; 4], [0.0; 4])
        };

        let art_tint_color = if show_art_fg || show_art_bg || show_color_bg {
            if renderer.state.transition_progress < 1.0 {
                lerp_colour(
                    renderer.art.prev_color,
                    renderer.art.target_color,
                    renderer.state.transition_progress,
                )
            } else {
                renderer.art.target_color
            }
        } else {
            [0.1, 0.1, 0.1]
        };

        let elapsed = renderer.start_time.elapsed().as_secs_f32();

        // Ambient sky uniforms are only needed when no custom background
        // texture will cover them.
        let sky_color_data = if renderer.background.bind_group().is_none() {
            Some((elapsed, weather_type, final_sky))
        } else {
            None
        };

        let vis_shape_u32 = match renderer.theme.visualiser.shape {
            VisShape::Circular => 0,
            VisShape::Linear => 1,
            VisShape::Square => 2,
        };
        let vis_align_u32 = match renderer.theme.visualiser.align {
            VisAlign::Left => 0,
            VisAlign::Center => 1,
            VisAlign::Right => 2,
        };
        let vis_pos_size_rot = [
            renderer.theme.visualiser.position[0],
            renderer.theme.visualiser.position[1],
            renderer.theme.visualiser.size,
            renderer.theme.visualiser.rotation.to_radians(),
        ];
        let is_waveform_u32 = if renderer.is_waveform_style { 1 } else { 0 };

        let album_art_bg_mode = if show_color_bg {
            3
        } else if appearance.disable_blur {
            2
        } else {
            0
        };
        let album_art_bg_alpha = (1.0 - renderer.state.transparent_fade) * renderer.art.fade;

        let mut album_art_fg_pos = renderer.theme.album_art.position;
        let mut album_art_fg_size = renderer.theme.album_art.size;
        let mut album_art_fg_shape = if renderer.theme.album_art.shape == ArtShape::Circular {
            1
        } else {
            0
        };
        if dock_art_active(has_audio, &renderer.theme.visualiser) {
            album_art_fg_pos = renderer.theme.visualiser.position;
            album_art_fg_size = renderer.theme.visualiser.size;
            album_art_fg_shape = 1;
        }

        let custom_bg_mode = if appearance.disable_blur { 2 } else { 0 };
        let custom_bg_alpha = 1.0 - renderer.state.transparent_fade;

        let visualiser_instance_count = if renderer.is_waveform_style {
            1
        } else if renderer.theme.visualiser.shape == VisShape::Linear {
            renderer.state.config.audio.bands as u32
        } else {
            renderer.state.config.audio.bands as u32 * 2
        };

        let (lyric_start_idx, lyric_end_idx) = lyric_window_bounds(
            renderer.current_lyric_idx,
            renderer
                .state
                .current_track
                .as_ref()
                .and_then(|t| t.lyrics.as_ref())
                .map(|l| l.len()),
        );

        // Screen-invariant foreground album art transform components.
        let album_art_aspect = renderer.art.aspect();
        let fg_art_base_uv = get_uv_transform(1, 1.0, album_art_aspect);
        // Theme sizes come from hand-editable TOML with no clamp, so
        // size = 0.0 is reachable; dividing by it would NaN-poison the
        // whole fg transform.
        let inv_album_art_fg_size = 1.0 / album_art_fg_size.max(1e-3);
        let fg_k1 = inv_album_art_fg_size * fg_art_base_uv[0];
        let fg_k2 = 0.5 * fg_art_base_uv[0] + fg_art_base_uv[2];
        let fg_k3 = album_art_fg_pos[0] * fg_k1;
        let fg_scale_y = inv_album_art_fg_size * fg_art_base_uv[1];
        let fg_offset_y = (0.5 - album_art_fg_pos[1] * inv_album_art_fg_size) * fg_art_base_uv[1]
            + fg_art_base_uv[3];

        let map_align = |a: &TextAlign| -> cosmic_text::Align {
            match a {
                TextAlign::Left => cosmic_text::Align::Left,
                TextAlign::Center => cosmic_text::Align::Center,
                TextAlign::Right => cosmic_text::Align::Right,
            }
        };

        FrameParams {
            has_audio,
            audio_energy,
            show_art_fg,
            show_art_bg,
            show_color_bg,
            clear_colour,
            is_weather_active: renderer.is_weather_active,
            active_particles: renderer.active_particles,
            top_col,
            bottom_col,
            art_tint_color,
            elapsed,
            track_hash: renderer.cached_track_hash,
            weather_hash: renderer.cached_weather_hash,
            sky_color_data,
            vis_shape_u32,
            vis_align_u32,
            vis_pos_size_rot,
            is_waveform_u32,
            album_art_bg_mode,
            album_art_bg_alpha,
            album_art_aspect,
            album_art_fg_pos,
            album_art_fg_size,
            album_art_fg_shape,
            custom_bg_mode,
            custom_bg_alpha,
            custom_bg_aspect: renderer.background.aspect(),
            blur_opacity: appearance.blur_opacity,
            visualiser_instance_count,
            lyric_start_idx,
            lyric_end_idx,
            fg_k1,
            fg_k2,
            fg_k3,
            fg_scale_y,
            fg_offset_y,
            secondary_text: renderer.secondary_text_color,
            text_color_diff: renderer.text_color_diff,
            lyrics_align: map_align(&renderer.theme.lyrics.align),
            track_info_align: map_align(&renderer.theme.track_info.align),
            weather_align: map_align(&renderer.theme.weather.align),
            font_family: appearance
                .font_family
                .clone()
                .or_else(|| renderer.theme.font_family.clone()),
            lyric_bounce: renderer.lyric_bounce_value,
            beat_pulse_mul: renderer.audio.beat_pulse * 2.0,
        }
    }
}

fn get_final_sky_color(renderer: &super::Renderer) -> [f32; 3] {
    let sky = time_to_sky_colour(renderer.state.time_of_day);
    if let Some(weather) = &renderer.state.weather {
        if renderer.state.config.weather.enabled {
            match weather.condition {
                WeatherCondition::Rain | WeatherCondition::Thunderstorm => {
                    lerp_colour(sky, [0.2, 0.2, 0.25], 0.6)
                }
                WeatherCondition::Snow => lerp_colour(sky, [0.8, 0.85, 0.9], 0.4),
                _ => sky,
            }
        } else {
            sky
        }
    } else {
        sky
    }
}

fn get_clear_colour_from_sky(renderer: &super::Renderer, final_sky: [f32; 3]) -> wgpu::Color {
    if renderer.state.config.appearance.transparent_background {
        return wgpu::Color::TRANSPARENT;
    }

    let scene = match renderer.state.config.mode {
        WallpaperMode::Weather => SceneHint::Ambient,
        WallpaperMode::AlbumArt => SceneHint::AlbumArt,
        WallpaperMode::AudioVisualiser => SceneHint::AudioVisualiser,
        WallpaperMode::Auto => renderer.state.scene_description(),
    };

    match scene {
        SceneHint::Ambient => wgpu::Color {
            r: final_sky[0] as f64,
            g: final_sky[1] as f64,
            b: final_sky[2] as f64,
            a: 1.0,
        },
        SceneHint::AlbumArt => wgpu::Color {
            r: 0.05,
            g: 0.05,
            b: 0.05,
            a: 1.0,
        },
        SceneHint::AudioVisualiser => wgpu::Color {
            r: 0.1,
            g: 0.1,
            b: 0.15,
            a: 1.0,
        },
    }
}

/// UV transform for cover (mode 0/2) or contain (mode 1) fitting of an
/// image of `image_aspect` into a target of `screen_aspect`.
pub(crate) fn get_uv_transform(mode: u32, screen_aspect: f32, image_aspect: f32) -> [f32; 4] {
    let new_aspect = screen_aspect / image_aspect;

    let mut scale_x = 1.0;
    let mut scale_y = 1.0;
    let mut offset_x = 0.0;
    let mut offset_y = 0.0;

    if mode == 0 || mode == 2 {
        // object-fit: cover
        if new_aspect > 1.0 {
            scale_x = 1.0 / new_aspect;
            offset_x = (1.0 - scale_x) / 2.0;
        } else {
            scale_y = new_aspect;
            offset_y = (1.0 - scale_y) / 2.0;
        }
    } else if mode == 1 {
        // object-fit: contain
        if new_aspect > 1.0 {
            scale_x = new_aspect;
            offset_x = (1.0 - scale_x) / 2.0;
        } else {
            scale_y = 1.0 / new_aspect;
            offset_y = (1.0 - scale_y) / 2.0;
        }
    }

    [scale_x, scale_y, offset_x, offset_y]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::config::VisualiserLayout;

    fn vis(shape: VisShape, dock_art: bool) -> VisualiserLayout {
        VisualiserLayout {
            shape,
            position: [0.5, 0.5],
            size: 0.25,
            rotation: 0.0,
            amplitude: 1.0,
            align: VisAlign::Left,
            color_top: None,
            color_bottom: None,
            shader: None,
            dock_art,
        }
    }

    #[test]
    fn has_audio_gating() {
        use WallpaperMode as M;
        // Energy + a track: on.
        assert!(has_audio_active(0.5, &M::Auto, true));
        // Energy but no track to attribute it to: off.
        assert!(!has_audio_active(0.5, &M::Auto, false));
        // Below the visibility threshold: off.
        assert!(!has_audio_active(0.0005, &M::Auto, true));
        // Forced visualiser needs neither energy nor a track.
        assert!(has_audio_active(0.0, &M::AudioVisualiser, false));
        // Forced weather/art modes always suppress it.
        assert!(!has_audio_active(0.5, &M::Weather, true));
        assert!(!has_audio_active(0.5, &M::AlbumArt, true));
    }

    #[test]
    fn dock_art_requires_circular_shape_audio_and_opt_in() {
        assert!(dock_art_active(true, &vis(VisShape::Circular, true)));
        // Theme opted out: the art keeps its own layout.
        assert!(!dock_art_active(true, &vis(VisShape::Circular, false)));
        // Docking is a circular-ring concept only.
        assert!(!dock_art_active(true, &vis(VisShape::Linear, true)));
        // Music paused: the art returns to its own layout.
        assert!(!dock_art_active(false, &vis(VisShape::Circular, true)));
    }

    #[test]
    fn lyric_window_bounds_clamp_to_the_track() {
        // No lyrics: the canonical empty range.
        assert_eq!(lyric_window_bounds(0, None), (1, 0));
        // Start of the track: window can't reach line 0 (indices are 1-based).
        assert_eq!(lyric_window_bounds(0, Some(10)), (1, 2));
        // Mid-track: symmetric ±2 window.
        assert_eq!(lyric_window_bounds(5, Some(10)), (3, 7));
        // End of the track: clamped to the last line.
        assert_eq!(lyric_window_bounds(10, Some(10)), (8, 10));
        // Empty lyric list still yields an empty range, not a panic.
        assert_eq!(lyric_window_bounds(0, Some(0)), (1, 0));
    }

    #[test]
    fn uv_transform_cover_and_contain() {
        // Square image on a square screen: identity either way.
        assert_eq!(get_uv_transform(0, 1.0, 1.0), [1.0, 1.0, 0.0, 0.0]);
        assert_eq!(get_uv_transform(1, 1.0, 1.0), [1.0, 1.0, 0.0, 0.0]);
        // Wide screen, square image, cover: crop vertically (scale_y < 1).
        let cover = get_uv_transform(0, 2.0, 1.0);
        assert!(cover[0] < 1.0 && cover[1] == 1.0);
        // Same but contain: letterbox horizontally (scale_x > 1).
        let contain = get_uv_transform(1, 2.0, 1.0);
        assert!(contain[0] > 1.0 && contain[1] == 1.0);
    }
}
