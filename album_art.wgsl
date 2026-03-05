// =============================================================================
// shaders/album_art.wgsl
// =============================================================================
// WGSL (WebGPU Shading Language) shader for the album art wallpaper scene.
//
// This shader runs entirely on the GPU. It:
//   1. Takes the album art as a texture
//   2. Applies a Gaussian blur
//   3. Overlays a colour wash from the dominant palette colours
//   4. Vignettes the edges for a cinematic look
//
// For beginners: a shader is a small program uploaded to the GPU. The "vertex
// shader" runs once per corner of our rectangle, the "fragment shader" runs
// once per pixel and decides its colour.
// =============================================================================

// --- Uniforms (values we send from Rust to the GPU each frame) ---
struct Uniforms {
    // How far through the transition we are (0.0 = old scene, 1.0 = new scene)
    transition: f32,
    // Time of day, 0.0–1.0, for subtle animation
    time: f32,
    // Dominant colour from the album art palette
    palette_primary: vec3<f32>,
    palette_secondary: vec3<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var album_art: texture_2d<f32>;
@group(0) @binding(2) var art_sampler: sampler;

// --- Vertex shader ---
// We draw a full-screen quad (two triangles covering the whole surface).
// The vertex shader just passes UV coordinates to the fragment shader.
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    // Full-screen triangle trick — generates a quad from 4 vertices
    var positions = array<vec2<f32>, 4>(
        vec2(-1.0, -1.0),
        vec2( 1.0, -1.0),
        vec2(-1.0,  1.0),
        vec2( 1.0,  1.0),
    );
    var uvs = array<vec2<f32>, 4>(
        vec2(0.0, 1.0),
        vec2(1.0, 1.0),
        vec2(0.0, 0.0),
        vec2(1.0, 0.0),
    );

    var out: VertexOutput;
    out.position = vec4(positions[idx], 0.0, 1.0);
    out.uv = uvs[idx];
    return out;
}

// --- Fragment shader ---
// Runs for every pixel. Returns the colour of that pixel.
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // --- Gaussian blur (simplified box blur for clarity) ---
    // In practice you'd do multiple passes with a proper Gaussian kernel.
    // Here we sample 9 nearby pixels and average them.
    let blur_radius = 0.008;
    var colour = vec3(0.0);
    var weight = 0.0;

    for (var x: i32 = -2; x <= 2; x++) {
        for (var y: i32 = -2; y <= 2; y++) {
            let offset = vec2(f32(x), f32(y)) * blur_radius;
            let sample_uv = clamp(uv + offset, vec2(0.0), vec2(1.0));
            let w = 1.0 - length(offset) / (blur_radius * 3.0);
            colour += textureSample(album_art, art_sampler, sample_uv).rgb * w;
            weight += w;
        }
    }
    colour /= weight;

    // --- Colour grade: blend with dominant palette colour ---
    // This tints the blurred art with the album's colour scheme,
    // making the wallpaper feel cohesive with the album's aesthetic.
    let tint = mix(uniforms.palette_primary, uniforms.palette_secondary,
                   sin(uniforms.time * 3.14159) * 0.5 + 0.5);
    colour = mix(colour, tint, 0.25); // 25% tint strength

    // --- Vignette: darken the edges ---
    let vignette_uv = uv * 2.0 - 1.0; // remap to -1..1
    let vignette = 1.0 - dot(vignette_uv, vignette_uv) * 0.4;
    colour *= vignette;

    // --- Slightly darken overall so desktop icons remain readable ---
    colour *= 0.75;

    // --- Apply transition fade ---
    colour = mix(vec3(0.0), colour, uniforms.transition);

    return vec4(colour, 1.0);
}
