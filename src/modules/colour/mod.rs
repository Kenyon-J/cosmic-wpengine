use image::{DynamicImage, GenericImageView, Rgba};

pub fn extract_palette(image: &DynamicImage) -> Box<[[f32; 3]]> {
    // Optimization: Skip expensive Lanczos3 filtering and extra allocations
    // by sampling the original image directly. For a coarse 512-bucket histogram,
    // simple nearest-neighbor sampling is significantly faster and more than sufficient.
    let (width, height) = image.dimensions();
    let step_x = (width / 64).max(1);
    let step_y = (height / 64).max(1);

    // Optimization: Use a fixed-size array for the histogram instead of a HashMap.
    // Since we bin colors into 32-unit buckets (8 levels per channel), there are
    // only 8 * 8 * 8 = 512 possible buckets. This avoids heap allocations and hashing overhead.
    let mut buckets = [0u32; 512];

    for y in (0..height).step_by(step_y as usize) {
        for x in (0..width).step_by(step_x as usize) {
            let Rgba([r, g, b, a]) = image.get_pixel(x, y);
            if a < 128 {
                continue;
            }

            // Map RGB to 512 buckets: 3 bits per channel (8 levels each)
            let r_idx = (r / 32) as usize;
            let g_idx = (g / 32) as usize;
            let b_idx = (b / 32) as usize;
            let index = (r_idx << 6) | (g_idx << 3) | b_idx;
            buckets[index] += 1;
        }
    }

    // Convert buckets to a vector for sorting, filtering out empty buckets and invalid brightness.
    // We filter before sorting to reduce the number of elements handled by the sort algorithm.
    let mut sorted: Vec<((u8, u8, u8), u32)> = Vec::with_capacity(512);
    for (index, &count) in buckets.iter().enumerate() {
        if count == 0 {
            continue;
        }

        let r = ((index >> 6) & 0x07) as u8 * 32;
        let g = ((index >> 3) & 0x07) as u8 * 32;
        let b = (index & 0x07) as u8 * 32;

        let brightness = (r as u32 + g as u32 + b as u32) / 3;
        if brightness > 30 && brightness < 220 {
            sorted.push(((r, g, b), count));
        }
    }

    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    sorted
        .iter()
        .take(5)
        .map(|((r, g, b), _)| [*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0])
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

pub fn lerp_colour(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

pub fn time_to_sky_colour(time: f32) -> [f32; 3] {
    const MIDNIGHT: [f32; 3] = [0.02, 0.02, 0.08];
    const DAWN: [f32; 3] = [0.6, 0.3, 0.2];
    const NOON: [f32; 3] = [0.4, 0.6, 0.9];
    const DUSK: [f32; 3] = [0.7, 0.3, 0.15];

    match time {
        // Optimization: Replace divisions with multiplications by reciprocal (1.0 / 0.25 = 4.0)
        t if t < 0.25 => lerp_colour(MIDNIGHT, DAWN, t * 4.0),
        t if t < 0.5 => lerp_colour(DAWN, NOON, (t - 0.25) * 4.0),
        t if t < 0.75 => lerp_colour(NOON, DUSK, (t - 0.5) * 4.0),
        t => lerp_colour(DUSK, MIDNIGHT, (t - 0.75) * 4.0),
    }
}

#[cfg(test)]
mod tests;
