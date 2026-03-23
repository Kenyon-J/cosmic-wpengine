struct Uniforms {
    color_and_transition: vec4<f32>, // rgb = color, a = transition
    res: vec2<f32>,
    art_position: vec2<f32>,
    audio_energy: f32,
    mode: u32,
    bg_alpha: f32,
    art_size: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var t_diffuse: texture_2d<f32>;
@group(0) @binding(2) var s_diffuse: sampler;
@group(0) @binding(3) var t_previous: texture_2d<f32>;

// Precomputed Vogel Spiral Offsets (12 samples)
var<private> SPIRAL_OFFSETS: array<vec2<f32>, 12> = array<vec2<f32>, 12>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(-0.061449, 0.056288),
    vec2<f32>(0.014582, -0.166027),
    vec2<f32>(0.150835, 0.199370),
    vec2<f32>(-0.327483, -0.062173),
    vec2<f32>(0.348087, -0.229017),
    vec2<f32>(-0.085280, 0.492670),
    vec2<f32>(-0.332208, -0.479488),
    vec2<f32>(0.662473, 0.074640),
    vec2<f32>(-0.586260, 0.467745),
    vec2<f32>(0.210816, -0.806216),
    vec2<f32>(0.444693, 0.801570)
);

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
    let uv = in.uv;
    let aspect = uniforms.res.x / uniforms.res.y;
    let transition = uniforms.color_and_transition.a;

    if uniforms.mode == 1u {
        let size = uniforms.art_size;
        let radius = 0.02;
        let p = vec2<f32>((uv.x - uniforms.art_position.x) * aspect, uv.y - uniforms.art_position.y);
        let d = length(max(abs(p) - vec2<f32>(size - radius), vec2<f32>(0.0))) - radius;

        if d > 0.0 {
            return vec4<f32>(0.0);
        }

        let tex_uv = p / (size * 2.0) + vec2<f32>(0.5);
        var final_color = textureSample(t_diffuse, s_diffuse, tex_uv).rgb;
        
        if transition < 1.0 {
            let prev_color = textureSample(t_previous, s_diffuse, tex_uv).rgb;
            final_color = mix(prev_color, final_color, transition);
        }

        return vec4<f32>(final_color, 1.0);
    } else if uniforms.mode == 2u {
        // --- Unblurred Background Mode (Disable Blur) ---
        var color = textureSample(t_diffuse, s_diffuse, uv).rgb;
        if transition < 1.0 {
            let prev_color = textureSample(t_previous, s_diffuse, uv).rgb;
            color = mix(prev_color, color, transition);
        }
        let final_color = mix(color, uniforms.color_and_transition.rgb, 0.3) * 0.75;
        return vec4<f32>(final_color, 0.8 * uniforms.bg_alpha);
    } else {
        var color = vec3<f32>(0.0);
        let blur_radius = 0.08 + (uniforms.audio_energy * 0.02); // Blur intensifies softly with audio

        if transition >= 1.0 {
            for (var i = 0u; i < 12u; i = i + 1u) {
                let offset = SPIRAL_OFFSETS[i] * blur_radius;
                color += textureSample(t_diffuse, s_diffuse, uv + offset).rgb;
            }
            color = color / 12.0;
        } else {
            var prev_color = vec3<f32>(0.0);
            for (var i = 0u; i < 12u; i = i + 1u) {
                let offset = SPIRAL_OFFSETS[i] * blur_radius;
                color += textureSample(t_diffuse, s_diffuse, uv + offset).rgb;
                prev_color += textureSample(t_previous, s_diffuse, uv + offset).rgb;
            }
            color = mix(prev_color / 12.0, color / 12.0, transition);
        }
        
        // Audio-reactive frosted noise
        let noise = fract(sin(dot(uv, vec2<f32>(12.9898, 78.233))) * 43758.5453);
        color += vec3<f32>(noise * (0.02 + uniforms.audio_energy * 0.02));

        let final_color = mix(color, uniforms.color_and_transition.rgb, 0.5) * 0.5;
        
        // Multiply by bg_alpha to allow transparent backgrounds to fade cleanly
        return vec4<f32>(final_color, 0.8 * uniforms.bg_alpha);
    }
}