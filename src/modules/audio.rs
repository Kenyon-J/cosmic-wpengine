use anyhow::Result;
use pipewire::context::ContextBox;
use pipewire::main_loop::MainLoopBox;
use pipewire::properties::properties;
use rustfft::{num_complex::Complex, FftPlanner};
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use super::event::{Event, PooledAudioBuffer};

const FFT_SIZE: usize = 2048;
// Tweak this value to change how tall the bars render at normal volumes
const SCALE_FACTOR: f32 = 100.0;

pub struct AudioCapture;

impl AudioCapture {
    pub async fn run(
        tx: Sender<Event>,
        mut visible_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        info!("Audio capture started");

        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(16);
        let (recycle_bands_tx, mut recycle_bands_rx) = tokio::sync::mpsc::channel::<Box<[f32]>>(4);
        let (recycle_waveform_tx, recycle_waveform_rx) =
            tokio::sync::mpsc::channel::<Box<[f32]>>(4);
        let (recycle_complex_tx, mut recycle_complex_rx) =
            tokio::sync::mpsc::channel::<Vec<Complex<f32>>>(4);

        std::thread::spawn(move || {
            if let Err(e) = Self::run_pipewire_capture(audio_tx, recycle_waveform_rx) {
                warn!("PipeWire capture failed: {}", e);
            }
        });

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        let mut last_warn = std::time::Instant::now() - std::time::Duration::from_secs(5);

        // Pre-calculate the Hann window coefficients to avoid redundant trig calculations in the hot loop.
        // This optimization saves ~2048 cos() calls per FFT processing window.
        let hann_window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| {
                0.5 * (1.0
                    - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE as f32 - 1.0)).cos())
            })
            .collect();

        loop {
            tokio::select! {
                Ok(_) = visible_rx.changed() => {
                    // Wake up and re-evaluate visibility without waiting
                }
                res = tokio::time::timeout(tokio::time::Duration::from_millis(250), audio_rx.recv()) => {
                    match res {
                        Ok(Some(samples)) => {
                            if !*visible_rx.borrow() {
                                continue;
                            }

                            let mut process_buffer = recycle_complex_rx.try_recv().unwrap_or_else(|_| Vec::with_capacity(FFT_SIZE));
                            process_buffer.clear();
                            // Optimization: Use zipped iterators and `.extend()` instead of manual `.push()`
                            // to enable LLVM auto-vectorization and eliminate bounds checking overhead
                            process_buffer.extend(samples.iter().zip(hann_window.iter()).map(|(&s, &w)| Complex {
                                re: s * w,
                                im: 0.0,
                            }));

                            let fft_clone = std::sync::Arc::clone(&fft);

                            let mut norm_buffer = recycle_bands_rx.try_recv().map(|b| b.into_vec()).unwrap_or_else(|_| Vec::with_capacity(FFT_SIZE / 2));
                            let recycle_complex_tx_clone = recycle_complex_tx.clone();

                            // Optimization: Offload heavy CPU bound math to the dedicated blocking thread pool
                            let (normalised, original_waveform) = match tokio::task::spawn_blocking(move || {
                                fft_clone.process(&mut process_buffer);
                                let half = FFT_SIZE / 2;
                                norm_buffer.clear();
                                // Pre-allocate the required capacity to avoid reallocations during extension
                                norm_buffer.reserve_exact(half);
                                // Avoid allocating inside the mapping closure; use zipped iterators or direct maps
                                norm_buffer.extend(process_buffer[0..half]
                                    .iter()
                                    .map(|c| (c.norm() / SCALE_FACTOR).clamp(0.0, 1.0)));
                                let _ = recycle_complex_tx_clone.try_send(process_buffer);
                                (norm_buffer, samples) // Return the processed data
                            }).await {
                                Ok(res) => res,
                                Err(e) => {
                                    tracing::error!("FFT task failed: {}", e);
                                    let mut default_norm = Vec::with_capacity(FFT_SIZE / 2);
                                    default_norm.extend(std::iter::repeat_n(0.0, FFT_SIZE / 2));
                                    let mut default_wave = Vec::with_capacity(FFT_SIZE);
                                    default_wave.extend(std::iter::repeat_n(0.0, FFT_SIZE));
                                    (default_norm, default_wave)
                                }
                            };

                            let _ = tx
                                .send(Event::AudioFrame {
                                    bands: PooledAudioBuffer::new(normalised.into_boxed_slice(), recycle_bands_tx.clone()),
                                    waveform: PooledAudioBuffer::new(original_waveform.into_boxed_slice(), recycle_waveform_tx.clone()),
                                })
                                .await;
                        }
                        Ok(None) => break,
                        Err(_) => {
                            if last_warn.elapsed() >= std::time::Duration::from_secs(5) {
                                warn!("PipeWire audio receive timeout - no data arriving. (Is stream paused/empty?)");
                                last_warn = std::time::Instant::now();
                            }
                            let mut norm_buffer = recycle_bands_rx.try_recv().map(|b| b.into_vec()).unwrap_or_else(|_| Vec::with_capacity(FFT_SIZE / 2));
                            norm_buffer.clear();
                            norm_buffer.extend(std::iter::repeat_n(0.0, FFT_SIZE / 2));
                            let mut wave_buffer = Vec::with_capacity(FFT_SIZE);
                            wave_buffer.clear();
                            wave_buffer.extend(std::iter::repeat_n(0.0, FFT_SIZE));

                            let _ = tx
                                .send(Event::AudioFrame {
                                    bands: PooledAudioBuffer::new(norm_buffer.into_boxed_slice(), recycle_bands_tx.clone()),
                                    waveform: PooledAudioBuffer::new(wave_buffer.into_boxed_slice(), recycle_waveform_tx.clone()),
                                })
                                .await;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn run_pipewire_capture(
        tx: tokio::sync::mpsc::Sender<Vec<f32>>,
        mut recycle_rx: tokio::sync::mpsc::Receiver<Box<[f32]>>,
    ) -> Result<()> {
        pipewire::init();

        let mainloop = MainLoopBox::new(None)
            .map_err(|e| anyhow::anyhow!("Failed to create PipeWire mainloop: {}", e))?;
        let context = ContextBox::new(mainloop.loop_(), None)
            .map_err(|e| anyhow::anyhow!("Failed to create PipeWire context: {}", e))?;
        let core = context
            .connect(None)
            .map_err(|e| anyhow::anyhow!("Failed to connect to PipeWire: {}", e))?;

        let props = properties! {
            *pipewire::keys::APP_NAME => "cosmic-wallpaper",
            *pipewire::keys::MEDIA_TYPE => "Audio",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::MEDIA_ROLE => "Music",
            *pipewire::keys::MEDIA_CLASS => "Stream/Input/Audio",
            *pipewire::keys::STREAM_CAPTURE_SINK => "true",
            *pipewire::keys::NODE_ALWAYS_PROCESS => "true",
            *pipewire::keys::NODE_PAUSE_ON_IDLE => "false",
            "audio.format" => "F32P",
            "audio.rate" => "48000",
            "audio.channels" => "2",
            "audio.position" => "FL,FR",
        };

        let stream = pipewire::stream::StreamBox::new(&core, "cosmic-wallpaper", props)
            .map_err(|e| anyhow::anyhow!("Failed to create stream: {}", e))?;

        let mut sample_buffer = Vec::with_capacity(FFT_SIZE * 2);
        let mut frame_counter = 0u32;
        let mut empty_chunks = 0u32;
        let mut valid_chunks = 0u32;

        let _listener = stream
            .add_local_listener::<()>()
            .state_changed(|_, _, old, new| {
                info!("PipeWire stream state: {:?} -> {:?}", old, new);
            })
            .param_changed(|_, _, id, _| {
                info!("PipeWire stream param negotiated: {:?}", id);
            })
            .process(move |stream, _| {
                frame_counter += 1;

                if let Some(mut buffer) = stream.dequeue_buffer() {
                    let datas = buffer.datas_mut();
                    if datas.is_empty() {
                        return;
                    }

                    let mut valid = 0;

                    if datas.len() >= 2 {
                        let left_size = datas[0].chunk().size() as usize;
                        let right_size = datas[1].chunk().size() as usize;

                        let (left_part, right_part) = datas.split_at_mut(1);
                        if let (Some(left_data), Some(right_data)) =
                            (left_part[0].data(), right_part[0].data())
                        {
                            let valid_l = std::cmp::min(left_data.len(), left_size) / 4;
                            let valid_r = std::cmp::min(right_data.len(), right_size) / 4;
                            valid = std::cmp::min(valid_l, valid_r);

                            if valid > 0 {
                                let left_f32 = unsafe {
                                    std::slice::from_raw_parts(
                                        left_data.as_ptr() as *const f32,
                                        valid,
                                    )
                                };
                                let right_f32 = unsafe {
                                    std::slice::from_raw_parts(
                                        right_data.as_ptr() as *const f32,
                                        valid,
                                    )
                                };
                                // Optimization: Use zipped iterators and `.extend()` instead of manual `.push()`
                                // to enable LLVM auto-vectorization and eliminate bounds checking overhead
                                sample_buffer.extend(
                                    left_f32[..valid]
                                        .iter()
                                        .zip(right_f32[..valid].iter())
                                        .map(|(&l, &r)| (l + r) * 0.5),
                                );
                            }
                        }
                    } else if datas.len() == 1 {
                        let size = datas[0].chunk().size() as usize;
                        if let Some(data) = datas[0].data() {
                            valid = std::cmp::min(data.len(), size) / 4;

                            if valid > 0 {
                                let f32_samples = unsafe {
                                    std::slice::from_raw_parts(data.as_ptr() as *const f32, valid)
                                };
                                // If there is only 1 buffer in Planar (F32P) format, it is a mono stream.
                                // Averaging adjacent temporal samples acts as an unintended low-pass filter!
                                sample_buffer.extend_from_slice(f32_samples);
                            }
                        }
                    }

                    if valid == 0 {
                        empty_chunks += 1;
                        if empty_chunks.is_multiple_of(50) {
                            warn!("PipeWire process: Received {} empty chunks!", empty_chunks);
                        }
                    } else {
                        valid_chunks += 1;
                        if valid_chunks == 1 {
                            info!("PipeWire process: First valid audio chunk received!");
                        } else if valid_chunks.is_multiple_of(500) {
                            info!(
                                "PipeWire process: Heartbeat - {} valid chunks processed.",
                                valid_chunks
                            );
                        }
                    }

                    while sample_buffer.len() >= FFT_SIZE {
                        let mut frame = recycle_rx
                            .try_recv()
                            .map(|b| b.into_vec())
                            .unwrap_or_else(|_| Vec::with_capacity(FFT_SIZE));
                        frame.clear();
                        frame.extend_from_slice(&sample_buffer[..FFT_SIZE]);
                        sample_buffer.drain(..FFT_SIZE);
                        match tx.try_send(frame) {
                            Ok(_) => {}
                            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                tracing::warn!(
                                    "Audio channel closed, shutting down PipeWire callback."
                                );
                                return; // Safely halts further processing if channel dies
                            }
                            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                // Drop frames to handle backpressure safely
                            }
                        }
                    }
                } else if frame_counter.is_multiple_of(50) {
                    warn!("PipeWire process callback fired, but dequeue_buffer() returned None.");
                }
            })
            .register()
            .map_err(|e| anyhow::anyhow!("Failed to register stream listener: {}", e))?;

        // 1. Build the Rust Object AST (Keep your exact param macro from before)
        let param = pipewire::spa::pod::object!(
            // ... [Your existing macro code here] ...
            pipewire::spa::utils::SpaTypes::ObjectParamFormat,
            pipewire::spa::param::ParamType::EnumFormat,
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::MediaType,
                pipewire::spa::pod::Value::Id(pipewire::spa::utils::Id(
                    pipewire::spa::param::format::MediaType::Audio.as_raw()
                ))
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::MediaSubtype,
                pipewire::spa::pod::Value::Id(pipewire::spa::utils::Id(
                    pipewire::spa::param::format::MediaSubtype::Raw.as_raw()
                ))
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::AudioFormat,
                pipewire::spa::pod::Value::Id(pipewire::spa::utils::Id(
                    pipewire::spa::param::audio::AudioFormat::F32P.as_raw()
                ))
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::AudioRate,
                pipewire::spa::pod::Value::Int(48000_i32)
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::AudioChannels,
                pipewire::spa::pod::Value::Int(2_i32)
            )
        );

        // 2. Wrap it in the `Value` enum so the Serializer accepts it!
        let param_value = pipewire::spa::pod::Value::Object(param);

        // 3. Serialize into bytes
        let values: Vec<u8> = pipewire::spa::pod::serialize::PodSerializer::serialize(
            std::io::Cursor::new(Vec::new()),
            &param_value,
        )
        .map_err(|e| anyhow::anyhow!("Failed to serialize POD: {:?}", e))?
        .0
        .into_inner();

        // 4. Cast the raw bytes into the binary C-level Pod
        let pod = pipewire::spa::pod::Pod::from_bytes(&values)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse POD bytes"))?;

        // 5. Connect perfectly
        stream
            .connect(
                pipewire::spa::utils::Direction::Input,
                Some(0xffffffff),
                pipewire::stream::StreamFlags::AUTOCONNECT
                    | pipewire::stream::StreamFlags::MAP_BUFFERS
                    | pipewire::stream::StreamFlags::RT_PROCESS,
                &mut [pod], // Pass the perfectly formatted binary Pod
            )
            .map_err(|e| anyhow::anyhow!("Failed to connect stream: {}", e))?;

        info!("PipeWire capture stream created");
        mainloop.run();
        Ok(())
    }
}
