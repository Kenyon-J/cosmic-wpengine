import re

with open("src/modules/event.rs", "r") as f:
    content = f.read()

# Add PooledAudioBuffer definition
pooled_audio_buffer = """
pub struct PooledAudioBuffer<T> {
    buf: Option<Box<[T]>>,
    recycle_tx: tokio::sync::mpsc::Sender<Box<[T]>>,
}

impl<T> PooledAudioBuffer<T> {
    pub fn new(buf: Box<[T]>, recycle_tx: tokio::sync::mpsc::Sender<Box<[T]>>) -> Self {
        Self { buf: Some(buf), recycle_tx }
    }

    pub fn into_raw(mut self) -> Box<[T]> {
        self.buf.take().unwrap()
    }
}

impl<T> std::ops::Deref for PooledAudioBuffer<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.buf.as_ref().unwrap()
    }
}

impl<T> Drop for PooledAudioBuffer<T> {
    fn drop(&mut self) {
        if let Some(buf) = self.buf.take() {
            let _ = self.recycle_tx.try_send(buf);
        }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for PooledAudioBuffer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PooledAudioBuffer").finish()
    }
}
"""

content = content.replace("#[derive(Debug)]\npub enum Event {", pooled_audio_buffer + "\n#[derive(Debug)]\npub enum Event {")

# Modify Event::AudioFrame
content = content.replace("bands: Box<[f32]>,", "bands: PooledAudioBuffer<f32>,")
content = content.replace("waveform: Box<[f32]>,", "waveform: PooledAudioBuffer<f32>,")

with open("src/modules/event.rs", "w") as f:
    f.write(content)

print("Patch applied successfully.")
