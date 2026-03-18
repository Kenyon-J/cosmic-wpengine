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
    let y = 1.0 - uv.y; // Flip Y so 0.0 is the bottom
    
    // Calculate which band we're currently drawing
    let f_band = uv.x * f32(uniforms.band_count);
    let band_idx = u32(f_band);
    let within_bar = fract(f_band);

    // Create a gap between bars
    if (within_bar > 0.85) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let band_val = bands[band_idx];

    // Ensure a minimum height for the bars so they never completely disappear
    let height = max(band_val, 0.02) + (uniforms.lyric_pulse * 0.15); // Pulse raises the bars!

    if (y <= height) {
        // Smooth vibrant gradient colour for the bars
        let t = y / height;
        let bar_color = mix(uniforms.color_bottom.rgb, uniforms.color_top.rgb, t);
        return vec4<f32>(bar_color, 1.0);
    }

    // Add a slight atmospheric glow above the active height
    let dist = y - height;
    let glow = clamp(0.01 / (dist * dist + 0.01) - 0.1, 0.0, 1.0) * (1.0 + uniforms.lyric_pulse * 3.0); // Pulse intensifies glow
    return vec4<f32>(uniforms.color_top.rgb * glow, min(glow, 1.0));
}