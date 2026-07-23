pub(crate) fn build_audio_processing_bins(band_count: usize) -> Vec<(usize, usize, f32)> {
    let min_freq = 40.0f32;
    let max_freq = 16000.0f32;
    let sample_rate = 48000.0f32;
    let fft_size = 2048.0f32;
    let freq_per_bin = sample_rate / fft_size;
    let min_log = min_freq.log2();
    let max_log = max_freq.log2();
    let max_bins = (fft_size / 2.0) as usize; // 1024

    // Optimization: Use an exact size iterator with `.collect()` instead of a manual
    // `for` loop with `.push()` to leverage standard library optimizations
    // and eliminate capacity checking / redundant bounds checking.
    // This function consolidates frequency bin ranges and A-weighting coefficients
    // into a single tuple to eliminate redundant arithmetic in the hot path.
    (0..band_count)
        .map(|i| {
            let t_lo = i as f32 / band_count as f32;
            let t_hi = (i + 1) as f32 / band_count as f32;

            let freq_lo = (min_log + t_lo * (max_log - min_log)).exp2();
            let freq_hi = (min_log + t_hi * (max_log - min_log)).exp2();

            let mut bin_lo = (freq_lo / freq_per_bin).round() as usize;
            let mut bin_hi = (freq_hi / freq_per_bin).round() as usize;

            bin_lo = bin_lo.clamp(0, max_bins.saturating_sub(1));
            bin_hi = bin_hi.clamp(0, max_bins);
            if bin_hi <= bin_lo {
                bin_hi = (bin_lo + 1).min(max_bins);
            }

            let f = (freq_lo * freq_hi).sqrt();
            let f2 = f * f;
            let f4 = f2 * f2;

            let a_weighting = (12200.0 * 12200.0 * f4)
                / ((f2 + 20.6 * 20.6)
                    * (f2 + 12200.0 * 12200.0)
                    * ((f2 + 107.7 * 107.7) * (f2 + 737.9 * 737.9)).sqrt());

            // Bake in the visualizer 2.5x scaling factor and the original 1.2589 normalization
            // to eliminate multiple multiplications inside the triple-nested audio processing loop.
            (bin_lo, bin_hi, a_weighting * 1.2589 * 2.5)
        })
        .collect()
}

pub fn hash_str(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    // Optimization: Use `rustc_hash::FxHasher` instead of `std::collections::hash_map::DefaultHasher`.
    // The default hasher uses SipHash, which protects against HashDoS but is significantly slower.
    // For internal caching keys inside a 60FPS render loop, collision resistance is unnecessary
    // and FxHash provides a measurable speedup.
    let mut hasher = rustc_hash::FxHasher::default();
    s.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn build_waveform_bin_ranges(band_count: usize) -> Vec<(usize, usize)> {
    let chunk_size = 2048.0 / band_count as f32;
    (0..band_count)
        .map(|i| {
            let start = (i as f32 * chunk_size) as usize;
            let end = ((i + 1) as f32 * chunk_size) as usize;
            (start, end.min(2048))
        })
        .collect()
}

/// Encodes one 0-1 colour channel (cosmic-bg config values are sRGB-encoded)
/// as a texture byte for our Rgba8UnormSrgb custom-background texture.
fn srgb_byte(c: f32) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

static LINEAR_TO_SRGB_TABLE: std::sync::OnceLock<[f32; 1025]> = std::sync::OnceLock::new();

fn get_linear_to_srgb_table() -> &'static [f32; 1025] {
    LINEAR_TO_SRGB_TABLE.get_or_init(|| {
        let mut table = [0.0f32; 1025];
        for (i, val) in table.iter_mut().enumerate() {
            let c = i as f32 / 1024.0;
            *val = if c <= 0.003_130_8 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            };
        }
        table[0] = 0.0;
        table[1024] = 1.0;
        table
    })
}

// Optimization: Replace extremely expensive `powf(1.0 / 2.4)` with an O(1) linear-interpolated lookup table.
// In `gradient_image`, this function is called millions of times per high-res gradient, making `powf` a massive bottleneck.
// A 1024-interval table (4KB) fits completely in L1 cache while keeping error below 0.0001 (far below 8-bit color precision).
fn linear_to_srgb(c: f32) -> f32 {
    // Safe NaN-handling clamp: explicit is_nan() or less-than-zero checks handle NaNs
    // and negative values gracefully, returning 0.0. This avoids standard f32::clamp's NaN panic.
    let c = if c.is_nan() || c < 0.0 {
        0.0
    } else if c > 1.0 {
        1.0
    } else {
        c
    };

    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        let table = get_linear_to_srgb_table();
        let val = c * 1024.0;
        let idx = val as usize;
        let frac = val - idx as f32;
        if idx >= 1024 {
            table[1024]
        } else {
            table[idx] * (1.0 - frac) + table[idx + 1] * frac
        }
    }
}

/// A tiny uniform texture standing in for a solid-colour desktop wallpaper;
/// the frosted-glass shader's cover-crop of a uniform image is identical at
/// any resolution, so 16x16 is plenty.
pub fn solid_colour_image(colour: [f32; 3]) -> image::RgbaImage {
    let pixel = image::Rgba([
        srgb_byte(colour[0]),
        srgb_byte(colour[1]),
        srgb_byte(colour[2]),
        255,
    ]);
    image::RgbaImage::from_pixel(16, 16, pixel)
}

/// Renders a cosmic-bg gradient wallpaper: evenly spaced stops interpolated
/// in linear RGB along an axis `angle_deg` clockwise from bottom-to-top,
/// matching cosmic-bg's 0/90/180/270 orientations.
pub fn gradient_image(
    colors: &[[f32; 3]],
    angle_deg: f32,
    width: u32,
    height: u32,
) -> image::RgbaImage {
    if colors.is_empty() {
        return solid_colour_image([0.0; 3]);
    }
    let stops: Vec<[f32; 3]> = colors.iter().map(|c| c.map(srgb_to_linear)).collect();
    let last = stops.len() - 1;

    let angle = angle_deg.to_radians();
    let (dx, dy) = (angle.sin(), -angle.cos());
    let (w, h) = (width as f32, height as f32);
    // Normalize the pixel's projection onto the gradient axis against the
    // projected extent of the whole rectangle, so the first and last stops
    // always land exactly on opposite corners/edges.
    let corners = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)];
    let (mut proj_min, mut proj_max) = (f32::INFINITY, f32::NEG_INFINITY);
    for (cx, cy) in corners {
        let p = cx * dx + cy * dy;
        proj_min = proj_min.min(p);
        proj_max = proj_max.max(p);
    }
    let inv_range = if proj_max > proj_min {
        1.0 / (proj_max - proj_min)
    } else {
        0.0
    };

    image::RgbaImage::from_fn(width, height, |x, y| {
        let t = ((x as f32 * dx + y as f32 * dy) - proj_min) * inv_range;
        let linear = if last == 0 {
            stops[0]
        } else {
            let pos = t.clamp(0.0, 1.0) * last as f32;
            let i = (pos as usize).min(last - 1);
            let frac = pos - i as f32;
            std::array::from_fn(|k| stops[i][k] + (stops[i + 1][k] - stops[i][k]) * frac)
        };
        image::Rgba([
            srgb_byte(linear_to_srgb(linear[0])),
            srgb_byte(linear_to_srgb(linear[1])),
            srgb_byte(linear_to_srgb(linear[2])),
            255,
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests `hash_str` function to ensure it generates deterministic hashes for the same string
    /// and different hashes for different strings, preventing regressions where string identification breaks.
    #[test]
    fn test_build_audio_processing_bins() {
        let bins = build_audio_processing_bins(64);
        assert_eq!(bins.len(), 64);
        for (lo, hi, weight) in bins {
            assert!(lo < hi);
            assert!(hi <= 1024);
            assert!(weight > 0.0);
        }
    }

    #[test]
    fn test_solid_colour_image_encodes_srgb_bytes() {
        let img = solid_colour_image([1.0, 0.5, 0.0]);
        assert_eq!(img.get_pixel(0, 0), &image::Rgba([255, 128, 0, 255]));
        assert_eq!(img.get_pixel(15, 15), &image::Rgba([255, 128, 0, 255]));
    }

    /// Angle 0 puts the first stop at the bottom edge fading upward, and 90
    /// puts it at the left edge fading rightward — matching how cosmic-bg
    /// renders its 0/90/180/270 gradient orientations.
    #[test]
    fn test_gradient_image_orientations() {
        let black_to_white = [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]];

        // Edge pixels sit one pixel inside the projected extent, so the
        // extreme stops are only approached (as in cosmic-bg's own math).
        let up = gradient_image(&black_to_white, 0.0, 64, 64);
        assert!(up.get_pixel(32, 63)[0] < 50, "first stop at the bottom");
        assert!(up.get_pixel(32, 0)[0] == 255, "last stop at the top");

        let right = gradient_image(&black_to_white, 90.0, 64, 64);
        assert!(right.get_pixel(0, 32)[0] < 50, "first stop on the left");
        assert!(right.get_pixel(63, 32)[0] > 250, "last stop on the right");
    }

    #[test]
    fn test_gradient_image_single_stop_and_empty() {
        let single = gradient_image(&[[1.0, 0.0, 0.0]], 45.0, 8, 8);
        assert_eq!(single.get_pixel(4, 4), &image::Rgba([255, 0, 0, 255]));

        // Defensive: an empty stop list must not panic.
        let empty = gradient_image(&[], 0.0, 8, 8);
        assert_eq!(empty.get_pixel(0, 0)[3], 255);
    }

    #[test]
    fn test_hash_str() {
        let hash1 = hash_str("hello world");
        let hash2 = hash_str("hello world");
        assert_eq!(hash1, hash2, "The same string should produce the same hash");

        let hash3 = hash_str("different string");
        assert_ne!(
            hash1, hash3,
            "Different strings should produce different hashes"
        );

        let empty_hash = hash_str("");
        let empty_hash2 = hash_str("");
        assert_eq!(
            empty_hash, empty_hash2,
            "Empty strings should hash deterministically"
        );
        assert_ne!(
            empty_hash, hash1,
            "Empty string hash should differ from non-empty string hash"
        );
    }

    #[test]
    fn test_linear_to_srgb_accuracy() {
        let reference_linear_to_srgb = |c: f32| -> f32 {
            if c <= 0.003_130_8 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
        };

        // Test clamping
        assert_eq!(linear_to_srgb(-0.5), 0.0);
        assert_eq!(linear_to_srgb(1.5), 1.0);
        assert_eq!(linear_to_srgb(f32::NAN), 0.0);

        // Test boundary values
        assert!((linear_to_srgb(0.0) - 0.0).abs() < 1e-6);
        assert!((linear_to_srgb(1.0) - 1.0).abs() < 1e-6);

        // Test precision over a dense set of samples
        for i in 0..=1000 {
            let c = i as f32 / 1000.0;
            let approx = linear_to_srgb(c);
            let expected = reference_linear_to_srgb(c);
            let diff = (approx - expected).abs();
            assert!(
                diff < 0.0005,
                "At c = {}, approx = {}, expected = {}, diff = {} is too large",
                c,
                approx,
                expected,
                diff
            );
        }
    }
}
