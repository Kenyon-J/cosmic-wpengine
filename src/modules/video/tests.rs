#![cfg(test)]

use super::*;
use image::RgbaImage;
use tokio::sync::mpsc;

/// Tests that a `PooledImage` correctly releases its internal `RgbaImage` into its raw vector representation when dropped/converted.
/// This prevents memory leaks and ensures our frame pooling mechanism reuses buffers instead of thrashing the allocator.
#[test]
fn test_pooled_image_into_raw() {
    let (tx, _rx) = mpsc::channel(1);

    // Create a 2x2 red image
    let mut img = RgbaImage::new(2, 2);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba([255, 0, 0, 255]);
    }
    let expected_raw = img.clone().into_raw();

    let pooled_image = PooledImage::new(img, tx);
    let raw = pooled_image.into_raw();

    assert_eq!(raw, expected_raw);
}

#[test]
fn scaled_video_size_downscales_to_cover_the_display() {
    // 4K video on a 1080p monitor: exactly half in both directions.
    assert_eq!(
        scaled_video_size((3840, 2160), Some((1920, 1080))),
        (1920, 1080)
    );
    // Aspect ratio mismatch: scaled so the *larger* ratio still covers the
    // target (here height), keeping the video's own aspect.
    assert_eq!(
        scaled_video_size((3840, 1600), Some((1920, 1080))),
        (2592, 1080)
    );
    // Portrait + landscape monitors combined (maxed independently).
    assert_eq!(
        scaled_video_size((3840, 2160), Some((1920, 1920))),
        (3414, 1920)
    );
}

#[test]
fn scaled_video_size_never_upscales_or_guesses() {
    // Smaller than the display: left alone.
    assert_eq!(
        scaled_video_size((1280, 720), Some((1920, 1080))),
        (1280, 720)
    );
    // Display size unknown or degenerate: source size.
    assert_eq!(scaled_video_size((3840, 2160), None), (3840, 2160));
    assert_eq!(
        scaled_video_size((3840, 2160), Some((0, 1080))),
        (3840, 2160)
    );
    // Degenerate source doesn't divide by zero.
    assert_eq!(scaled_video_size((0, 0), Some((1920, 1080))), (0, 0));
}

/// What a test clip's frames contain.
#[derive(Clone, Copy)]
enum ClipStyle {
    /// A moving gradient, so consecutive frames differ.
    Gradient,
    /// One flat limited-range YUV colour, tagged with `space`.
    Flat {
        yuv: [u8; 3],
        space: ffmpeg::color::Space,
    },
}

/// Encodes a small test clip (mpeg4 in Matroska - both built into every
/// ffmpeg configuration, including the static-ffmpeg one) of `frames`
/// frames at `fps`, so the local decoder can be exercised without an
/// ffmpeg binary or a checked-in media file.
fn encode_test_clip(
    path: &std::path::Path,
    width: u32,
    height: u32,
    fps: i32,
    frames: i64,
    style: ClipStyle,
) {
    ffmpeg::init().unwrap();
    let mut octx = ffmpeg::format::output(&path).unwrap();
    let global_header = octx
        .format()
        .flags()
        .contains(ffmpeg::format::Flags::GLOBAL_HEADER);

    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::MPEG4).expect("mpeg4 encoder");
    let mut stream = octx.add_stream(codec).unwrap();
    let mut encoder = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()
        .unwrap();
    encoder.set_width(width);
    encoder.set_height(height);
    encoder.set_format(ffmpeg::format::Pixel::YUV420P);
    encoder.set_time_base(ffmpeg::Rational(1, fps));
    encoder.set_frame_rate(Some(ffmpeg::Rational(fps, 1)));
    if global_header {
        encoder.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
    }
    if let ClipStyle::Flat { space, .. } = style {
        encoder.set_colorspace(space);
        encoder.set_color_range(ffmpeg::color::Range::MPEG);
    }
    let mut encoder = encoder.open_as(codec).unwrap();
    stream.set_parameters(&encoder);
    stream.set_time_base(ffmpeg::Rational(1, fps));
    let stream_index = stream.index();
    octx.write_header().unwrap();
    let stream_time_base = octx.stream(stream_index).unwrap().time_base();

    let write_packets = |encoder: &mut ffmpeg::encoder::Video,
                         octx: &mut ffmpeg::format::context::Output| {
        let mut packet = ffmpeg::Packet::empty();
        while encoder.receive_packet(&mut packet).is_ok() {
            packet.set_stream(stream_index);
            packet.rescale_ts(ffmpeg::Rational(1, fps), stream_time_base);
            packet.write_interleaved(octx).unwrap();
        }
    };

    let mut frame = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::YUV420P, width, height);
    for i in 0..frames {
        for plane in 0..3 {
            let stride = frame.stride(plane);
            let data = frame.data_mut(plane);
            for (y, row) in data.chunks_mut(stride).enumerate() {
                for (x, px) in row.iter_mut().enumerate() {
                    *px = match style {
                        ClipStyle::Gradient => ((x + y + i as usize * 4) % 256) as u8,
                        ClipStyle::Flat { yuv, .. } => yuv[plane],
                    };
                }
            }
        }
        frame.set_pts(Some(i));
        encoder.send_frame(&frame).unwrap();
        write_packets(&mut encoder, &mut octx);
    }
    encoder.send_eof().unwrap();
    write_packets(&mut encoder, &mut octx);
    octx.write_trailer().unwrap();
}

/// Waits up to `timeout` for the next background video frame's size.
async fn next_frame_size(
    rx: &mut mpsc::Receiver<Event>,
    timeout: std::time::Duration,
) -> Option<(u32, u32)> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Some(Event::BackgroundVideoFrame(frame))) => return Some(frame.dimensions()),
            Ok(Some(_)) => continue,
            Ok(None) | Err(_) => return None,
        }
    }
}

/// Counts background video frames arriving within `window`.
async fn frames_within(rx: &mut mpsc::Receiver<Event>, window: std::time::Duration) -> usize {
    let deadline = tokio::time::Instant::now() + window;
    let mut count = 0;
    while let Ok(Some(event)) = tokio::time::timeout_at(deadline, rx.recv()).await {
        if matches!(event, Event::BackgroundVideoFrame(_)) {
            count += 1;
        }
    }
    count
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_decoder_scales_to_display_and_pauses_while_hidden() {
    use std::time::Duration;

    let dir = tempfile::tempdir().unwrap();
    let clip = dir.path().join("clip.mkv");
    // 4 seconds at 30fps: long enough that the loop-back seek never lands
    // inside the measured windows below.
    encode_test_clip(&clip, 320, 240, 30, 120, ClipStyle::Gradient);

    let (tx, mut rx) = mpsc::channel(64);
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    let (_config_tx, config_rx) =
        tokio::sync::watch::channel(crate::modules::config::Config::default());
    let (visible_tx, visible_rx) = tokio::sync::watch::channel(true);
    let (size_tx, size_rx) = tokio::sync::watch::channel(Some((160, 90)));
    let (recycle_tx, recycle_rx) = mpsc::channel(3);

    VideoDecoder::run_local_decoder(
        clip.to_string_lossy().to_string(),
        tx,
        cancel_rx,
        config_rx,
        visible_rx,
        size_rx,
        recycle_rx,
        recycle_tx,
    )
    .await
    .unwrap();

    // Converted at display size: 320x240 scaled by max(160/320, 90/240) = 0.5.
    assert_eq!(
        next_frame_size(&mut rx, Duration::from_secs(5)).await,
        Some((160, 120))
    );

    // Hidden: after at most one in-flight frame, nothing more is decoded.
    visible_tx.send(false).unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    while rx.try_recv().is_ok() {}
    assert_eq!(frames_within(&mut rx, Duration::from_millis(600)).await, 0);

    // Visible again: frames resume promptly (the pacing clock skipped the
    // pause instead of racing to catch up or stalling).
    visible_tx.send(true).unwrap();
    assert!(next_frame_size(&mut rx, Duration::from_secs(2))
        .await
        .is_some());

    // A new display size rebuilds the scaler on the fly.
    size_tx.send(Some((640, 480))).unwrap();
    let mut resized = false;
    for _ in 0..30 {
        if next_frame_size(&mut rx, Duration::from_secs(2)).await == Some((320, 240)) {
            resized = true;
            break;
        }
    }
    assert!(resized, "decoder never switched to the new output size");

    // Cancel: the decoder thread exits and drops its sender.
    cancel_tx.send(true).unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        while rx.recv().await.is_some() {}
    })
    .await;
    assert!(closed.is_ok(), "decoder did not exit after cancel");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_decoder_cancel_while_hidden_exits() {
    use std::time::Duration;

    let dir = tempfile::tempdir().unwrap();
    let clip = dir.path().join("clip.mkv");
    encode_test_clip(&clip, 64, 48, 30, 30, ClipStyle::Gradient);

    let (tx, mut rx) = mpsc::channel(64);
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    let (_config_tx, config_rx) =
        tokio::sync::watch::channel(crate::modules::config::Config::default());
    // Starts hidden: the decoder parks before its first packet.
    let (_visible_tx, visible_rx) = tokio::sync::watch::channel(false);
    let (_size_tx, size_rx) = tokio::sync::watch::channel(None);
    let (recycle_tx, recycle_rx) = mpsc::channel(3);

    VideoDecoder::run_local_decoder(
        clip.to_string_lossy().to_string(),
        tx,
        cancel_rx,
        config_rx,
        visible_rx,
        size_rx,
        recycle_rx,
        recycle_tx,
    )
    .await
    .unwrap();

    assert_eq!(frames_within(&mut rx, Duration::from_millis(500)).await, 0);

    cancel_tx.send(true).unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        while rx.recv().await.is_some() {}
    })
    .await;
    assert!(closed.is_ok(), "parked decoder did not exit after cancel");
}

#[test]
fn sws_colorspace_uses_the_tag_then_guesses_by_size() {
    use ffmpeg::color::Space;
    use ffmpeg::ffi::{SWS_CS_BT2020, SWS_CS_ITU601, SWS_CS_ITU709};
    assert_eq!(sws_colorspace(Space::BT709, 480), SWS_CS_ITU709);
    assert_eq!(sws_colorspace(Space::SMPTE170M, 2160), SWS_CS_ITU601);
    assert_eq!(sws_colorspace(Space::BT2020NCL, 2160), SWS_CS_BT2020);
    // Untagged: HD is almost always BT.709, SD BT.601.
    assert_eq!(sws_colorspace(Space::Unspecified, 1080), SWS_CS_ITU709);
    assert_eq!(sws_colorspace(Space::Unspecified, 720), SWS_CS_ITU709);
    assert_eq!(sws_colorspace(Space::Unspecified, 576), SWS_CS_ITU601);
}

/// Decodes the first frame of `path` through the local decoder's own
/// stream/scaler setup (software decoding) and returns its centre pixel.
fn first_frame_centre_pixel(path: &std::path::Path) -> [u8; 3] {
    let mut stream = open_video_stream(&path.to_string_lossy(), None).unwrap();
    let mut decoded = ffmpeg::frame::Video::empty();
    let mut got_frame = false;
    for (packet_stream, packet) in stream.ictx.packets() {
        if packet_stream.index() != stream.stream_index {
            continue;
        }
        stream.decoder.send_packet(&packet).unwrap();
        if stream.decoder.receive_frame(&mut decoded).is_ok() {
            got_frame = true;
            break;
        }
    }
    if !got_frame {
        stream.decoder.send_eof().unwrap();
        stream.decoder.receive_frame(&mut decoded).unwrap();
    }

    let mut slot = None;
    let (scaler, _) = FrameScaler::for_frame(&mut slot, &decoded, None).unwrap();
    let mut rgb = ffmpeg::frame::Video::empty();
    scaler.ctx.run(&decoded, &mut rgb).unwrap();
    let (x, y) = (scaler.width as usize / 2, scaler.height as usize / 2);
    let offset = y * rgb.stride(0) + x * 4;
    let px = &rgb.data(0)[offset..offset + 3];
    [px[0], px[1], px[2]]
}

fn assert_close(actual: [u8; 3], expected: [u8; 3], what: &str) {
    let off = actual
        .iter()
        .zip(expected)
        .map(|(&a, e)| (a as i32 - e as i32).abs())
        .max()
        .unwrap();
    assert!(
        off <= 6,
        "{what}: got {actual:?}, expected about {expected:?}"
    );
}

#[test]
fn local_decoder_honours_the_stream_colour_matrix() {
    use ffmpeg::color::Space;
    // Limited-range BT.709 pure green: Y = 16 + 219*0.7152, and
    // Cb/Cr = 128 - 224*0.7152/{1.8556, 1.5748}.
    let yuv = [173, 42, 26];
    let dir = tempfile::tempdir().unwrap();

    let bt709 = dir.path().join("bt709.mkv");
    encode_test_clip(
        &bt709,
        64,
        48,
        30,
        3,
        ClipStyle::Flat {
            yuv,
            space: Space::BT709,
        },
    );
    assert_close(first_frame_centre_pixel(&bt709), [0, 255, 0], "BT.709 clip");

    // The same bytes tagged BT.601 must decode differently - proving the
    // tag is read, not that BT.709 is simply hard-coded.
    let bt601 = dir.path().join("bt601.mkv");
    encode_test_clip(
        &bt601,
        64,
        48,
        30,
        3,
        ClipStyle::Flat {
            yuv,
            space: Space::BT470BG,
        },
    );
    assert_close(
        first_frame_centre_pixel(&bt601),
        [20, 255, 9],
        "BT.601 clip",
    );
}
