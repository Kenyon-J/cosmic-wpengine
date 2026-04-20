#![cfg(test)]

use super::*;

/// Tests the creation, dereferencing, and recycling behavior of `PooledAudioBuffer`.
/// This prevents memory allocations during the high-frequency audio visualizer render loop.
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
