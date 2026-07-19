use image::{DynamicImage, GenericImageView, Rgba};

pub fn extract_palette(image: &DynamicImage) -> Box<[[f32; 3]]> {
    // Optimization: Skip expensive Lanczos3 filtering and extra allocations
    // by sampling the original image directly. For a coarse 512-bucket histogram,
    // simple nearest-neighbor sampling is significantly faster and more than sufficient.
    let (width, height) = image.dimensions();
    let step_x = (width / 64).max(1);
    let step_y = (height / 64).max(1);

    // Optimization: Avoid dynamic/enum dispatch overhead of `DynamicImage::get_pixel`
    // in the hot loop by extracting a direct reference or copy of the underlying `RgbaImage` once.
    let rgba_image_storage;
    let rgba_ref = if let Some(ref_img) = image.as_rgba8() {
        ref_img
    } else {
        rgba_image_storage = image.to_rgba8();
        &rgba_image_storage
    };

    // Optimization: Use a fixed-size array for the histogram instead of a HashMap.
    // Since we bin colors into 32-unit buckets (8 levels per channel), there are
    // only 8 * 8 * 8 = 512 possible buckets. This avoids heap allocations and hashing overhead.
    let mut buckets = [0u32; 512];

    for y in (0..height).step_by(step_y as usize) {
        for x in (0..width).step_by(step_x as usize) {
            let Rgba([r, g, b, a]) = *rgba_ref.get_pixel(x, y);
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

    // Optimization: Use unstable sort for a small performance boost when relative order doesn't matter.
    sorted.sort_unstable_by_key(|b| std::cmp::Reverse(b.1));

    sorted
        .iter()
        .take(5)
        .map(|((r, g, b), _)| [*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0])
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

/// Area-weighted mean colour of an image, for judging what text drawn over
/// it must contrast against. Unlike [`extract_palette`] this keeps very dark
/// and very bright pixels: a mostly-black or mostly-white wallpaper should
/// pull the mean toward black or white, not toward its accent colours.
pub fn average_colour(img: &image::RgbaImage) -> [f32; 3] {
    let (width, height) = img.dimensions();
    let step_x = (width / 64).max(1);
    let step_y = (height / 64).max(1);

    let mut sum = [0.0f64; 3];
    let mut count = 0u32;
    for y in (0..height).step_by(step_y as usize) {
        for x in (0..width).step_by(step_x as usize) {
            let Rgba([r, g, b, a]) = *img.get_pixel(x, y);
            if a < 128 {
                continue;
            }
            sum[0] += r as f64;
            sum[1] += g as f64;
            sum[2] += b as f64;
            count += 1;
        }
    }
    if count == 0 {
        return [0.1, 0.1, 0.1];
    }
    let n = count as f64 * 255.0;
    [
        (sum[0] / n) as f32,
        (sum[1] / n) as f32,
        (sum[2] / n) as f32,
    ]
}

pub fn lerp_colour(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

pub fn time_to_sky_colour(time: f32) -> [f32; 3] {
    let midnight = [0.02, 0.02, 0.08];
    let dawn = [0.6, 0.3, 0.2];
    let noon = [0.4, 0.6, 0.9];
    let dusk = [0.7, 0.3, 0.15];

    match time {
        t if t < 0.25 => lerp_colour(midnight, dawn, t / 0.25),
        t if t < 0.5 => lerp_colour(dawn, noon, (t - 0.25) / 0.25),
        t if t < 0.75 => lerp_colour(noon, dusk, (t - 0.5) / 0.25),
        t => lerp_colour(dusk, midnight, (t - 0.75) / 0.25),
    }
}

/// WCAG relative luminance of an sRGB color (components 0.0-1.0).
pub fn relative_luminance(c: [f32; 3]) -> f32 {
    fn lin(u: f32) -> f32 {
        if u <= 0.04045 {
            u / 12.92
        } else {
            ((u + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// WCAG contrast ratio between two sRGB colors, in the range 1.0..=21.0.
pub fn contrast_ratio(a: [f32; 3], b: [f32; 3]) -> f32 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (l1, l2) = (la.max(lb), la.min(lb));
    (l1 + 0.05) / (l2 + 0.05)
}

/// Returns `text` nudged toward black or white (whichever contrasts better
/// with `bg`) until it reaches `min_ratio` against `bg`, preserving hue for
/// as long as possible. Returns `text` unchanged if it already passes.
pub fn ensure_contrast(text: [f32; 3], bg: [f32; 3], min_ratio: f32) -> [f32; 3] {
    if contrast_ratio(text, bg) >= min_ratio {
        return text;
    }
    // 0.179 is the background luminance at which black and white text give
    // equal contrast (WCAG's own crossover point).
    let target = if relative_luminance(bg) > 0.179 {
        [0.0; 3]
    } else {
        [1.0; 3]
    };
    // 16 fixed steps is plenty of resolution for a wallpaper text color;
    // binary search would be overkill for a range this small.
    for i in 1..=16 {
        let t = i as f32 / 16.0;
        let candidate = lerp_colour(text, target, t);
        if contrast_ratio(candidate, bg) >= min_ratio {
            return candidate;
        }
    }
    target
}

#[cfg(test)]
mod tests;
