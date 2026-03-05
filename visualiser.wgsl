// =============================================================================
// shaders/visualiser.wgsl
// =============================================================================
// Audio visualiser shader — renders frequency bands as a glowing bar spectrum.
//
// The Rust side uploads the frequency band array as a uniform buffer each
// frame. The shader reads that data and decides per-pixel whether we're
// inside a bar, and how bright/coloured that bar should be.
// =============================================================================

// Maximum number of bands we support in the shader.
// Must match the value in config.rs (default: 64)
const MAX_BANDS: u32 = 64u;

struct Uniforms {
    // The frequency band amplitudes — one f32 per band, 0.0–1.0
    // Padded to vec4 as WGSL requires 16-byte aligned uniforms
    bands: array<vec4<f32>, 16>, // 16 vec4s = 64 f32 values

    band_count: u32,
    time: f32,

    // Colour of the bars — comes from the current track's palette
    bar_colour_low:  vec3<f32>,  // colour at low frequencies
    bar_colour_high: vec3<f32>,  // colour at high frequencies
}

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 4>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0),
        vec2(-1.0,  1.0), vec2(1.0,  1.0),
    );
    var uvs = array<vec2<f32>, 4>(
        vec2(0.0, 1.0), vec2(1.0, 1.0),
        vec2(0.0, 0.0), vec2(1.0, 0.0),
    );
    var out: VertexOutput;
    out.position = vec4(positions[idx], 0.0, 1.0);
    out.uv = uvs[idx];
    return out;
}

// Helper: get a band value by index, unpacking from vec4 array
fn get_band(index: u32) -> f32 {
    let vec_idx = index / 4u;
    let component = index % 4u;
    let v = u.bands[vec_idx];
    if component == 0u { return v.x; }
    if component == 1u { return v.y; }
    if component == 2u { return v.z; }
    return v.w;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv; // 0,0 = bottom-left, 1,1 = top-right

    let band_count = u.band_count;
    let band_width = 1.0 / f32(band_count);

    // Which band does this pixel's X position fall into?
    let band_index = u32(uv.x / band_width);
    let band_amplitude = get_band(min(band_index, band_count - 1u));

    // Gap between bars — 15% of bar width
    let bar_fill = 0.85;
    let local_x = fract(uv.x / band_width); // 0.0–1.0 within this bar's column
    let in_bar = local_x < bar_fill;

    // Is this pixel below the bar height?
    let bar_height = band_amplitude;
    let in_height = uv.y < bar_height;

    if !in_bar || !in_height {
        // Outside the bar: render a dark background
        // Slight reflection below the bars for a nice mirror effect
        let reflection_threshold = 0.05;
        if uv.y > bar_height && uv.y < bar_height + reflection_threshold {
            let fade = 1.0 - (uv.y - bar_height) / reflection_threshold;
            let t = f32(band_index) / f32(band_count);
            let bar_col = mix(u.bar_colour_low, u.bar_colour_high, t);
            return vec4(bar_col * 0.15 * fade, fade * 0.3);
        }
        return vec4(0.05, 0.05, 0.08, 1.0); // dark background
    }

    // --- Inside a bar: calculate colour ---

    // Interpolate colour from low-freq colour to high-freq colour
    let t = f32(band_index) / f32(band_count);
    var bar_colour = mix(u.bar_colour_low, u.bar_colour_high, t);

    // Brighten the top of each bar for a "peak" highlight effect
    let from_top = 1.0 - (uv.y / bar_height);
    let highlight = pow(from_top, 4.0) * 1.5;
    bar_colour = bar_colour + highlight * 0.3;

    // Add a subtle pulse tied to overall energy
    let pulse = sin(u.time * 8.0) * 0.05 * band_amplitude;
    bar_colour = bar_colour * (1.0 + pulse);

    // Glow: pixels near the bar edges get a soft bloom
    let edge_dist = min(local_x, bar_fill - local_x) / bar_fill;
    let glow = 1.0 - pow(1.0 - edge_dist, 3.0) * 0.3;
    bar_colour *= glow;

    return vec4(bar_colour, 1.0);
}
