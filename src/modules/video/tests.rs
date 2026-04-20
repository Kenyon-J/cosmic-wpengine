#![cfg(test)]

use super::*;
use image::RgbaImage;
use tokio::sync::mpsc;

/// Tests that a `PooledImage` correctly releases its internal `RgbaImage` into its raw vector representation when dropped/converted.
/// This prevents memory leaks and ensures our frame pooling mechanism reuses buffers instead of thrashing the allocator.
#[test]
fn test_pooled_image_into_raw() {
    let (tx, _rx) = mpsc::channel(1);

    // Create a 2x2 red image
    let mut img = RgbaImage::new(2, 2);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba([255, 0, 0, 255]);
    }
    let expected_raw = img.clone().into_raw();

    let pooled_image = PooledImage::new(img, tx);
    let raw = pooled_image.into_raw();

    assert_eq!(raw, expected_raw);
}
