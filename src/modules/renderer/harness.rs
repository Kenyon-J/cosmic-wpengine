//! Dev-only offscreen render harness - phase 4 of the renderer
//! decomposition (`docs/PLAN-renderer-decomposition.md`). Builds a
//! `Renderer` with no Wayland surfaces at all, feeds it a fixed synthetic
//! scene (frosted glass on, weather off, a known track with synthetic
//! album art, a fixed audio spectrum), renders exactly one frame to an
//! offscreen texture via [`super::draw::encode_frame`], and writes it to a
//! PNG. Deterministic across runs of the same code, so a refactor's
//! before/after can be diffed without a live desktop session - this is what
//! makes the acceptance harness for phases 4-5 (and beyond) not depend on
//! a human eyeballing `cosmic-screenshot` output.
//!
//! Invoked via the engine binary's hidden `--render-frame <out.png>
//! [--compare <baseline.png>] [--style <name>]` flag; never reachable from
//! the normal startup path.

use super::core::Renderer;
use super::draw;
use super::frame_params::FrameParams;
use crate::modules::config::Config;
use crate::modules::event::{Event, PooledAudioBuffer, TrackInfo};
use crate::modules::state::AppState;
use anyhow::{Context, Result};
use std::path::Path;

/// Fixed offscreen render-target resolution. Arbitrary, but must stay
/// constant across any two runs being diffed against each other.
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

/// Renders one frame against a fixed synthetic scene and writes it to
/// `out_path`. When `compare_path` is given, also diffs the render against
/// that baseline PNG and reports pass/fail (mean-absolute-difference per
/// channel under 1/255, matching the frosted-glass live-verification
/// threshold this harness replaces). `style` overrides which theme
/// (`audio.style`) the scene is rendered with - e.g. to exercise a theme
/// with a custom visualiser shader set, without needing a real saved theme.
pub async fn render_frame_to_png(
    out_path: &Path,
    compare_path: Option<&Path>,
    style: Option<&str>,
) -> Result<()> {
    let mut config = Config::default();
    // Config::default() already matches the acceptance recipe (fps 30,
    // weather off, blur on); audio.style is left at its default so the
    // harness exercises a real theme, not a hand-rolled stand-in, unless
    // the caller asked for a specific one.
    config.weather.enabled = false;
    if let Some(style) = style {
        config.audio.style = style.to_string();
    }

    let state = AppState::new(config);
    let (show_lyrics_tx, _show_lyrics_rx) = tokio::sync::watch::channel(true);

    let mut renderer = Renderer::new_headless(state, show_lyrics_tx)
        .await
        .context("building the headless renderer")?;

    feed_synthetic_scene(&mut renderer).await;

    let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Offscreen Render Target"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: renderer.surface_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let params = FrameParams::compute(&renderer);

    // The live per-monitor loop writes these once per resolution
    // (last_uniform_res dedup) before encoding; a one-shot offscreen
    // render has no cache to reuse, so it always needs this call.
    draw::write_frame_uniforms(
        &renderer.queue,
        &renderer.visualiser_pass.uniform_buffer,
        &renderer.art,
        &renderer.background,
        WIDTH,
        HEIGHT,
        params.has_audio,
        renderer.state.current_track.is_some(),
        renderer.state.transition_progress,
        renderer.state.config.audio.bands as u32,
        params.beat_pulse_mul,
        params.top_col,
        params.bottom_col,
        params.vis_pos_size_rot,
        renderer.theme.visualiser.amplitude,
        params.vis_shape_u32,
        params.elapsed,
        params.vis_align_u32,
        params.is_waveform_u32,
        params.show_art_fg,
        params.show_art_bg,
        params.show_color_bg,
        params.art_tint_color,
        params.album_art_aspect,
        params.album_art_bg_mode,
        params.audio_energy,
        params.album_art_bg_alpha,
        params.blur_opacity,
        params.fg_k1,
        params.fg_k2,
        params.fg_k3,
        params.fg_scale_y,
        params.fg_offset_y,
        params.album_art_fg_pos,
        params.album_art_fg_size,
        params.album_art_fg_shape,
        params.custom_bg_aspect,
        params.custom_bg_mode,
        params.custom_bg_alpha,
        params.sky_color_data,
    );

    draw::encode_frame(
        &renderer.device,
        &renderer.queue,
        &view,
        WIDTH,
        HEIGHT,
        &renderer.album_art_pipeline,
        &renderer.art,
        &renderer.background,
        &renderer.weather_render_pipeline,
        &renderer.weather_render_bind_group,
        &renderer.visualiser_pass,
        &renderer.text.text_renderer,
        params.clear_colour,
        params.show_art_bg,
        params.show_color_bg,
        params.show_art_fg,
        params.is_weather_active,
        params.active_particles,
        params.has_audio,
        params.visualiser_instance_count,
    );

    let rendered =
        read_texture_to_image(&renderer.device, &renderer.queue, &texture, WIDTH, HEIGHT)?;
    rendered
        .save(out_path)
        .with_context(|| format!("writing {out_path:?}"))?;
    tracing::info!("Wrote offscreen render to {:?}", out_path);

    if let Some(baseline_path) = compare_path {
        let baseline = image::open(baseline_path)
            .with_context(|| format!("opening baseline {baseline_path:?}"))?
            .to_rgba8();
        perceptual_diff(&rendered, &baseline)?;
    }

    Ok(())
}

/// Feeds a fixed, deterministic scene through the same event path real
/// events use (`Renderer::handle_event`), rather than poking renderer
/// fields directly: this exercises the real MPRIS/audio-frame handling
/// code, not a hand-rolled shortcut that could drift from it.
async fn feed_synthetic_scene(renderer: &mut Renderer) {
    // 512-bin raw spectrum (matches AudioCapture's real FFT resolution at
    // 48kHz/2048 - see audio_analysis.rs's own test fixture for the same
    // shape): a ramp, so both the bass/treble detection bins and the
    // visualiser's per-band energy see plausible, nonzero values.
    let raw_len = 512;
    let raw_bands: Vec<f32> = (0..raw_len)
        .map(|i| 0.1 + 0.8 * (i as f32 / raw_len as f32))
        .collect();
    let raw_waveform: Vec<f32> = (0..raw_len)
        .map(|i| ((i as f32 / raw_len as f32) * std::f32::consts::TAU).sin() * 0.5)
        .collect();

    let (recycle_bands_tx, _recycle_bands_rx) = tokio::sync::mpsc::channel(1);
    let (recycle_waveform_tx, _recycle_waveform_rx) = tokio::sync::mpsc::channel(1);
    renderer
        .handle_event(Event::AudioFrame {
            bands: PooledAudioBuffer::new(raw_bands.into_boxed_slice(), recycle_bands_tx),
            waveform: PooledAudioBuffer::new(raw_waveform.into_boxed_slice(), recycle_waveform_tx),
        })
        .await;

    let track = TrackInfo {
        title: "Harness Track".into(),
        artist: "Harness Artist".into(),
        album: "Harness Album".into(),
        track_key: "harness-track".into(),
        album_art: Some(synthetic_album_art()),
        palette: None,
        lyrics: None,
        video_url: None,
    };
    renderer
        .handle_event(Event::TrackChanged(Box::new(track)))
        .await;
    renderer.handle_event(Event::PlaybackResumed).await;
}

/// A small synthetic checkerboard - deterministic, no network/filesystem
/// dependency, unlike real album art.
fn synthetic_album_art() -> image::RgbaImage {
    const SIZE: u32 = 64;
    image::RgbaImage::from_fn(SIZE, SIZE, |x, y| {
        if (x / 8 + y / 8) % 2 == 0 {
            image::Rgba([220, 90, 60, 255])
        } else {
            image::Rgba([40, 60, 200, 255])
        }
    })
}

/// Copies `texture` back to the CPU as an `RgbaImage`. wgpu requires each
/// copied row to be padded to a 256-byte stride; this strips that padding
/// back out, mirroring `upload_rgba_to_texture`'s padding math in reverse.
fn read_texture_to_image(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<image::RgbaImage> {
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let unpadded_bytes_per_row = width * 4;
    let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) & !(align - 1);
    let buffer_size = (padded_bytes_per_row * height) as wgpu::BufferAddress;

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Offscreen Readback Buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Offscreen Readback Encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::PollType::wait_indefinitely())?;
    rx.recv()
        .context("readback map_async callback never fired")?
        .context("mapping the readback buffer failed")?;

    let padded: Vec<u8> = slice
        .get_mapped_range()
        .context("reading back the mapped buffer")?
        .to_vec();
    staging.unmap();

    let mut pixels = Vec::with_capacity((unpadded_bytes_per_row * height) as usize);
    for row in padded.chunks_exact(padded_bytes_per_row as usize) {
        pixels.extend_from_slice(&row[..unpadded_bytes_per_row as usize]);
    }

    image::RgbaImage::from_raw(width, height, pixels)
        .context("readback buffer size didn't match the expected image dimensions")
}

/// Reports (via `println!` - this is a dev CLI tool, not the app's own
/// logging) whether `rendered` matches `baseline` within a small
/// mean-absolute-difference tolerance per channel, and returns an error if
/// it doesn't (so the harness's own exit code reflects pass/fail). Text
/// antialiasing and the animated visualiser mean an exact pixel match isn't
/// realistic even between two runs of identical code - this mirrors the
/// tolerance already proven out during the Kawase port's live
/// verification.
fn perceptual_diff(rendered: &image::RgbaImage, baseline: &image::RgbaImage) -> Result<()> {
    anyhow::ensure!(
        rendered.dimensions() == baseline.dimensions(),
        "dimension mismatch: rendered {:?}, baseline {:?}",
        rendered.dimensions(),
        baseline.dimensions()
    );

    let mut total_diff: f64 = 0.0;
    let mut max_diff: u8 = 0;
    for (r, b) in rendered.pixels().zip(baseline.pixels()) {
        for c in 0..4 {
            let d = r.0[c].abs_diff(b.0[c]);
            total_diff += d as f64;
            max_diff = max_diff.max(d);
        }
    }
    let n = (rendered.width() * rendered.height() * 4) as f64;
    let mean_diff = total_diff / n;

    println!(
        "Perceptual diff: mean {:.4}/255 per channel, max {}/255",
        mean_diff, max_diff
    );

    anyhow::ensure!(
        mean_diff < 1.0,
        "render diverged from baseline: mean {mean_diff:.4}/255 per channel (threshold 1.0/255)"
    );

    println!("PASS: render matches baseline within tolerance.");
    Ok(())
}
