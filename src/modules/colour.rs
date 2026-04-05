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
        } else if count > 10 {
            // Retain extreme colors (black/white) with a severe penalty (1/100th weight).
            // This ensures monochromatic albums still extract a valid background palette!
            sorted.push(((r, g, b), (count / 100).max(1)));
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

pub fn relative_luminance(c: [f32; 3]) -> f32 {
    let f = |x: f32| -> f32 {
        if x <= 0.03928 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
}

pub fn contrast_ratio(l1: f32, l2: f32) -> f32 {
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

pub fn ensure_contrast(fg: [f32; 3], bg: [f32; 3], target_ratio: f32) -> [f32; 3] {
    ensure_contrast_blended(fg, bg, 1.0, target_ratio)
}

pub fn ensure_contrast_blended(
    fg: [f32; 3],
    bg: [f32; 3],
    alpha: f32,
    target_ratio: f32,
) -> [f32; 3] {
    let l_bg = relative_luminance(bg);
    let blended_fg = lerp_colour(bg, fg, alpha);
    let l_fg = relative_luminance(blended_fg);

    if contrast_ratio(l_fg, l_bg) >= target_ratio {
        return fg;
    }

    let cr_white = contrast_ratio(
        relative_luminance(lerp_colour(bg, [1.0, 1.0, 1.0], alpha)),
        l_bg,
    );
    let cr_black = contrast_ratio(
        relative_luminance(lerp_colour(bg, [0.0, 0.0, 0.0], alpha)),
        l_bg,
    );

    let mix_target = if cr_white > cr_black {
        [1.0, 1.0, 1.0]
    } else {
        [0.0, 0.0, 0.0]
    };

    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..10 {
        let mid = (low + high) / 2.0;
        let opaque_candidate = lerp_colour(fg, mix_target, mid);
        let blended = lerp_colour(bg, opaque_candidate, alpha);
        let cr = contrast_ratio(relative_luminance(blended), l_bg);
        if cr >= target_ratio {
            high = mid;
        } else {
            low = mid;
        }
    }

    lerp_colour(fg, mix_target, high)
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

    fn approx_eq_f32(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn test_relative_luminance() {
        // Black
        assert!(approx_eq_f32(
            relative_luminance([0.0, 0.0, 0.0]),
            0.0
        ));
        // White
        assert!(approx_eq_f32(
            relative_luminance([1.0, 1.0, 1.0]),
            1.0
        ));

        // Pure colors
        assert!(approx_eq_f32(
            relative_luminance([1.0, 0.0, 0.0]),
            0.2126
        ));
        assert!(approx_eq_f32(
            relative_luminance([0.0, 1.0, 0.0]),
            0.7152
        ));
        assert!(approx_eq_f32(
            relative_luminance([0.0, 0.0, 1.0]),
            0.0722
        ));

        // Low threshold (< 0.03928)
        let low = 0.03;
        assert!(approx_eq_f32(
            relative_luminance([low, low, low]),
            low / 12.92
        ));

        // High threshold (>= 0.03928)
        let high = 0.5;
        let expected_high = ((high + 0.055) / 1.055_f32).powf(2.4);
        assert!(approx_eq_f32(
            relative_luminance([high, high, high]),
            expected_high
        ));
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

        // We expect 3 colors since bright is retained with a severe penalty, and transparent is filtered out
        assert_eq!(palette.len(), 3);

        // The background color should be the most common
        let expected_bg = [96.0 / 255.0, 96.0 / 255.0, 96.0 / 255.0];
        assert!(approx_eq(palette[0], expected_bg));

        // The foreground color should be the second most common
        let expected_fg = [192.0 / 255.0, 96.0 / 255.0, 96.0 / 255.0];
        assert!(approx_eq(palette[1], expected_fg));
    }

    #[test]
    fn test_extract_palette_top_5() {
        let mut img = image::RgbaImage::new(128, 128);

        // Define 6 valid colors that map exactly to distinct bins
        // Color A (largest area)
        let color_a = image::Rgba([64, 64, 64, 255]); // bin: (64, 64, 64)
                                                      // Color B
        let color_b = image::Rgba([96, 64, 64, 255]); // bin: (96, 64, 64)
                                                      // Color C
        let color_c = image::Rgba([64, 96, 64, 255]); // bin: (64, 96, 64)
                                                      // Color D
        let color_d = image::Rgba([64, 64, 96, 255]); // bin: (64, 64, 96)
                                                      // Color E
        let color_e = image::Rgba([96, 96, 64, 255]); // bin: (96, 96, 64)
                                                      // Color F (smallest area)
        let color_f = image::Rgba([64, 96, 96, 255]); // bin: (64, 96, 96)

        for y in 0..128 {
            for x in 0..128 {
                let color = if y < 64 {
                    color_a // 64 rows
                } else if y < 96 {
                    color_b // 32 rows
                } else if y < 112 {
                    color_c // 16 rows
                } else if y < 120 {
                    color_d // 8 rows
                } else if y < 126 {
                    color_e // 6 rows
                } else {
                    color_f // 2 rows
                };
                img.put_pixel(x, y, color);
            }
        }

        let dyn_img = image::DynamicImage::ImageRgba8(img);
        let palette = extract_palette(&dyn_img);

        // Should return exactly 5 colors (ignoring F)
        assert_eq!(palette.len(), 5);

        let expected_a = [64.0 / 255.0, 64.0 / 255.0, 64.0 / 255.0];
        let expected_b = [96.0 / 255.0, 64.0 / 255.0, 64.0 / 255.0];
        let expected_c = [64.0 / 255.0, 96.0 / 255.0, 64.0 / 255.0];
        let expected_d = [64.0 / 255.0, 64.0 / 255.0, 96.0 / 255.0];
        let expected_e = [96.0 / 255.0, 96.0 / 255.0, 64.0 / 255.0];

        assert!(approx_eq(palette[0], expected_a));
        assert!(approx_eq(palette[1], expected_b));
        assert!(approx_eq(palette[2], expected_c));
        assert!(approx_eq(palette[3], expected_d));
        assert!(approx_eq(palette[4], expected_e));
    }

    #[test]
    fn test_extract_palette_no_valid_colors() {
        let mut img = image::RgbaImage::new(64, 64);

        // Add pure black (too dark: 0, 0, 0 -> brightness 0 <= 30)
        let dark_color = image::Rgba([0, 0, 0, 255]);
        for y in 0..32 {
            for x in 0..64 {
                img.put_pixel(x, y, dark_color);
            }
        }

        // Add pure white (too bright: 255, 255, 255 -> brightness 255 >= 220)
        let bright_color = image::Rgba([255, 255, 255, 255]);
        for y in 32..64 {
            for x in 0..64 {
                img.put_pixel(x, y, bright_color);
            }
        }

        // Remaining implicit pixels are transparent [0,0,0,0], alpha < 128

        let dyn_img = image::DynamicImage::ImageRgba8(img);
        let palette = extract_palette(&dyn_img);

        // We expect 2 colors since bright and dark are retained with severe penalties
        assert_eq!(palette.len(), 2);
    }
}
