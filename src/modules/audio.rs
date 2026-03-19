use anyhow::Result;
use rustfft::{num_complex::Complex, FftPlanner};
use tokio::sync::mpsc::Sender;
use tracing::info;
use std::sync::{Arc, Mutex};

use super::event::Event;

const FFT_SIZE: usize = 2048;

pub struct AudioCapture;

impl AudioCapture {
    pub async fn run(tx: Sender<Event>) -> Result<()> {
        info!("Audio capture started");

        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(8);

        tokio::task::spawn_blocking(move || Self::run_pipewire_capture(audio_tx));

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        loop {
            match tokio::time::timeout(tokio::time::Duration::from_millis(50), audio_rx.recv()).await {
                Ok(Some(samples)) => {
                    let mut buffer: Vec<Complex<f32>> = samples
                        .iter()
                        .map(|&s| Complex { re: s, im: 0.0 })
                        .collect();

                    fft.process(&mut buffer);

                    let half = FFT_SIZE / 2;
                    let magnitudes: Vec<f32> = buffer[..half]
                        .iter()
                        .map(|c| c.norm())
                        .collect();

                    let max = magnitudes.iter().cloned().fold(0.0f32, f32::max);
                    // Lowered threshold so even quiet music triggers the visualizer
                    let normalised: Vec<f32> = if max > 0.001 {
                        magnitudes.iter().map(|&m| m / max).collect()
                    } else {
                        vec![0.0; half]
                    };

                    let _ = tx.send(Event::AudioFrame(normalised)).await;
                }
                Ok(None) => break,
                Err(_) => {
                    // Timeout: PipeWire isn't sending data (audio paused), push zeroes to smoothly decay the bars
                    let _ = tx.send(Event::AudioFrame(vec![0.0; FFT_SIZE / 2])).await;
                }
            }
        }

        Ok(())
    }

    fn run_pipewire_capture(tx: tokio::sync::mpsc::Sender<Vec<f32>>) {
        use pipewire::context::ContextBox;
        use pipewire::main_loop::MainLoopBox;
        use pipewire::properties::properties;

        pipewire::init();

        let mainloop = MainLoopBox::new(None).expect("Failed to create PipeWire mainloop");
        let context = ContextBox::new(mainloop.loop_(), None).expect("Failed to create PipeWire context");
        let core = context
            .connect(None)
            .expect("Failed to connect to PipeWire");

        let stream = pipewire::stream::StreamBox::new(
            &core,
            "cosmic-wallpaper",
            properties! {
                *pipewire::keys::MEDIA_TYPE => "Audio",
                *pipewire::keys::MEDIA_CATEGORY => "Capture",
                *pipewire::keys::MEDIA_ROLE => "Music",
                *pipewire::keys::STREAM_CAPTURE_SINK => "true",
                "audio.format" => "F32",
                "audio.rate" => "48000",
                "audio.channels" => "2",
            },
        ).expect("Failed to create stream");

        let sample_buffer = Arc::new(Mutex::new(Vec::with_capacity(FFT_SIZE * 2)));

        let _listener = stream
            .add_local_listener::<()>()
            .process(move |stream, _| {
                if let Some(mut buffer) = stream.dequeue_buffer() {
                    let datas = buffer.datas_mut();
                    if datas.is_empty() { return; }
                    let chunk_size = datas[0].chunk().size() as usize;
                    if let Some(samples) = datas[0].data() {
                        let valid_bytes = std::cmp::min(samples.len(), chunk_size);

                        let f32_samples: &[f32] = unsafe {
                            std::slice::from_raw_parts(
                                samples.as_ptr() as *const f32,
                                valid_bytes / 4
                            )
                        };

                        let mut buf = sample_buffer.lock().unwrap();
                        buf.extend_from_slice(f32_samples);

                        if buf.len() >= FFT_SIZE {
                            let frame = buf[..FFT_SIZE].to_vec();
                            buf.clear();
                            let _ = tx.try_send(frame);
                        }
                    }
                }
            })
            .register()
            .expect("Failed to register stream listener");

        stream.connect(
            pipewire::spa::utils::Direction::Input,
            Some(0xffffffff), // PW_ID_ANY
            pipewire::stream::StreamFlags::AUTOCONNECT
                | pipewire::stream::StreamFlags::MAP_BUFFERS
                | pipewire::stream::StreamFlags::RT_PROCESS,
            &mut [],
        ).expect("Failed to connect stream");

        info!("PipeWire capture stream created");
        mainloop.run();
    }
}
