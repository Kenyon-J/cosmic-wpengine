// v4
struct ArtUniforms {
    color_and_transition: vec4<f32>,
    uv_transform: vec4<f32>, // scale_x, scale_y, offset_x, offset_y
    art_position: vec2<f32>,
    blur_step: vec2<f32>,
    audio_energy: f32,
    mode: u32,
    bg_alpha: f32,
    art_size: f32,
    shape: u32,
    blur_opacity: f32,
    screen_aspect: f32,
    _padding: u32,
}

@group(0) @binding(0) var<uniform> uniforms: ArtUniforms;
@group(0) @binding(1) var current_art: texture_2d<f32>;
@group(0) @binding(2) var art_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Standard full-screen triangle trick
@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    var out: VertexOutput = VertexOutput(
        vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0),
        vec2<f32>(x, y)
    );
    return out;
}

fn sample_art(uv: vec2<f32>) -> vec4f {
    // textureSampleLevel MUST be used instead of textureSample here! 
    // The 'discard' statements in the fragment shader break the 2x2 pixel quads 
    // needed to calculate derivatives (dpdx/dpdy) for mipmap level selection.
    
    // Strictly clamp the UVs to bypass any strict driver out-of-bounds panics
    return textureSampleLevel(current_art, art_sampler, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0);
}

var<private> BLUR_OFFSETS: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
    vec2<f32>(0.176777, 0.000000),
    vec2<f32>(-0.225772, 0.206827),
    vec2<f32>(0.034556, -0.393771),
    vec2<f32>(0.284575, 0.371170),
    vec2<f32>(-0.522224, -0.092367),
    vec2<f32>(0.494690, -0.314693),
    vec2<f32>(-0.165454, 0.615528),
    vec2<f32>(-0.315575, -0.607587),
    vec2<f32>(0.684649, 0.250013),
    vec2<f32>(-0.712248, 0.294030),
    vec2<f32>(0.343331, -0.733740),
    vec2<f32>(0.253759, 0.808923),
    vec2<f32>(-0.764763, -0.443156),
    vec2<f32>(0.897126, -0.197270),
    vec2<f32>(-0.547472, 0.778797),
    vec2<f32>(-0.126534, -0.976084)
);

// Optimized Golden Ratio (Vogel) Spiral Blur.
// Provides an incredibly smooth, frosted glass look matching Kawase quality
// but executes in a single pass to save framerate.
fn blur(uv: vec2<f32>) -> vec4f {
    var total = vec4<f32>(0.0);

    for (var i: i32 = 0; i < 16; i++) {
        let offset = BLUR_OFFSETS[i];
        total += sample_art(uv + offset * uniforms.blur_step);
    }

    return total / 16.0;
}

fn get_shape_mask(point: vec2<f32>, size: f32, shape: u32) -> f32 {
    let half_size = size * 0.5;
    var mask = 0.0;
    if shape == 1u { // Circular
        let dist = length(point);
        mask = 1.0 - smoothstep(half_size - 0.005, half_size, dist);
    } else { // Square with soft corners
        let abs_p = abs(point);
        let max_dist = max(abs_p.x, abs_p.y);
        mask = 1.0 - smoothstep(half_size - 0.005, half_size, max_dist);
    }
    return mask;
}


// --- Fragment shader ---
// Runs for every pixel. Returns the colour of that pixel.
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let uv = in.uv;

    // --- Mode 0: Frosted Glass Background ---
    if uniforms.mode == 0u {
        let cover_uv = uv * uniforms.uv_transform.xy + uniforms.uv_transform.zw;
        let raw_bg = sample_art(cover_uv);
        
        if uniforms.blur_opacity < 0.01 {
            return vec4<f32>(raw_bg.rgb, uniforms.bg_alpha);
        }
        
        let blurred_bg = blur(cover_uv); // Use pre-calculated blur step
        
        // Fade between the sharp and blurred image
        var final_color = mix(raw_bg.rgb, blurred_bg.rgb, uniforms.blur_opacity);
        
        // Apply a balanced dimming tint so the effect isn't completely overpowering like before.
        let dim_factor = 1.0 - (uniforms.blur_opacity * 0.4); 
        let tint = mix(vec3<f32>(1.0), uniforms.color_and_transition.rgb, uniforms.blur_opacity * 0.5);
        
        final_color = final_color * tint * dim_factor;
        
        return vec4<f32>(final_color, uniforms.bg_alpha);
    }

    // --- Mode 1: Foreground Album Art ---
    if uniforms.mode == 1u {
        let p = uv - uniforms.art_position;
        let p_aspect = p * vec2<f32>(uniforms.screen_aspect, 1.0);

        // --- Drop Shadow ---
        // Create a soft, blurred shadow by sampling the shape mask at an offset.
        let shadow_offset = vec2<f32>(0.005, 0.005) * (uniforms.art_size / 0.25);
        let shadow_mask = get_shape_mask(p_aspect - shadow_offset, uniforms.art_size, uniforms.shape);

        // --- Foreground Art ---
        let art_mask = get_shape_mask(p_aspect, uniforms.art_size, uniforms.shape);

        // If we are fully transparent for both art and shadow, we can discard early.
        if art_mask < 0.01 && shadow_mask < 0.01 {
            discard;
        }

        // Use CPU-hoisted MAD UV transform for foreground containment
        let art_uv = uv * uniforms.uv_transform.xy + uniforms.uv_transform.zw;

        var final_color = vec4<f32>(0.0, 0.0, 0.0, shadow_mask * 0.6); // Base shadow
        
        // Only sample the texture if we are inside the letterboxed art bounds
        if art_mask > 0.01 {
            if art_uv.x >= 0.0 && art_uv.x <= 1.0 && art_uv.y >= 0.0 && art_uv.y <= 1.0 {
                let sampled = sample_art(art_uv);
                // Guarantee alpha is 1.0 to prevent transparent album art from disappearing
                final_color = mix(final_color, vec4<f32>(sampled.rgb, 1.0), art_mask);
            } else {
                // Draw a dark letterbox if the image aspect ratio doesn't fill the shape
                final_color = mix(final_color, vec4<f32>(0.05, 0.05, 0.05, 1.0), art_mask);
            }
        }

        return final_color;
    }

    // --- Mode 2: Solid (Un-blurred) Background ---
    if uniforms.mode == 2u {
        let cover_uv = uv * uniforms.uv_transform.xy + uniforms.uv_transform.zw;
        let art_color = sample_art(cover_uv);
        return vec4<f32>(art_color.rgb, uniforms.bg_alpha);
    }

    // --- Mode 3: Solid Color Background ---
    if uniforms.mode == 3u {
        // Darken the primary color for a sleek, deep background
        let dark_bg = uniforms.color_and_transition.rgb * 0.2; 
        
        // Add a subtle vignette drop shadow in the corners for extra depth
        let dist = distance(uv, vec2<f32>(0.5, 0.5));
        let vignette = smoothstep(0.8, 0.2, dist);
        
        return vec4<f32>(dark_bg * vignette, uniforms.bg_alpha);
    }

    return vec4<f32>(0.0);
}
