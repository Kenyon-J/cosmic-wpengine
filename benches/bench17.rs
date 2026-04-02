use std::time::Instant;

fn main() {
    let bands = vec![0.5f32; 2048];
    let mut audio_bands = vec![0.0f32; 128];
    let config_visualiser_band_count = 64usize;
    let bands_per_bar = (bands.len() as f32 / config_visualiser_band_count as f32).max(1.0);

    let start = Instant::now();
    for _ in 0..100_000 {
        // Original logic for audio bounds
        for (i, _current) in audio_bands.iter_mut().enumerate() {
            let bin_lo = (i as f32 * bands_per_bar) as usize;
            let bin_hi = ((i + 1) as f32 * bands_per_bar) as usize;
            let mut max_val: f32 = 0.0;
            for &val in &bands[bin_lo..bin_hi.min(bands.len())] {
                max_val = max_val.max(val);
            }
        }
    }
    println!("Original inline bounds: {:?}", start.elapsed());

    let mut audio_bands = vec![0.0f32; 128];
    let start = Instant::now();
    for _ in 0..100_000 {
        // Optimized logic
        let bands_len = bands.len();
        for (i, _current) in audio_bands.iter_mut().enumerate() {
            let bin_lo = (i as f32 * bands_per_bar) as usize;
            let bin_hi = ((i + 1) as f32 * bands_per_bar) as usize;

            let _max_val = bands
                .get(bin_lo..bin_hi.min(bands_len))
                .map_or(0.0, |slice| {
                    slice
                        .iter()
                        .fold(0.0f32, |acc, &val| if val > acc { val } else { acc })
                });
        }
    }
    println!("Optimized map_or iter fold inline: {:?}", start.elapsed());
}
