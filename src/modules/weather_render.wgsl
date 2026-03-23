struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    lifetime: f32,
    scale: f32,
    padding: vec2<f32>,
}

@group(0) @binding(0) var<storage, read> particles: array<Particle>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacity: f32,
};

@vertex
fn vs_main(@builtin(vertex_index) v_idx: u32, @builtin(instance_index) i_idx: u32) -> VertexOutput {
    let p = particles[i_idx];
    var out: VertexOutput;

    var pos = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2( 1.0, -1.0), vec2(-1.0,  1.0),
        vec2(-1.0,  1.0), vec2( 1.0, -1.0), vec2( 1.0,  1.0)
    );
    let quad_pos = pos[v_idx];
    let speed = length(p.vel);
    
    // Fast falling rain stretches into a streak, snow stays rounder
    let is_fast = speed > 0.4;
    let width = select(0.005, 0.002, is_fast) * p.scale;
    let height = select(0.005, 0.01 + speed * 0.03, is_fast) * p.scale;

    let angle = atan2(p.vel.y, p.vel.x) - 1.570796; 
    let s = sin(angle);
    let c = cos(angle);

    let local_pos = vec2<f32>(quad_pos.x * width, quad_pos.y * height);
    let rot_pos = vec2<f32>(local_pos.x * c - local_pos.y * s, local_pos.x * s + local_pos.y * c);

    out.clip_position = vec4<f32>(p.pos.x * 2.0 - 0.5 + rot_pos.x, 1.0 - (p.pos.y * 2.0) + rot_pos.y, 0.0, 1.0);
    out.uv = quad_pos * 0.5 + 0.5;
    
    let fade_in = min(1.0, (p.pos.y + 0.2) * 5.0);
    out.opacity = clamp(p.lifetime, 0.0, 1.0) * clamp(fade_in, 0.0, 1.0) * 0.5;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let cuv = in.uv * 2.0 - 1.0;
    let dist = length(cuv);
    if dist > 1.0 { discard; }

    let alpha = (1.0 - dist * dist) * in.opacity;
    let highlight = smoothstep(0.6, 0.0, length(cuv - vec2(-0.2, -0.2))) * 0.8;
    return vec4<f32>(0.85 + highlight, 0.95 + highlight, 1.0 + highlight, alpha);
}