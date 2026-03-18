struct Uniforms {
    // We pack RGB tint and the Alpha transition progress into a single vec4 
    // to perfectly align with a 16-byte buffer in Rust.
    tint_and_transition: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var t_diffuse: texture_2d<f32>;
@group(0) @binding(2) var s_diffuse: sampler;
@group(0) @binding(3) var t_previous: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // Draw a full-screen triangle using only 3 vertices
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = vec3<f32>(0.0);
    let uv = in.uv;
    
    // Smooth 16-tap spiral blur for a stylized background
    let samples = 16.0;
    let radius = 0.08;
    
    // Performance optimization: Avoid sampling the previous texture 16 times if the crossfade is complete
    if uniforms.tint_and_transition.a >= 1.0 {
        for (var i = 0u; i < 16u; i = i + 1u) {
            let t = f32(i) / samples;
            let angle = t * 18.84955; // 3 full turns (3 * 2 * PI)
            let offset = vec2<f32>(cos(angle), sin(angle)) * (t * radius);
            color += textureSample(t_diffuse, s_diffuse, uv + offset).rgb;
        }
        color = color / samples;
    } else {
        var prev_color = vec3<f32>(0.0);
        for (var i = 0u; i < 16u; i = i + 1u) {
            let t = f32(i) / samples;
            let angle = t * 18.84955;
            let offset = vec2<f32>(cos(angle), sin(angle)) * (t * radius);
            color += textureSample(t_diffuse, s_diffuse, uv + offset).rgb;
            prev_color += textureSample(t_previous, s_diffuse, uv + offset).rgb;
        }
        color = color / samples;
        prev_color = prev_color / samples;
        color = mix(prev_color, color, uniforms.tint_and_transition.a);
    }
    
    // Mix with the dominant palette colour and darken slightly
    let final_color = mix(color, uniforms.tint_and_transition.rgb, 0.65) * 0.4;
    return vec4<f32>(final_color, 1.0);
}