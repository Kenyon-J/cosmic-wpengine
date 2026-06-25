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

pub(crate) fn get_uv_transform(mode: u32, screen_aspect: f32, image_aspect: f32) -> [f32; 4] {
    let new_aspect = screen_aspect / image_aspect;

    let mut scale_x = 1.0;
    let mut scale_y = 1.0;
    let mut offset_x = 0.0;
    let mut offset_y = 0.0;

    if mode == 0 || mode == 2 {
        // object-fit: cover
        if new_aspect > 1.0 {
            scale_x = 1.0 / new_aspect;
            offset_x = (1.0 - scale_x) / 2.0;
        } else {
            scale_y = new_aspect;
            offset_y = (1.0 - scale_y) / 2.0;
        }
    } else if mode == 1 {
        // object-fit: contain
        if new_aspect > 1.0 {
            scale_x = new_aspect;
            offset_x = (1.0 - scale_x) / 2.0;
        } else {
            scale_y = 1.0 / new_aspect;
            offset_y = (1.0 - scale_y) / 2.0;
        }
    }

    [scale_x, scale_y, offset_x, offset_y]
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
}
