// v5
struct ArtUniforms {
    color_and_transition: vec4<f32>,
    uv_transform: vec4<f32>, // scale_x, scale_y, offset_x, offset_y
    art_position: vec2<f32>,
    blur_step: vec2<f32>, // unused since the Kawase blur moved offscreen; kept for layout
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
// Cached dual-Kawase blur of current_art, rebuilt offscreen when the artwork
// or the blur amount changes (renderer/blur.rs). Bound to current_art itself
// for pipeline modes that never sample it.
@group(0) @binding(3) var blurred_art: texture_2d<f32>;

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

// COSMIC's glass look composites the theme's flat neutral background over the
// compositor blur (cosmic-theme dark gray_1 #1B1B1B, here in linear light).
const GLASS_TINT: vec3<f32> = vec3<f32>(0.011, 0.011, 0.011);

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

        // Same UV space as the sharp art: the cached blur preserves the
        // source's aspect ratio, only its resolution differs.
        let blurred_bg = textureSampleLevel(
            blurred_art,
            art_sampler,
            clamp(cover_uv, vec2<f32>(0.0), vec2<f32>(1.0)),
            0.0
        );

        // Fade between the sharp and blurred image
        var final_color = mix(raw_bg.rgb, blurred_bg.rgb, uniforms.blur_opacity);

        // COSMIC composites its translucent neutral surface over the blur
        // (AlphaMap alphas run 0.60-0.90 for windows); scaled down here so
        // the artwork stays visible behind the frost instead of flattening
        // to a solid pane.
        final_color = mix(final_color, GLASS_TINT, uniforms.blur_opacity * 0.45);

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
