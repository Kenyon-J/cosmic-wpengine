use anyhow::Result;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use super::event::Event;

pub struct VideoDecoder;

impl VideoDecoder {
    pub async fn run_decoder(
        url: String,
        tx: Sender<Event>,
        mut cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        info!("Starting FFmpeg video decoder for: {}", url);

        let width = 1080;
        let height = 1920;
        let frame_size = width * height * 4;

        let mut child = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-re", // Read input at native frame rate so we don't peg the CPU!
                "-stream_loop",
                "-1", // Loop the video stream infinitely
                "-i",
                &url,
                // Scale and crop seamlessly to ensure it fits the 9:16 Canvas perfectly
                "-vf",
                "scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920",
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
        let mut buffer = vec![0u8; frame_size];

        loop {
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
                            if let Some(img) = image::RgbaImage::from_raw(width as u32, height as u32, buffer.clone()) {
                                if tx.send(Event::VideoFrame(img)).await.is_err() {
                                    break;
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
