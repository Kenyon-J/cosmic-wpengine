use image::{DynamicImage, GenericImageView, Rgba};

pub fn extract_palette(image: &DynamicImage) -> Vec<[f32; 3]> {
    let thumb = image.thumbnail(64, 64);

    let mut buckets: std::collections::HashMap<(u8, u8, u8), u32> =
        std::collections::HashMap::new();

    for (_, _, Rgba([r, g, b, a])) in thumb.pixels() {
        if a < 128 {
            continue;
        }

        let key = ((r / 32) * 32, (g / 32) * 32, (b / 32) * 32);
        *buckets.entry(key).or_insert(0) += 1;
    }

    let mut sorted: Vec<_> = buckets.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    sorted
        .iter()
        .filter(|((r, g, b), _)| {
            let brightness = (*r as u32 + *g as u32 + *b as u32) / 3;
            brightness > 30 && brightness < 220
        })
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
