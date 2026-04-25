use anyhow::Result;
use ffmpeg_next as ffmpeg;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use super::event::Event;

pub struct PooledImage {
    img: Option<image::RgbaImage>,
    recycle_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
}

impl PooledImage {
    pub fn new(img: image::RgbaImage, recycle_tx: tokio::sync::mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            img: Some(img),
            recycle_tx,
        }
    }

    // Keeps backwards compatibility if the renderer manually consumes the raw buffer
    pub fn into_raw(mut self) -> Vec<u8> {
        self.img.take().map(|i| i.into_raw()).unwrap_or_default()
    }
}

impl std::ops::Deref for PooledImage {
    type Target = image::RgbaImage;
    fn deref(&self) -> &Self::Target {
        // Using a static default image guarantees no panic if dereferenced after drop
        static DEFAULT_IMG: std::sync::LazyLock<image::RgbaImage> =
            std::sync::LazyLock::new(|| image::RgbaImage::new(1, 1));
        self.img.as_ref().unwrap_or(&DEFAULT_IMG)
    }
}

impl Drop for PooledImage {
    fn drop(&mut self) {
        if let Some(img) = self.img.take() {
            let _ = self.recycle_tx.try_send(img.into_raw());
        }
    }
}

impl std::fmt::Debug for PooledImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PooledImage").finish()
    }
}

pub struct VideoDecoder;

impl VideoDecoder {
    pub async fn run_local_decoder(
        path: String,
        tx: Sender<Event>,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
        config_rx: tokio::sync::watch::Receiver<super::config::Config>,
        mut recycle_rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
        recycle_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    ) -> Result<()> {
        let _ = ffmpeg::init();
        info!("Starting local ffmpeg-next video decoder for: {}", path);

        tokio::task::spawn_blocking(move || {
            let mut ictx = match ffmpeg::format::input(&path) {
                Ok(ctx) => ctx,
                Err(e) => {
                    warn!("ffmpeg-next failed to open input {}: {}", path, e);
                    return;
                }
            };

            let input = ictx
                .streams()
                .best(ffmpeg::media::Type::Video)
                .ok_or(ffmpeg::Error::StreamNotFound);

            let input = match input {
                Ok(stream) => stream,
                Err(e) => {
                    warn!("ffmpeg-next failed to find video stream: {}", e);
                    return;
                }
            };

            let stream_index = input.index();

            let context_decoder =
                match ffmpeg::codec::context::Context::from_parameters(input.parameters()) {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        warn!("ffmpeg-next failed to get codec context: {}", e);
                        return;
                    }
                };

            let mut decoder = match context_decoder.decoder().video() {
                Ok(dec) => dec,
                Err(e) => {
                    warn!("ffmpeg-next failed to get video decoder: {}", e);
                    return;
                }
            };

            let width = decoder.width();
            let height = decoder.height();
            let frame_size = (width * height * 4) as usize;

            let mut scaler = match ffmpeg::software::scaling::Context::get(
                decoder.format(),
                width,
                height,
                ffmpeg::format::Pixel::RGBA,
                width,
                height,
                ffmpeg::software::scaling::flag::Flags::BILINEAR,
            ) {
                Ok(s) => s,
                Err(e) => {
                    warn!("ffmpeg-next failed to create scaler: {}", e);
                    return;
                }
            };

            let time_base = input.time_base();
            let time_base_f64 = time_base.numerator() as f64 / time_base.denominator() as f64;

            // Loop infinitely
            'outer: loop {
                if *cancel_rx.borrow() {
                    break;
                }

                // We need to seek to the beginning if we loop.
                if let Err(e) = ictx.seek(0, 0..ictx.duration().max(0)) {
                    warn!("ffmpeg-next seek failed: {}", e);
                    // Just reopen if seek fails
                    ictx = match ffmpeg::format::input(&path) {
                        Ok(ctx) => ctx,
                        Err(_) => break,
                    };
                }
                decoder.flush();
                let mut first_pts: Option<i64> = None;
                let start_time = tokio::time::Instant::now();
                let mut last_sent_time = 0.0;

                for (stream, packet) in ictx.packets() {
                    if *cancel_rx.borrow() {
                        break 'outer;
                    }

                    if stream.index() == stream_index && decoder.send_packet(&packet).is_ok() {
                        let mut decoded = ffmpeg::frame::Video::empty();
                        while decoder.receive_frame(&mut decoded).is_ok() {
                            if *cancel_rx.borrow() {
                                break 'outer;
                            }

                            let pts = decoded.pts().unwrap_or(0);
                            if first_pts.is_none() {
                                first_pts = Some(pts);
                            }

                            let pts_diff = pts - first_pts.unwrap();
                            let target_time = pts_diff as f64 * time_base_f64;
                            let elapsed = start_time.elapsed().as_secs_f64();

                            if target_time > elapsed {
                                let sleep_duration =
                                    std::time::Duration::from_secs_f64(target_time - elapsed);
                                std::thread::sleep(sleep_duration);
                            }

                            // Dynamic FPS throttling to save CPU
                            let target_fps = config_rx.borrow().fps as f64;
                            let frame_duration = 1.0 / target_fps;

                            // If this frame's target time is less than the duration from the last frame we sent, drop it!
                            if target_time < last_sent_time + frame_duration && last_sent_time > 0.0
                            {
                                continue;
                            }

                            let mut rgb_frame = ffmpeg::frame::Video::empty();
                            if scaler.run(&decoded, &mut rgb_frame).is_ok() {
                                let mut buffer = recycle_rx
                                    .try_recv()
                                    .unwrap_or_else(|_| vec![0u8; frame_size]);
                                if buffer.len() != frame_size {
                                    buffer.resize(frame_size, 0);
                                }

                                let stride = rgb_frame.stride(0);
                                let data = rgb_frame.data(0);
                                let expected_row_bytes = (width * 4) as usize;

                                // Optimization: If the video frame is densely packed (stride == expected row width),
                                // we can perform a single bulk `copy_from_slice` to leverage a highly optimized
                                // `memcpy` instead of iterating row-by-row and suffering bounds checking overhead.
                                if stride == expected_row_bytes {
                                    buffer[..frame_size].copy_from_slice(&data[..frame_size]);
                                } else {
                                    // Optimization: Use exact chunks and zip to eliminate manual bounds checking
                                    // and index arithmetic, allowing LLVM to auto-vectorize the memory copy.
                                    for (dst_row, src_row) in buffer[..frame_size]
                                        .chunks_exact_mut(expected_row_bytes)
                                        .zip(data.chunks(stride))
                                    {
                                        dst_row.copy_from_slice(&src_row[..expected_row_bytes]);
                                    }
                                }

                                if let Some(img) = image::RgbaImage::from_raw(width, height, buffer)
                                {
                                    let pooled_img =
                                        Box::new(PooledImage::new(img, recycle_tx.clone()));
                                    match tx.try_send(Event::BackgroundVideoFrame(pooled_img)) {
                                        Ok(_) => {
                                            last_sent_time = target_time;
                                        }
                                        Err(tokio::sync::mpsc::error::TrySendError::Full(
                                            Event::BackgroundVideoFrame(dropped),
                                        )) => {
                                            let _ = recycle_tx.try_send(dropped.into_raw());
                                        }
                                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                            break 'outer
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
                // Flush decoder at end of stream
                if decoder.send_eof().is_ok() {
                    let mut decoded = ffmpeg::frame::Video::empty();
                    while decoder.receive_frame(&mut decoded).is_ok() {
                        let mut rgb_frame = ffmpeg::frame::Video::empty();
                        if scaler.run(&decoded, &mut rgb_frame).is_ok() {
                            let mut buffer = recycle_rx
                                .try_recv()
                                .unwrap_or_else(|_| vec![0u8; frame_size]);
                            if buffer.len() != frame_size {
                                buffer.resize(frame_size, 0);
                            }

                            let stride = rgb_frame.stride(0);
                            let data = rgb_frame.data(0);
                            let expected_row_bytes = (width * 4) as usize;

                            // Optimization: If the video frame is densely packed (stride == expected row width),
                            // we can perform a single bulk `copy_from_slice` to leverage a highly optimized
                            // `memcpy` instead of iterating row-by-row and suffering bounds checking overhead.
                            if stride == expected_row_bytes {
                                buffer[..frame_size].copy_from_slice(&data[..frame_size]);
                            } else {
                                // Optimization: Use exact chunks and zip to eliminate manual bounds checking
                                // and index arithmetic, allowing LLVM to auto-vectorize the memory copy.
                                for (dst_row, src_row) in buffer[..frame_size]
                                    .chunks_exact_mut(expected_row_bytes)
                                    .zip(data.chunks(stride))
                                {
                                    dst_row.copy_from_slice(&src_row[..expected_row_bytes]);
                                }
                            }

                            if let Some(img) = image::RgbaImage::from_raw(width, height, buffer) {
                                let pooled_img =
                                    Box::new(PooledImage::new(img, recycle_tx.clone()));
                                let _ = tx.try_send(Event::BackgroundVideoFrame(pooled_img));
                            }
                        }
                    }
                }
            }

            info!("ffmpeg-next local decoder exited");
        });

        Ok(())
    }
    pub async fn run_decoder(
        url: String,
        tx: Sender<Event>,
        mut cancel_rx: tokio::sync::watch::Receiver<bool>,
        mut recycle_rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
        recycle_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    ) -> Result<()> {
        // Validate URL before passing to FFmpeg to prevent command injection/arbitrary file reads
        let parsed_url = match url::Url::parse(&url) {
            Ok(u) => u,
            Err(e) => {
                warn!("Invalid video URL provided: {}. Error: {}", url, e);
                return Ok(());
            }
        };

        let scheme = parsed_url.scheme();
        if scheme != "http" && scheme != "https" {
            warn!("Security violation: Unsupported video URL scheme '{}'. Only http/https are allowed.", scheme);
            return Ok(());
        }

        let safe_url = parsed_url.to_string();

        let ffmpeg_bin = match crate::modules::utils::resolve_binary("ffmpeg") {
            Some(path) => path,
            None => {
                warn!("FFmpeg is not installed or not in trusted paths! Video backgrounds will not play.");
                return Ok(());
            }
        };

        // Runtime check to verify FFmpeg is available before trying to decode
        if Command::new(&ffmpeg_bin)
            .arg("-version")
            .output()
            .await
            .is_err()
        {
            warn!("FFmpeg failed to execute! Video backgrounds will not play.");
            return Ok(());
        }

        info!("Starting FFmpeg video decoder for: {}", safe_url);

        let width = 540;
        let height = 960;
        let frame_size = width * height * 4;

        let mut child = Command::new(&ffmpeg_bin)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-protocol_whitelist",
                "http,https,tcp,tls,crypto",
                "-re", // Read input at native frame rate so we don't peg the CPU!
                "-stream_loop",
                "-1", // Loop the video stream infinitely
                "-i",
                &safe_url,
                // Scale and crop seamlessly to ensure it fits the 9:16 Canvas perfectly
                "-vf",
                "scale=540:960:force_original_aspect_ratio=increase,crop=540:960",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-r",
                "30", // Lock output to 30fps
                "-",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true) // Ensure the FFmpeg process dies instantly if the task is dropped
            .spawn()?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to open ffmpeg stdout"))?;

        loop {
            let mut buffer = recycle_rx
                .try_recv()
                .unwrap_or_else(|_| vec![0u8; frame_size]);

            if buffer.len() != frame_size {
                buffer.resize(frame_size, 0);
            }
            tokio::select! {
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() {
                        info!("Cancelling video stream playback");
                        break;
                    }
                }
                result = stdout.read_exact(&mut buffer) => {
                    match result {
                        Ok(_) => {
                            if let Some(img) = image::RgbaImage::from_raw(width as u32, height as u32, buffer) {
                                let pooled_img = Box::new(PooledImage::new(img, recycle_tx.clone()));
                            match tx.try_send(Event::CanvasVideoFrame(pooled_img)) {
                                    Ok(_) => {}
                                Err(tokio::sync::mpsc::error::TrySendError::Full(Event::CanvasVideoFrame(dropped))) => {
                                        warn!("Renderer busy, dropping video frame to prevent memory bloat");
                                        let _ = recycle_tx.try_send(dropped.into_raw());
                                    }
                                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break,
                                    _ => {}
                                }
                            }
                        }
                        Err(e) => {
                            warn!("FFmpeg stream ended or errored: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
