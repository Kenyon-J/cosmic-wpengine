use std::time::Instant;

fn main() {
    let bands = vec![0.5f32; 2048];
    let mut audio_bands = vec![0.0f32; 128];
    let config_visualiser_band_count = 64usize;
    let bands_per_bar = (bands.len() as f32 / config_visualiser_band_count as f32).max(1.0);

    let start = Instant::now();
    for _ in 0..100_000 {
        // Original logic for audio bounds
        for (i, current) in audio_bands.iter_mut().enumerate() {
            let bin_lo = (i as f32 * bands_per_bar) as usize;
            let bin_hi = ((i + 1) as f32 * bands_per_bar) as usize;
            let mut max_val: f32 = 0.0;
            // The original uses bin_lo.min() ? Wait, let's check the issue text again.
            // Original: for &val in &bands[bin_lo..bin_hi.min(bands.len())]
            let limit = bands.len();
            let safe_lo = bin_lo.min(limit);
            let safe_hi = bin_hi.min(limit);
            for &val in &bands[safe_lo..safe_hi] {
                max_val = max_val.max(val);
            }
            *current = max_val;
        }
    }
    println!("Original inline bounds (fixed panic): {:?}", start.elapsed());

    let mut audio_bands = vec![0.0f32; 128];
    let start = Instant::now();
    for _ in 0..100_000 {
        // Optimized logic
        let bands_len = bands.len();
        for (i, current) in audio_bands.iter_mut().enumerate() {
            let bin_lo = (i as f32 * bands_per_bar) as usize;
            let bin_hi = ((i + 1) as f32 * bands_per_bar) as usize;

            // `get` with safe range handles if bin_lo > bands_len by returning None
            let max_val = bands.get(bin_lo..bin_hi.min(bands_len)).map_or(0.0, |slice| {
                slice.iter().fold(0.0f32, |acc, &val| if val > acc { val } else { acc })
            });
            *current = max_val;
        }
    }
    println!("Optimized map_or iter fold inline: {:?}", start.elapsed());
}
