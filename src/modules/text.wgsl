// v1
@group(0) @binding(0) var text_texture: texture_2d<f32>;
@group(0) @binding(1) var text_sampler: sampler;

struct VertexInput {
    @location(0) pos: vec2<f32>,
    @location(1) tex_pos: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) tex_pos: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.pos = vec4<f32>(input.pos, 0.0, 1.0);
    out.tex_pos = input.tex_pos;
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = textureSample(text_texture, text_sampler, in.tex_pos).r;
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}