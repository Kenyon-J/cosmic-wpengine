struct AmbientUniforms {
    // Packs perfectly to 32 bytes
    resolution: vec2<f32>,
    time: f32,
    weather_type: u32, // 0 = Clear, 1 = Cloudy, 2 = Rain, 3 = Snow
    sky_color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: AmbientUniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

// A fast procedural pseudo-random generator
fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // Base sky gradient (darker at the top, matching the time of day)
    var color = mix(uniforms.sky_color.rgb, uniforms.sky_color.rgb * 0.35, uv.y);
    
    // Correct aspect ratio so circular particles don't stretch
    let aspect = uniforms.resolution.x / uniforms.resolution.y;
    let st = vec2<f32>(uv.x * aspect, uv.y);
    
    if (uniforms.weather_type == 2u) {
        // --- Procedural Rain ---
        var rain_val = 0.0;
        for (var i: u32 = 0u; i < 4u; i = i + 1u) {
            let fi = f32(i);
            let uv_rain = vec2<f32>(st.x * 8.0 + fi * 11.0, st.y);
            let id_x = floor(uv_rain.x);
            
            let offset = hash12(vec2<f32>(id_x, fi));
            let speed = 3.0 + offset * 2.0;
            let y_offset = uniforms.time * speed + offset * 100.0;
            
            let p = vec2<f32>(uv_rain.x, uv_rain.y * 3.0 + y_offset);
            let id = floor(p);
            let f = fract(p);
            
            // Sparse distribution of raindrops
            if (hash12(id) > 0.75) {
                let streak = smoothstep(0.1, 0.0, abs(f.x - 0.5)) * smoothstep(0.8, 0.0, f.y);
                rain_val += streak * 0.4;
            }
        }
        color += vec3<f32>(rain_val);
        
    } else if (uniforms.weather_type == 3u) {
        // --- Procedural Snow ---
        var snow_val = 0.0;
        for (var i: u32 = 0u; i < 5u; i = i + 1u) {
            let fi = f32(i);
            let scale = 4.0 + fi * 2.0;
            let speed = 0.2 + fi * 0.1;
            
            // Add horizontal sine wave drift
            let drift = sin(uniforms.time * 0.5 + fi) * 0.3 * st.y;
            
            let p = vec2<f32>(st.x * scale + drift, st.y * scale + uniforms.time * speed);
            let id = floor(p);
            let f = fract(p);
            
            if (hash12(id) > 0.6) {
                let d = length(f - vec2<f32>(0.5, 0.5));
                // Draw soft glowing circles
                snow_val += smoothstep(0.4, 0.1, d) * (0.4 + hash12(id) * 0.6);
            }
        }
        color += vec3<f32>(snow_val);
        
    } else if (uniforms.weather_type == 1u) {
        // --- Cloudy / Fog ---
        color = mix(color, vec3<f32>(0.7, 0.75, 0.8), 0.25);
    }

    return vec4<f32>(color, 1.0);
}