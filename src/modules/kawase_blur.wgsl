// Dual-Kawase blur, ported from cosmic-comp's
// src/backend/render/shaders/blur_{downsample,upsample}.frag so the frosted
// glass background matches COSMIC's native compositor effect.
//
// half_pixel is 0.5 / source_texture_size for each pass; offset comes from
// cosmic-comp's strength table (see renderer/blur.rs). The alpha
// renormalisation in the originals is dropped: our sources are opaque, so the
// weight sums are the constants 8 (down) and 12 (up).

struct BlurUniforms {
    half_pixel: vec2<f32>,
    offset: f32,
    _padding: f32,
}

@group(0) @binding(0) var<uniform> uniforms: BlurUniforms;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var src_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Standard full-screen triangle trick
@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    return VertexOutput(
        vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0),
        vec2<f32>(x, y)
    );
}

@fragment
fn fs_down(in: VertexOutput) -> @location(0) vec4<f32> {
    let step = uniforms.half_pixel * uniforms.offset;

    var sum = textureSample(src, src_sampler, in.uv) * 4.0;
    sum += textureSample(src, src_sampler, in.uv - step);
    sum += textureSample(src, src_sampler, in.uv + step);
    sum += textureSample(src, src_sampler, in.uv + vec2<f32>(step.x, -step.y));
    sum += textureSample(src, src_sampler, in.uv - vec2<f32>(step.x, -step.y));

    return sum / 8.0;
}

@fragment
fn fs_up(in: VertexOutput) -> @location(0) vec4<f32> {
    let step = uniforms.half_pixel * uniforms.offset;

    var sum = textureSample(src, src_sampler, in.uv + vec2<f32>(-step.x * 2.0, 0.0));
    sum += textureSample(src, src_sampler, in.uv + vec2<f32>(-step.x, step.y)) * 2.0;
    sum += textureSample(src, src_sampler, in.uv + vec2<f32>(0.0, step.y * 2.0));
    sum += textureSample(src, src_sampler, in.uv + vec2<f32>(step.x, step.y)) * 2.0;
    sum += textureSample(src, src_sampler, in.uv + vec2<f32>(step.x * 2.0, 0.0));
    sum += textureSample(src, src_sampler, in.uv + vec2<f32>(step.x, -step.y)) * 2.0;
    sum += textureSample(src, src_sampler, in.uv + vec2<f32>(0.0, -step.y * 2.0));
    sum += textureSample(src, src_sampler, in.uv - vec2<f32>(step.x, step.y)) * 2.0;

    return sum / 12.0;
}
