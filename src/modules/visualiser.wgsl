struct VisualiserUniforms {
    resolution: vec2<f32>,
    band_count: u32,
    lyric_pulse: f32,
    color_top: vec4<f32>,
    color_bottom: vec4<f32>,
    style: u32,
    size: f32,
    position: vec2<f32>,
    rotation: f32,
    amplitude: f32,
    pad1: u32,
    pad2: u32,
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

    if uniforms.style == 0u {
        // --- Circular Style ---
        let p = vec2<f32>((uv.x - uniforms.position.x) * aspect, uv.y - uniforms.position.y);
        
        let s = sin(uniforms.rotation);
        let c = cos(uniforms.rotation);
        let p_rot = vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
        let angle = atan2(p_rot.y, p_rot.x) + 3.14159; 
        
        let d = length(p_rot) - uniforms.size;
        if d < 0.0 {
            return vec4<f32>(0.0);
        }

        let normalized_angle = angle / 6.28318;
        var f_band = normalized_angle * 2.0;
        if f_band > 1.0 { f_band = 2.0 - f_band; }

        f_band = f_band * f32(uniforms.band_count);
        let band_idx = u32(f_band);
        let within_bar = fract(f_band);
        if within_bar > 0.85 { return vec4<f32>(0.0); }

        let band_val = bands[band_idx];
        let max_dist = max(band_val, 0.02) * 0.25 * uniforms.amplitude + (uniforms.lyric_pulse * 0.05);

        if d <= max_dist {
            let t = d / max_dist;
            let bar_color = mix(uniforms.color_bottom.rgb, uniforms.color_top.rgb, t);
            return vec4<f32>(bar_color, 1.0);
        }

        let glow_dist = d - max_dist;
        let glow = clamp(0.005 / (glow_dist * glow_dist + 0.005) - 0.1, 0.0, 1.0) * (1.0 + uniforms.lyric_pulse * 2.0);
        return vec4<f32>(uniforms.color_top.rgb * glow, min(glow, 1.0));
    } else {
        // --- Linear Style (Monstercat) ---
        let s = sin(uniforms.rotation);
        let c = cos(uniforms.rotation);
        let shifted = uv - uniforms.position;
        
        // Adjust aspect ratio only for the rotation matrix to keep horizontal bands evenly sized
        let p_rot = vec2<f32>(
            (shifted.x * aspect * c - shifted.y * s) / aspect,
            shifted.x * aspect * s + shifted.y * c
        );
        
        let local_uv = p_rot + uniforms.position;
        let hw = uniforms.size / 2.0; 
        if local_uv.x < uniforms.position.x - hw || local_uv.x > uniforms.position.x + hw {
            return vec4<f32>(0.0);
        }

        let normalized_x = (local_uv.x - (uniforms.position.x - hw)) / uniforms.size;
        let band_idx = u32(normalized_x * f32(uniforms.band_count));
        let within_bar = fract(normalized_x * f32(uniforms.band_count));
        if within_bar > 0.85 { return vec4<f32>(0.0); }

        let val = bands[band_idx];
        let max_h = max(val, 0.02) * 0.25 * uniforms.amplitude + (uniforms.lyric_pulse * 0.05);

        let h = uniforms.position.y - local_uv.y;
        if h < 0.0 { return vec4<f32>(0.0); }

        if h <= max_h {
            let bar_color = mix(uniforms.color_bottom.rgb, uniforms.color_top.rgb, h / max_h);
            return vec4<f32>(bar_color, 0.95);
        }

        let glow_dist = h - max_h;
        let glow = clamp(0.005 / (glow_dist * glow_dist * 10.0 + 0.005) - 0.1, 0.0, 1.0) * (1.0 + uniforms.lyric_pulse);
        return vec4<f32>(uniforms.color_top.rgb * glow, min(glow, 1.0));
    }
}