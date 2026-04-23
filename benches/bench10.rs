use std::time::Instant;

fn main() {
    let bands = vec![0.5f32; 2048];
    let waveform = vec![0.5f32; 2048];
    let mut audio_bands = vec![0.0f32; 128];
    let mut audio_waveform = vec![0.0f32; 128];

    let mut frequency_bin_ranges = vec![(0, 0); 128];
    let mut waveform_bin_ranges = vec![(0, 0); 128];
    for (i, item) in frequency_bin_ranges.iter_mut().enumerate().take(128) {
         *item = (i * 10, (i + 1) * 10 + 5);
        waveform_bin_ranges[i] = (i * 10, (i + 1) * 10 + 5);
    }
    let a_weighting_curve = vec![1.0f32; 128];
    let smoothing = 0.5f32;

    let start = Instant::now();
    for _ in 0..100_000 {
        // Original logic for audio bounds and waveform
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

        for (i, current) in audio_waveform.iter_mut().enumerate() {
            let (start, end) = waveform_bin_ranges[i];

            let mut peak = 0.0f32;
            for &val in &waveform[start..end.min(waveform.len())] {
                if val.abs() > peak.abs() {
                    peak = val;
                }
            }

            *current = *current * smoothing + peak * (1.0 - smoothing);
        }
    }
    println!("Original: {:?}", start.elapsed());

    let mut audio_bands = vec![0.0f32; 128];
    let mut audio_waveform = vec![0.0f32; 128];
    let start = Instant::now();
    for _ in 0..100_000 {
        // Optimized logic
        let bands_len = bands.len();
        for (i, current) in audio_bands.iter_mut().enumerate() {
            let (bin_lo, bin_hi) = frequency_bin_ranges[i];

            let max_val = bands
                .get(bin_lo..bin_hi.min(bands_len))
                .map_or(0.0, |slice| {
                    slice.iter().fold(0.0f32, |acc, &val| acc.max(val))
                });

            let a_weighting_norm = a_weighting_curve[i];
            let target = (max_val * a_weighting_norm * 2.5).clamp(0.0, 1.0);

            if target > *current {
                *current = *current * 0.2 + target * 0.8;
            } else {
                *current = *current * smoothing + target * (1.0 - smoothing);
            }
        }

        let wave_len = waveform.len();
        for (i, current) in audio_waveform.iter_mut().enumerate() {
            let (start, end) = waveform_bin_ranges[i];

            let peak = waveform.get(start..end.min(wave_len)).map_or(0.0, |slice| {
                slice.iter().fold(0.0f32, |acc, &val| {
                    let val_abs = val.abs();
                    if val_abs > acc.abs() {
                        val
                    } else {
                        acc
                    }
                })
            });

            *current = *current * smoothing + peak * (1.0 - smoothing);
        }
    }
    println!("Optimized 6 (acc.max(val)): {:?}", start.elapsed());
}
