use anyhow::Result;
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
        self.img.take().unwrap().into_raw()
    }
}

impl std::ops::Deref for PooledImage {
    type Target = image::RgbaImage;
    fn deref(&self) -> &Self::Target {
        self.img.as_ref().unwrap()
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
                                let pooled_img = PooledImage::new(img, recycle_tx.clone());
                                match tx.try_send(Event::VideoFrame(pooled_img)) {
                                    Ok(_) => {}
                                    Err(tokio::sync::mpsc::error::TrySendError::Full(Event::VideoFrame(dropped))) => {
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
