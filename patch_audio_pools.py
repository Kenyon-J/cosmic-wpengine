import re

with open("src/modules/audio.rs", "r") as f:
    content = f.read()

# Replace into_boxed_slice() calls in audio.rs with pool usage

# Let's import the PooledAudioBuffer
content = content.replace("use super::event::Event;", "use super::event::{Event, PooledAudioBuffer};")

# Add the recycle channels setup
# Find:         let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(16);
setup = """        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(16);
        let (recycle_bands_tx, mut recycle_bands_rx) = tokio::sync::mpsc::channel::<Box<[f32]>>(4);
        let (recycle_waveform_tx, mut recycle_waveform_rx) = tokio::sync::mpsc::channel::<Box<[f32]>>(4);
        let (recycle_complex_tx, mut recycle_complex_rx) = tokio::sync::mpsc::channel::<Vec<Complex<f32>>>(4);
"""
content = content.replace("        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(16);\n", setup)

# Update the loop
old_loop = """                        Ok(Some(samples)) => {
                            if !*visible_rx.borrow() {
                                continue;
                            }

                            let mut process_buffer = Vec::with_capacity(FFT_SIZE);
                            for (i, &s) in samples.iter().enumerate() {
                                let window = hann_window[i];
                                process_buffer.push(Complex {
                                    re: s * window,
                                    im: 0.0,
                                });
                            }

                            let fft_clone = std::sync::Arc::clone(&fft);

                            // Optimization: Offload heavy CPU bound math to the dedicated blocking thread pool
                            let (normalised, original_waveform) = tokio::task::spawn_blocking(move || {
                                fft_clone.process(&mut process_buffer);
                                let half = FFT_SIZE / 2;
                                let norm: Vec<f32> = process_buffer[0..half]
                                    .iter()
                                    .map(|c| (c.norm() / SCALE_FACTOR).clamp(0.0, 1.0))
                                    .collect();
                                (norm, samples) // Return the processed data
                            }).await.unwrap();

                            let _ = tx
                                .send(Event::AudioFrame {
                                    bands: normalised.into_boxed_slice(),
                                    waveform: original_waveform.into_boxed_slice(),
                                })
                                .await;
                        }"""

new_loop = """                        Ok(Some(samples)) => {
                            if !*visible_rx.borrow() {
                                continue;
                            }

                            let mut process_buffer = recycle_complex_rx.try_recv().unwrap_or_else(|_| Vec::with_capacity(FFT_SIZE));
                            process_buffer.clear();
                            for (i, &s) in samples.iter().enumerate() {
                                let window = hann_window[i];
                                process_buffer.push(Complex {
                                    re: s * window,
                                    im: 0.0,
                                });
                            }

                            let fft_clone = std::sync::Arc::clone(&fft);

                            let mut norm_buffer = recycle_bands_rx.try_recv().map(|b| b.into_vec()).unwrap_or_else(|_| Vec::with_capacity(FFT_SIZE / 2));
                            let recycle_complex_tx_clone = recycle_complex_tx.clone();

                            // Optimization: Offload heavy CPU bound math to the dedicated blocking thread pool
                            let (normalised, original_waveform) = tokio::task::spawn_blocking(move || {
                                fft_clone.process(&mut process_buffer);
                                let half = FFT_SIZE / 2;
                                norm_buffer.clear();
                                norm_buffer.extend(process_buffer[0..half]
                                    .iter()
                                    .map(|c| (c.norm() / SCALE_FACTOR).clamp(0.0, 1.0)));
                                let _ = recycle_complex_tx_clone.try_send(process_buffer);
                                (norm_buffer, samples) // Return the processed data
                            }).await.unwrap();

                            let _ = tx
                                .send(Event::AudioFrame {
                                    bands: PooledAudioBuffer::new(normalised.into_boxed_slice(), recycle_bands_tx.clone()),
                                    waveform: PooledAudioBuffer::new(original_waveform.into_boxed_slice(), recycle_waveform_tx.clone()),
                                })
                                .await;
                        }"""
content = content.replace(old_loop, new_loop)

# Update the empty send
old_empty = """                            let _ = tx
                                .send(Event::AudioFrame {
                                    bands: vec![0.0; FFT_SIZE / 2].into_boxed_slice(),
                                    waveform: vec![0.0; FFT_SIZE].into_boxed_slice(),
                                })
                                .await;"""
new_empty = """                            let mut norm_buffer = recycle_bands_rx.try_recv().map(|b| b.into_vec()).unwrap_or_else(|_| Vec::with_capacity(FFT_SIZE / 2));
                            norm_buffer.clear();
                            norm_buffer.extend(std::iter::repeat(0.0).take(FFT_SIZE / 2));
                            let mut wave_buffer = recycle_waveform_rx.try_recv().map(|b| b.into_vec()).unwrap_or_else(|_| Vec::with_capacity(FFT_SIZE));
                            wave_buffer.clear();
                            wave_buffer.extend(std::iter::repeat(0.0).take(FFT_SIZE));

                            let _ = tx
                                .send(Event::AudioFrame {
                                    bands: PooledAudioBuffer::new(norm_buffer.into_boxed_slice(), recycle_bands_tx.clone()),
                                    waveform: PooledAudioBuffer::new(wave_buffer.into_boxed_slice(), recycle_waveform_tx.clone()),
                                })
                                .await;"""
content = content.replace(old_empty, new_empty)

with open("src/modules/audio.rs", "w") as f:
    f.write(content)

print("Patch applied successfully.")
