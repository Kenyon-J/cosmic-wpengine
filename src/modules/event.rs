pub struct PooledAudioBuffer<T> {
    buf: Option<Box<[T]>>,
    recycle_tx: tokio::sync::mpsc::Sender<Box<[T]>>,
}

impl<T> PooledAudioBuffer<T> {
    pub fn new(buf: Box<[T]>, recycle_tx: tokio::sync::mpsc::Sender<Box<[T]>>) -> Self {
        Self {
            buf: Some(buf),
            recycle_tx,
        }
    }
}

impl<T> std::ops::Deref for PooledAudioBuffer<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.buf.as_deref().unwrap_or(&[])
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

#[derive(Debug)]
pub enum Event {
    ConfigUpdated(Box<super::config::Config>, Box<super::config::ThemeLayout>),
    TrackChanged(Box<TrackInfo>),
    PlaybackStopped,
    PlaybackResumed,
    PlayerShutDown,
    PlaybackPosition(std::time::Duration),
    AudioFrame {
        bands: PooledAudioBuffer<f32>,
        waveform: PooledAudioBuffer<f32>,
    },
    BackgroundVideoFrame(Box<super::video::PooledImage>),
    CanvasVideoFrame(Box<super::video::PooledImage>),
    WeatherUpdated(Box<WeatherData>),
}

#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub title: Box<str>,
    pub artist: Box<str>,
    pub album: Box<str>,
    pub album_art: Option<image::RgbaImage>,
    pub palette: Option<Box<[[f32; 3]]>>,
    pub lyrics: Option<Box<[LyricLine]>>,
    pub video_url: Option<Box<str>>,
}

#[derive(Debug, Clone)]
pub struct LyricLine {
    pub start_time_secs: f32,
    pub text: Box<str>,
    pub text_hash: u64,
}

#[derive(Debug, Clone)]
pub struct WeatherData {
    pub condition: WeatherCondition,
    pub temperature_celsius: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WeatherCondition {
    Clear,
    PartlyCloudy,
    Cloudy,
    Rain,
    Snow,
    Thunderstorm,
    Fog,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pooled_audio_buffer_new() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let data: Box<[f32]> = vec![1.0, 2.0, 3.0].into_boxed_slice();

        let buffer = PooledAudioBuffer::new(data.clone(), tx);

        // Test Deref implementation to ensure data is correct
        assert_eq!(&*buffer, &*data);
        assert_eq!(buffer.len(), 3);

        // Test drop behavior and recycle_tx
        drop(buffer);
        let recycled = rx
            .blocking_recv()
            .expect("Buffer should have been sent to recycle channel on drop");
        assert_eq!(recycled, data);
    }
}
