// =============================================================================
// modules/colour.rs
// =============================================================================
// Extracts a dominant colour palette from album art.
//
// The technique used here is simple but effective:
//   1. Downsample the image to a small thumbnail (fast)
//   2. Bucket pixels into colour regions
//   3. Find the most populated buckets
//   4. Return those as the palette
//
// A more sophisticated approach would use k-means clustering, but for a
// wallpaper engine the simple approach looks great and is much faster.
// =============================================================================

use image::{DynamicImage, GenericImageView, Rgba};

/// Extract up to `count` dominant colours from an image.
/// Returns colours as [r, g, b] floats in the range 0.0–1.0.
pub fn extract_palette(image: &DynamicImage) -> Vec<[f32; 3]> {
    // Downsample to a tiny thumbnail for speed — colour distribution is
    // preserved even at very low resolution
    let thumb = image.thumbnail(64, 64);

    // Count colour occurrences using a simple bucketing approach.
    // We reduce each channel to 8 buckets (3 bits) giving 512 possible colours.
    // This is crude but fast and works well for vibrant album art.
    let mut buckets: std::collections::HashMap<(u8, u8, u8), u32> =
        std::collections::HashMap::new();

    for (_, _, Rgba([r, g, b, a])) in thumb.pixels() {
        // Skip transparent or near-transparent pixels
        if a < 128 { continue; }

        // Quantise each channel to 32-value steps (256 / 8 = 32)
        let key = (
            (r / 32) * 32,
            (g / 32) * 32,
            (b / 32) * 32,
        );
        *buckets.entry(key).or_insert(0) += 1;
    }

    // Sort buckets by frequency, most common first
    let mut sorted: Vec<_> = buckets.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    // Take the top 5 colours, skipping near-black and near-white
    // (these are common but not interesting for colouring a wallpaper)
    sorted.iter()
        .filter(|((r, g, b), _)| {
            let brightness = (*r as u32 + *g as u32 + *b as u32) / 3;
            brightness > 30 && brightness < 220 // skip very dark and very light
        })
        .take(5)
        .map(|((r, g, b), _)| {
            [*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0]
        })
        .collect()
}

/// Linearly interpolate between two colours.
/// t=0.0 returns colour a, t=1.0 returns colour b.
pub fn lerp_colour(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Convert a time-of-day fraction (0.0–1.0) to a sky colour.
/// This gives a simple but pleasing day/night cycle as a fallback scene.
pub fn time_to_sky_colour(time: f32) -> [f32; 3] {
    // Key colours at different times of day
    let midnight = [0.02, 0.02, 0.08]; // deep blue-black
    let dawn     = [0.6,  0.3,  0.2 ]; // warm orange
    let noon     = [0.4,  0.6,  0.9 ]; // sky blue
    let dusk     = [0.7,  0.3,  0.15]; // warm red-orange

    // time: 0.0 = midnight, 0.25 = 6am, 0.5 = noon, 0.75 = 6pm, 1.0 = midnight
    match time {
        t if t < 0.25 => lerp_colour(midnight, dawn, t / 0.25),
        t if t < 0.5  => lerp_colour(dawn, noon, (t - 0.25) / 0.25),
        t if t < 0.75 => lerp_colour(noon, dusk, (t - 0.5) / 0.25),
        t             => lerp_colour(dusk, midnight, (t - 0.75) / 0.25),
    }
}
