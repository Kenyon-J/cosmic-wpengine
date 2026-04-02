use std::time::Instant;

fn main() {
    let bands = vec![0.5f32; 2048];
    let mut audio_bands = vec![0.0f32; 128];
    let mut frequency_bin_ranges = vec![(0, 0); 128];
    for (i, range) in frequency_bin_ranges.iter_mut().enumerate() {
        *range = (i * 10, (i + 1) * 10 + 5);
    }
    let a_weighting_curve = vec![1.0f32; 128];
    let smoothing = 0.5f32;

    let start = Instant::now();
    for _ in 0..100_000 {
        // Original
        for (i, current) in audio_bands.iter_mut().enumerate() {
            let (bin_lo, bin_hi) = frequency_bin_ranges[i];

            let mut max_val = 0.0f32;
            for &val in &bands[bin_lo..bin_hi.min(bands.len())] {
                if val > max_val {
                    max_val = val;
                }
            }

            let a_weighting_norm = a_weighting_curve[i];
            let target = (max_val * a_weighting_norm * 2.5).clamp(0.0, 1.0);

            if target > *current {
                *current = *current * 0.2 + target * 0.8;
            } else {
                *current = *current * smoothing + target * (1.0 - smoothing);
            }
        }
    }
    println!("Original: {:?}", start.elapsed());

    let mut audio_bands = vec![0.0f32; 128];
    let start = Instant::now();
    for _ in 0..100_000 {
        // Optimized
        let bands_len = bands.len();
        for (i, current) in audio_bands.iter_mut().enumerate() {
            let (bin_lo, bin_hi) = frequency_bin_ranges[i];
            let safe_start = bin_lo.min(bands_len);
            let safe_end = bin_hi.min(bands_len);

            let max_val = bands[safe_start..safe_end]
                .iter()
                .fold(0.0f32, |acc, &val| acc.max(val));

            let a_weighting_norm = a_weighting_curve[i];
            let target = (max_val * a_weighting_norm * 2.5).clamp(0.0, 1.0);

            if target > *current {
                *current = *current * 0.2 + target * 0.8;
            } else {
                *current = *current * smoothing + target * (1.0 - smoothing);
            }
        }
    }
    println!("Optimized: {:?}", start.elapsed());
}
