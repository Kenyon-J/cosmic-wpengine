use std::time::Instant;

fn main() {
    let bands = vec![0.5f32; 2048];
    let mut audio_bands = vec![0.0f32; 128];
    let mut frequency_bin_ranges = vec![(0, 0); 128];
    for (i, item) in frequency_bin_ranges.iter_mut().enumerate().take(128) {
         *item = (i * 10, (i + 1) * 10 + 5);
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
            let safe_end = bin_hi.min(bands_len);

            let max_val = if bin_lo < safe_end {
                bands[bin_lo..safe_end]
                    .iter()
                    .fold(0.0f32, |acc, &val| if val > acc { val } else { acc })
            } else {
                0.0
            };

            let a_weighting_norm = a_weighting_curve[i];
            let target = (max_val * a_weighting_norm * 2.5).clamp(0.0, 1.0);

            if target > *current {
                *current = *current * 0.2 + target * 0.8;
            } else {
                *current = *current * smoothing + target * (1.0 - smoothing);
            }
        }
    }
    println!("Optimized 1 (fold with if): {:?}", start.elapsed());

    let mut audio_bands = vec![0.0f32; 128];
    let start = Instant::now();
    for _ in 0..100_000 {
        // Optimized 2
        let bands_len = bands.len();
        for (i, current) in audio_bands.iter_mut().enumerate() {
            let (bin_lo, bin_hi) = frequency_bin_ranges[i];

            // let max_val = bands[bin_lo..bin_hi.min(bands_len)].iter().max_by(|a, b| a.partial_cmp(b).unwrap()).copied().unwrap_or(0.0);

            let max_val = bands
                .get(bin_lo..bin_hi.min(bands_len))
                .unwrap_or(&[])
                .iter()
                .fold(0.0f32, |acc, &val| if val > acc { val } else { acc });

            let a_weighting_norm = a_weighting_curve[i];
            let target = (max_val * a_weighting_norm * 2.5).clamp(0.0, 1.0);

            if target > *current {
                *current = *current * 0.2 + target * 0.8;
            } else {
                *current = *current * smoothing + target * (1.0 - smoothing);
            }
        }
    }
    println!("Optimized 2 (get with fold if): {:?}", start.elapsed());
}
