struct VisualiserUniforms {
    resolution: vec2<f32>,
    band_count: u32,
    lyric_pulse: f32,
    color_top: vec4<f32>,
    color_bottom: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: VisualiserUniforms;

// The read-only storage buffer containing our FFT frequency bands
@group(0) @binding(1) var<storage, read> bands: array<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // Clever trick to draw a full-screen triangle using only 3 vertices
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    
    // Output coordinates: 
    // Clip space: X goes left to right (-1 to 1), Y goes bottom to top (-1 to 1)
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    
    // UV space: X goes left to right (0 to 1), Y goes top to bottom (0 to 1)
    out.uv = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let aspect = uniforms.resolution.x / uniforms.resolution.y;

    let p = vec2<f32>((uv.x - 0.5) * aspect, uv.y - 0.5);
    let angle = atan2(p.y, p.x) + 3.14159; 
    
    let size = 0.25;
    let radius = 0.02;
    let d = length(max(abs(p) - vec2<f32>(size - radius), vec2<f32>(0.0))) - radius;

    // Mask out the inner square
    if d < 0.01 {
        return vec4<f32>(0.0);
    }

    let normalized_angle = angle / 6.28318;
    var f_band = normalized_angle * 2.0;
    if f_band > 1.0 { f_band = 2.0 - f_band; }

    f_band = f_band * f32(uniforms.band_count);
    let band_idx = u32(f_band);
    let within_bar = fract(f_band);

    if within_bar > 0.85 {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let band_val = bands[band_idx];
    let max_dist = max(band_val, 0.02) * 0.25 + (uniforms.lyric_pulse * 0.05);

    if d <= max_dist {
        let t = d / max_dist;
        let bar_color = mix(uniforms.color_bottom.rgb, uniforms.color_top.rgb, t);
        return vec4<f32>(bar_color, 1.0);
    }

    let glow_dist = d - max_dist;
    let glow = clamp(0.005 / (glow_dist * glow_dist + 0.005) - 0.1, 0.0, 1.0) * (1.0 + uniforms.lyric_pulse * 2.0);
    return vec4<f32>(uniforms.color_top.rgb * glow, min(glow, 1.0));
}