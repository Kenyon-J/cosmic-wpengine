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

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: [f32; 3], b: [f32; 3]) -> bool {
        let eps = 1e-6;
        (a[0] - b[0]).abs() < eps && (a[1] - b[1]).abs() < eps && (a[2] - b[2]).abs() < eps
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
}
