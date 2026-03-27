// v4
struct ArtUniforms {
    color_and_transition: vec4<f32>,
    res: vec2<f32>,
    art_position: vec2<f32>,
    audio_energy: f32,
    mode: u32,
    bg_alpha: f32,
    art_size: f32,
    shape: u32,
    blur_opacity: f32,
    image_res: vec2<f32>,
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

// Optimized Golden Ratio (Vogel) Spiral Blur.
// Provides an incredibly smooth, frosted glass look matching Kawase quality
// but executes in a single pass to save framerate.
fn blur(uv: vec2<f32>, radius: f32) -> vec4f {
    let texel_size = 1.0 / uniforms.res * radius * 6.0;
    var total = vec4<f32>(0.0);

    let samples = 16.0;
    let golden_angle = 2.39996; // ~137.5 degrees

    for (var i: i32 = 0; i < 16; i++) {
        let f_i = f32(i);
        let r = sqrt(f_i + 0.5) / sqrt(samples);
        let theta = f_i * golden_angle;

        let offset = vec2<f32>(cos(theta), sin(theta)) * r;
        total += sample_art(uv + offset * texel_size);
    }

    return total / samples;
}

// Maps UV coordinates to achieve an "object-fit: cover" effect.
// The image fills the entire area, potentially cropping parts of the image.
fn get_object_cover_uv(uv: vec2<f32>, screen_aspect: f32, image_aspect: f32) -> vec2<f32> {
    var tex_uv = uv;
    let new_aspect = screen_aspect / image_aspect;

    if (new_aspect > 1.0) { // Screen is wider than image, fit to height, crop width
        let scale = 1.0 / new_aspect;
        tex_uv.x = tex_uv.x * scale + (1.0 - scale) / 2.0;
    } else { // Screen is taller than image, fit to width, crop height
        let scale = new_aspect;
        tex_uv.y = tex_uv.y * scale + (1.0 - scale) / 2.0;
    }
    return tex_uv;
}

// Maps UV coordinates to achieve an "object-fit: contain" effect.
// The returned UVs may be outside the [0, 1] range, which indicates
// the pixel is in a letterbox/pillarbox area.
fn get_object_contain_uv(uv: vec2<f32>, screen_aspect: f32, image_aspect: f32) -> vec2<f32> {
    var tex_uv = uv - 0.5;
    let new_aspect = screen_aspect / image_aspect;

    if (new_aspect > 1.0) { // Screen is wider than image, pillarbox
        tex_uv.x *= new_aspect;
    } else { // Screen is taller than image, letterbox
        tex_uv.y /= new_aspect;
    }
    return tex_uv + 0.5;
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
    let screen_aspect = uniforms.res.x / uniforms.res.y;
    let image_aspect = max(uniforms.image_res.x / max(uniforms.image_res.y, 1.0), 0.001);
    let uv = in.uv;

    // --- Mode 0: Frosted Glass Background ---
    if uniforms.mode == 0u {
        let cover_uv = get_object_cover_uv(uv, screen_aspect, image_aspect); // Use cover for background
        let raw_bg = sample_art(cover_uv);
        
        if uniforms.blur_opacity < 0.01 {
            return vec4<f32>(raw_bg.rgb, uniforms.bg_alpha);
        }
        
        // Dynamically scale the blur radius based on the opacity slider
        let blur_radius = uniforms.blur_opacity * 5.0; 
        let blurred_bg = blur(cover_uv, blur_radius); // Use cover for background
        
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
        let p_aspect = p * vec2<f32>(screen_aspect, 1.0);

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

        let local_uv = (p_aspect / uniforms.art_size) + 0.5;
        let art_uv = get_object_contain_uv(local_uv, 1.0, image_aspect);

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
        let cover_uv = get_object_cover_uv(uv, screen_aspect, image_aspect); // Use cover for background
        let art_color = sample_art(cover_uv);
        return vec4<f32>(art_color.rgb, uniforms.bg_alpha);
    }

    return vec4<f32>(0.0);
}