// =============================================================================
// modules/audio.rs
// =============================================================================
// Captures audio from PipeWire and performs FFT (Fast Fourier Transform)
// analysis to produce a frequency spectrum for the visualiser.
//
// For beginners: FFT takes a chunk of raw audio samples (a waveform in time)
// and converts it into frequency bands (how loud each pitch is). This is
// exactly what those equaliser visualisers show.
//
// This module:
//   1. Opens a PipeWire stream to capture the system audio mix
//   2. Collects samples into a buffer
//   3. Runs FFT on each buffer
//   4. Sends normalised frequency bands to the renderer
// =============================================================================

use anyhow::Result;
use tokio::sync::mpsc::Sender;
use tracing::info;
use rustfft::{FftPlanner, num_complex::Complex};

use super::event::Event;

/// How many audio samples to collect before running FFT.
/// Larger = more frequency resolution, but higher latency.
const FFT_SIZE: usize = 2048;

pub struct AudioCapture;

impl AudioCapture {
    pub async fn run(tx: Sender<Event>) -> Result<()> {
        info!("Audio capture started");

        // PipeWire integration runs in a separate thread because its mainloop
        // is synchronous. We bridge back to async-land via a std channel.
        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(8);

        // Spawn the PipeWire capture on a blocking thread
        tokio::task::spawn_blocking(move || {
            Self::run_pipewire_capture(audio_tx)
        });

        // FFT planner — reused across frames for efficiency
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        while let Some(samples) = audio_rx.recv().await {
            // Convert real samples to complex numbers (FFT requires complex input)
            let mut buffer: Vec<Complex<f32>> = samples.iter()
                .map(|&s| Complex { re: s, im: 0.0 })
                .collect();

            // Run the FFT in place — buffer is transformed from time to frequency domain
            fft.process(&mut buffer);

            // We only need the first half of the FFT output (the rest is a mirror)
            let half = FFT_SIZE / 2;
            let magnitudes: Vec<f32> = buffer[..half].iter()
                .map(|c| c.norm()) // magnitude = sqrt(re² + im²)
                .collect();

            // Normalise to 0.0–1.0 range
            let max = magnitudes.iter().cloned().fold(0.0f32, f32::max);
            let normalised: Vec<f32> = if max > 0.0 {
                magnitudes.iter().map(|&m| m / max).collect()
            } else {
                vec![0.0; half]
            };

            let _ = tx.send(Event::AudioFrame(normalised)).await;
        }

        Ok(())
    }

    /// Runs the PipeWire capture loop on a blocking thread.
    /// Sends raw sample buffers back via the channel.
    fn run_pipewire_capture(tx: tokio::sync::mpsc::Sender<Vec<f32>>) {
        // NOTE: Full PipeWire setup requires creating a mainloop, context,
        // and stream. This is a simplified skeleton showing the structure.
        // The pipewire crate's examples show the full boilerplate.

        use pipewire::main_loop::MainLoop;
        use pipewire::context::Context;
        use pipewire::properties::properties;

        let mainloop = MainLoop::new(None).expect("Failed to create PipeWire mainloop");
        let context = Context::new(&mainloop).expect("Failed to create PipeWire context");
        let core = context.connect(None).expect("Failed to connect to PipeWire");

        // Create a capture stream targeting the monitor (system mix)
        // This captures whatever is currently playing through your speakers
        let props = properties! {
            "media.type" => "Audio",
            "media.category" => "Capture",
            "media.role" => "Music",
            // "monitor" means we capture the output mix, not a microphone
            "stream.capture.sink" => "true",
        };

        // Buffer to accumulate samples before sending for FFT
        let mut sample_buffer: Vec<f32> = Vec::with_capacity(FFT_SIZE);

        // The actual stream setup and callback registration would go here.
        // In the callback, we'd push samples to sample_buffer and when
        // sample_buffer.len() >= FFT_SIZE, send it via tx and clear the buffer.

        // For now, log that we're ready and run the mainloop
        info!("PipeWire capture stream created");
        mainloop.run();
    }
}
