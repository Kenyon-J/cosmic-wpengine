import re

with open("src/modules/audio.rs", "r") as f:
    content = f.read()

# We need to hook up recycle_waveform_rx to run_pipewire_capture so it stops allocating `Vec<f32>` per frame.
# Looking at AudioCapture::run, `run_pipewire_capture` is spawned:
#        std::thread::spawn(move || {
#            if let Err(e) = Self::run_pipewire_capture(audio_tx) {

# Let's modify run_pipewire_capture to accept recycle_waveform_rx
content = content.replace("std::thread::spawn(move || {\n            if let Err(e) = Self::run_pipewire_capture(audio_tx) {",
                          "std::thread::spawn(move || {\n            if let Err(e) = Self::run_pipewire_capture(audio_tx, recycle_waveform_rx) {")

content = content.replace("fn run_pipewire_capture(tx: tokio::sync::mpsc::Sender<Vec<f32>>) -> Result<()> {",
                          "fn run_pipewire_capture(tx: tokio::sync::mpsc::Sender<Vec<f32>>, mut recycle_rx: tokio::sync::mpsc::Receiver<Box<[f32]>>) -> Result<()> {")

# In run_pipewire_capture:
# let mut sample_buffer = Vec::with_capacity(FFT_SIZE * 2);
# When a chunk is ready to send:
#                    while sample_buffer.len() >= FFT_SIZE {
#                        let frame = sample_buffer[..FFT_SIZE].to_vec();
#                        sample_buffer.drain(..FFT_SIZE);

replacement = """                    while sample_buffer.len() >= FFT_SIZE {
                        let mut frame = recycle_rx.try_recv().map(|b| b.into_vec()).unwrap_or_else(|_| Vec::with_capacity(FFT_SIZE));
                        frame.clear();
                        frame.extend_from_slice(&sample_buffer[..FFT_SIZE]);
                        sample_buffer.drain(..FFT_SIZE);"""
content = content.replace("                    while sample_buffer.len() >= FFT_SIZE {\n                        let frame = sample_buffer[..FFT_SIZE].to_vec();\n                        sample_buffer.drain(..FFT_SIZE);", replacement)

with open("src/modules/audio.rs", "w") as f:
    f.write(content)

print("Patch applied successfully.")
