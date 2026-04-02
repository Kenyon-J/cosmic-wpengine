use anyhow::Result;
use std::sync::{Mutex, OnceLock};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use super::event::Event;

static FRAME_POOL: OnceLock<Mutex<Vec<Vec<u8>>>> = OnceLock::new();

pub fn get_frame_pool() -> &'static Mutex<Vec<Vec<u8>>> {
    FRAME_POOL.get_or_init(|| Mutex::new(Vec::with_capacity(3)))
}

pub struct VideoDecoder;

impl VideoDecoder {
    pub async fn run_decoder(
        url: String,
        tx: Sender<Event>,
        mut cancel_rx: tokio::sync::watch::Receiver<bool>,
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

        // Runtime check to verify FFmpeg is available before trying to decode
        if Command::new("ffmpeg")
            .arg("-version")
            .output()
            .await
            .is_err()
        {
            warn!("FFmpeg is not installed or not in PATH! Video backgrounds will not play.");
            return Ok(());
        }

        info!("Starting FFmpeg video decoder for: {}", safe_url);

        let width = 540;
        let height = 960;
        let frame_size = width * height * 4;

        let mut child = Command::new("ffmpeg")
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

        let mut stdout = child.stdout.take().expect("Failed to open stdout");

        loop {
            let mut buffer = get_frame_pool()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pop()
                .unwrap_or_else(|| vec![0u8; frame_size]);
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
                                match tx.try_send(Event::VideoFrame(img)) {
                                    Ok(_) => {}
                                    Err(tokio::sync::mpsc::error::TrySendError::Full(Event::VideoFrame(dropped_img))) => {
                                        warn!("Renderer busy, dropping video frame to prevent memory bloat");
                                        if let Ok(mut pool) = get_frame_pool().lock() {
                                            if pool.len() < 3 {
                                                pool.push(dropped_img.into_raw());
                                            }
                                        }
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
