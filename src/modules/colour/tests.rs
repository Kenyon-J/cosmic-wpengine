#![cfg(test)]

use super::*;

fn approx_eq(a: [f32; 3], b: [f32; 3]) -> bool {
    let eps = 1e-6;
    (a[0] - b[0]).abs() < eps && (a[1] - b[1]).abs() < eps && (a[2] - b[2]).abs() < eps
}

fn fill_rect(
    img: &mut image::RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: image::Rgba<u8>,
) {
    for dy in 0..height {
        for dx in 0..width {
            img.put_pixel(x + dx, y + dy, color);
        }
    }
}

/// Tests color linear interpolation over boundaries to ensure transitions never wrap or go out of bounds.
/// This prevents visual artifacts or flashing during time-of-day sky color blending.
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

/// Tests time-of-day boundaries map accurately to exact sky gradient colors.
/// This prevents sudden daylight color flashes at midnight or incorrect noon colors.
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

/// Tests intermediate time states yield a blended color between established gradients.
/// This guarantees smooth progression of color temperatures instead of sudden snapping.
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

/// Tests that album art palette extraction correctly filters out transparent, overly bright, and overly dark colors.
/// This prevents unreadable low-contrast text and UI elements when dynamically matching album colors.
#[test]
fn test_extract_palette() {
    let mut img = image::RgbaImage::new(128, 128);

    // Fill background with a color that passes brightness check
    // R: 100, G: 100, B: 100. Bin: (96, 96, 96). Brightness: 96 > 30 and < 220
    let bg_color = image::Rgba([100, 100, 100, 255]);
    fill_rect(&mut img, 0, 0, 128, 128, bg_color);

    // Add a smaller square of a different valid color
    // R: 200, G: 100, B: 100. Bin: (192, 96, 96). Brightness: (192+96+96)/3 = 128 > 30 and < 220
    let fg_color = image::Rgba([200, 100, 100, 255]);
    fill_rect(&mut img, 0, 0, 64, 64, fg_color);

    // Add pure white (too bright: 255, 255, 255 -> brightness 255 > 220)
    let bright_color = image::Rgba([255, 255, 255, 255]);
    fill_rect(&mut img, 64, 64, 32, 32, bright_color);

    // Add transparent pixels (a < 128) - these should be ignored
    let transparent_color = image::Rgba([150, 150, 150, 0]);
    fill_rect(&mut img, 96, 96, 32, 32, transparent_color);

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

/// Tests that the palette extraction accurately prioritizes the most frequently occurring colors.
/// This ensures the dominant theme accurately reflects the primary visual colors of the image.
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

    fill_rect(&mut img, 0, 0, 128, 64, color_a); // 64 rows
    fill_rect(&mut img, 0, 64, 128, 32, color_b); // 32 rows
    fill_rect(&mut img, 0, 96, 128, 16, color_c); // 16 rows
    fill_rect(&mut img, 0, 112, 128, 8, color_d); // 8 rows
    fill_rect(&mut img, 0, 120, 128, 6, color_e); // 6 rows
    fill_rect(&mut img, 0, 126, 128, 2, color_f); // 2 rows

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

/// Tests that palette extraction gracefully returns an empty vector when no viable colors are found.
/// This prevents crashes on solid black or white placeholder album art images.
#[test]
fn test_extract_palette_no_valid_colors() {
    let mut img = image::RgbaImage::new(64, 64);

    // Add pure black (too dark: 0, 0, 0 -> brightness 0 <= 30)
    let dark_color = image::Rgba([0, 0, 0, 255]);
    fill_rect(&mut img, 0, 0, 64, 32, dark_color);

    // Add pure white (too bright: 255, 255, 255 -> brightness 255 >= 220)
    let bright_color = image::Rgba([255, 255, 255, 255]);
    fill_rect(&mut img, 0, 32, 64, 32, bright_color);

    // Remaining implicit pixels are transparent [0,0,0,0], alpha < 128

    let dyn_img = image::DynamicImage::ImageRgba8(img);
    let palette = extract_palette(&dyn_img);

    // We expect 0 colors since bright, dark and transparent are filtered out
    assert_eq!(palette.len(), 0);
}
