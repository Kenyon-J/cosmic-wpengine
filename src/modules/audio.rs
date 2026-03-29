use anyhow::Result;
use pipewire::context::ContextBox;
use pipewire::main_loop::MainLoopBox;
use pipewire::properties::properties;
use rustfft::{num_complex::Complex, FftPlanner};
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use super::event::Event;

const FFT_SIZE: usize = 2048;
// Tweak this value to change how tall the bars render at normal volumes
const SCALE_FACTOR: f32 = 100.0;

pub struct AudioCapture;

impl AudioCapture {
    pub async fn run(
        tx: Sender<Event>,
        is_visible: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<()> {
        info!("Audio capture started");

        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(16);

        tokio::task::spawn_blocking(move || Self::run_pipewire_capture(audio_tx));

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        let mut last_warn = std::time::Instant::now() - std::time::Duration::from_secs(5);

        let mut buffer: Vec<Complex<f32>> = Vec::with_capacity(FFT_SIZE);

        loop {
            match tokio::time::timeout(tokio::time::Duration::from_millis(250), audio_rx.recv())
                .await
            {
                Ok(Some(samples)) => {
                    if !is_visible.load(std::sync::atomic::Ordering::Relaxed) {
                        continue;
                    }

                    buffer.clear();
                    for (i, &s) in samples.iter().enumerate() {
                        let window = 0.5
                            * (1.0
                                - (2.0 * std::f32::consts::PI * i as f32
                                    / (FFT_SIZE as f32 - 1.0))
                                    .cos());
                        buffer.push(Complex {
                            re: s * window,
                            im: 0.0,
                        });
                    }

                    fft.process(&mut buffer);

                    let half = FFT_SIZE / 2;

                    let mut normalised = Vec::with_capacity(half);
                    for c in &buffer[0..half] {
                        let magnitude = c.norm();
                        normalised.push((magnitude / SCALE_FACTOR).clamp(0.0, 1.0));
                    }

                    let _ = tx
                        .send(Event::AudioFrame {
                            bands: normalised,
                            waveform: samples,
                        })
                        .await;
                }
                Ok(None) => break,
                Err(_) => {
                    if last_warn.elapsed() >= std::time::Duration::from_secs(5) {
                        warn!("PipeWire audio receive timeout - no data arriving. (Is stream paused/empty?)");
                        last_warn = std::time::Instant::now();
                    }
                    let _ = tx
                        .send(Event::AudioFrame {
                            bands: vec![0.0; FFT_SIZE / 2],
                            waveform: vec![0.0; FFT_SIZE],
                        })
                        .await;
                }
            }
        }

        Ok(())
    }

    fn run_pipewire_capture(tx: tokio::sync::mpsc::Sender<Vec<f32>>) {
        pipewire::init();

        let mainloop = MainLoopBox::new(None).expect("Failed to create PipeWire mainloop");
        let context =
            ContextBox::new(mainloop.loop_(), None).expect("Failed to create PipeWire context");
        let core = context
            .connect(None)
            .expect("Failed to connect to PipeWire");

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
            .expect("Failed to create stream");

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

                    let mut mono_samples = Vec::new();
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
                                for i in 0..valid {
                                    mono_samples.push((left_f32[i] + right_f32[i]) * 0.5);
                                }
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
                                for chunk in f32_samples.chunks(2) {
                                    if chunk.len() == 2 {
                                        mono_samples.push((chunk[0] + chunk[1]) * 0.5);
                                    } else {
                                        mono_samples.push(chunk[0]);
                                    }
                                }
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

                    for s in mono_samples {
                        sample_buffer.push(s);
                    }

                    while sample_buffer.len() >= FFT_SIZE {
                        let frame = sample_buffer[..FFT_SIZE].to_vec();
                        sample_buffer.drain(..FFT_SIZE);
                        let _ = tx.try_send(frame);
                    }
                } else if frame_counter.is_multiple_of(50) {
                    warn!("PipeWire process callback fired, but dequeue_buffer() returned None.");
                }
            })
            .register()
            .expect("Failed to register stream listener");

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
        .expect("Failed to serialize POD")
        .0
        .into_inner();

        // 4. Cast the raw bytes into the binary C-level Pod
        let pod = pipewire::spa::pod::Pod::from_bytes(&values).expect("Failed to parse POD bytes");

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
            .expect("Failed to connect stream");

        info!("PipeWire capture stream created");
        mainloop.run();
    }
}
