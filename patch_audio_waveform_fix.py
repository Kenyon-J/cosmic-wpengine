import re

with open("src/modules/audio.rs", "r") as f:
    content = f.read()

# Since `AudioCapture::run` has a loop where it handles timeout scenarios by sending empty buffers, it also needs access to `recycle_waveform_rx`.
# But `tokio::sync::mpsc::Receiver` cannot be cloned.
# Thus we can just allocate new Vecs in the timeout edge-case, it's not a hot loop since it only happens when no audio arrives.

# Let's revert the `recycle_waveform_rx` in the timeout branch:
old_timeout_branch = """                            let mut wave_buffer = recycle_waveform_rx.try_recv().map(|b| b.into_vec()).unwrap_or_else(|_| Vec::with_capacity(FFT_SIZE));
                            wave_buffer.clear();
                            wave_buffer.extend(std::iter::repeat_n(0.0, FFT_SIZE));

                            let _ = tx
                                .send(Event::AudioFrame {
                                    bands: PooledAudioBuffer::new(norm_buffer.into_boxed_slice(), recycle_bands_tx.clone()),
                                    waveform: PooledAudioBuffer::new(wave_buffer.into_boxed_slice(), recycle_waveform_tx.clone()),
                                })
                                .await;"""

new_timeout_branch = """                            let mut wave_buffer = Vec::with_capacity(FFT_SIZE);
                            wave_buffer.extend(std::iter::repeat_n(0.0, FFT_SIZE));

                            let _ = tx
                                .send(Event::AudioFrame {
                                    bands: PooledAudioBuffer::new(norm_buffer.into_boxed_slice(), recycle_bands_tx.clone()),
                                    waveform: PooledAudioBuffer::new(wave_buffer.into_boxed_slice(), recycle_waveform_tx.clone()),
                                })
                                .await;"""

content = content.replace(old_timeout_branch, new_timeout_branch)

with open("src/modules/audio.rs", "w") as f:
    f.write(content)

print("Patch applied successfully.")
