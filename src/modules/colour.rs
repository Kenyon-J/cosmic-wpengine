use image::{DynamicImage, GenericImageView, Rgba};

pub fn extract_palette(image: &DynamicImage) -> Vec<[f32; 3]> {
    let thumb = image.thumbnail(64, 64);

    // Optimization: Use a fixed-size array for the histogram instead of a HashMap.
    // Since we bin colors into 32-unit buckets (8 levels per channel), there are
    // only 8 * 8 * 8 = 512 possible buckets. This avoids heap allocations and hashing overhead.
    let mut buckets = [0u32; 512];

    for (_, _, Rgba([r, g, b, a])) in thumb.pixels() {
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
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: [f32; 3], b: [f32; 3]) -> bool {
        let eps = 1e-6;
        (a[0] - b[0]).abs() < eps && (a[1] - b[1]).abs() < eps && (a[2] - b[2]).abs() < eps
    }

    #[test]
    fn test_lerp_colour() {
        let a = [0.0, 0.0, 0.0];
        let b = [1.0, 1.0, 1.0];

        assert!(approx_eq(lerp_colour(a, b, 0.0), a));
        assert!(approx_eq(lerp_colour(a, b, 1.0), b));
        assert!(approx_eq(lerp_colour(a, b, 0.5), [0.5, 0.5, 0.5]));

        let c = [0.2, 0.4, 0.6];
        let d = [0.8, 0.6, 0.4];

        assert!(approx_eq(lerp_colour(c, d, 0.0), c));
        assert!(approx_eq(lerp_colour(c, d, 1.0), d));
        assert!(approx_eq(lerp_colour(c, d, 0.5), [0.5, 0.5, 0.5]));

        let e = [0.0, 1.0, 0.5];
        let f = [0.0, 0.5, 1.0];

        assert!(approx_eq(lerp_colour(e, f, 0.0), e));
        assert!(approx_eq(lerp_colour(e, f, 1.0), f));
        assert!(approx_eq(lerp_colour(e, f, 0.5), [0.0, 0.75, 0.75]));
    }

    #[test]
    fn test_time_to_sky_colour_boundaries() {
        let midnight = [0.02, 0.02, 0.08];
        let dawn = [0.6, 0.3, 0.2];
        let noon = [0.4, 0.6, 0.9];
        let dusk = [0.7, 0.3, 0.15];

        // Midnight (0.0)
        assert!(approx_eq(time_to_sky_colour(0.0), midnight));

        // Dawn (0.25)
        assert!(approx_eq(time_to_sky_colour(0.25), dawn));

        // Noon (0.5)
        assert!(approx_eq(time_to_sky_colour(0.5), noon));

        // Dusk (0.75)
        assert!(approx_eq(time_to_sky_colour(0.75), dusk));

        // End of day (1.0)
        assert!(approx_eq(time_to_sky_colour(1.0), midnight));
    }

    #[test]
    fn test_time_to_sky_colour_intermediate() {
        let midnight = [0.02, 0.02, 0.08];
        let dawn = [0.6, 0.3, 0.2];

        // Test a point between midnight and dawn (0.125 is half way to 0.25)
        let c = time_to_sky_colour(0.125);
        let expected = [
            (midnight[0] + dawn[0]) / 2.0,
            (midnight[1] + dawn[1]) / 2.0,
            (midnight[2] + dawn[2]) / 2.0,
        ];
        assert!(approx_eq(c, expected));
    }

    #[test]
    fn test_extract_palette() {
        let mut img = image::RgbaImage::new(128, 128);

        // Fill background with a color that passes brightness check
        // R: 100, G: 100, B: 100. Bin: (96, 96, 96). Brightness: 96 > 30 and < 220
        let bg_color = image::Rgba([100, 100, 100, 255]);
        for pixel in img.pixels_mut() {
            *pixel = bg_color;
        }

        // Add a smaller square of a different valid color
        // R: 200, G: 100, B: 100. Bin: (192, 96, 96). Brightness: (192+96+96)/3 = 128 > 30 and < 220
        let fg_color = image::Rgba([200, 100, 100, 255]);
        for y in 0..64 {
            for x in 0..64 {
                img.put_pixel(x, y, fg_color);
            }
        }

        // Add pure white (too bright: 255, 255, 255 -> brightness 255 > 220)
        let bright_color = image::Rgba([255, 255, 255, 255]);
        for y in 64..96 {
            for x in 64..96 {
                img.put_pixel(x, y, bright_color);
            }
        }

        // Add transparent pixels (a < 128) - these should be ignored
        let transparent_color = image::Rgba([150, 150, 150, 0]);
        for y in 96..128 {
            for x in 96..128 {
                img.put_pixel(x, y, transparent_color);
            }
        }

        let dyn_img = image::DynamicImage::ImageRgba8(img);
        let palette = extract_palette(&dyn_img);

        // We expect only 2 colors since bright and transparent are filtered out
        assert_eq!(palette.len(), 2);

        // The background color should be the most common
        let expected_bg = [96.0 / 255.0, 96.0 / 255.0, 96.0 / 255.0];
        assert!(approx_eq(palette[0], expected_bg));

        // The foreground color should be the second most common
        let expected_fg = [192.0 / 255.0, 96.0 / 255.0, 96.0 / 255.0];
        assert!(approx_eq(palette[1], expected_fg));
    }
}
