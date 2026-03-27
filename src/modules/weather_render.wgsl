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
    @location(2) is_rain: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) v_idx: u32, @builtin(instance_index) i_idx: u32) -> VertexOutput {
    let p = particles[i_idx];
    var out: VertexOutput = VertexOutput();

    var pos = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2( 1.0, -1.0), vec2(-1.0,  1.0),
        vec2(-1.0,  1.0), vec2( 1.0, -1.0), vec2( 1.0,  1.0)
    );
    let quad_pos = pos[v_idx];
    let speed = length(p.vel);
    
    let is_fast = speed > 0.8;
    let width = select(0.005, 0.0008, is_fast) * p.scale;
    let height = select(0.005, 0.03 + speed * 0.015, is_fast) * p.scale;

    let angle = atan2(p.vel.y, p.vel.x) - 1.570796; 
    let s = sin(angle);
    let c = cos(angle);

    let local_pos = vec2<f32>(quad_pos.x * width, quad_pos.y * height);
    let rot_pos = vec2<f32>(local_pos.x * c - local_pos.y * s, local_pos.x * s + local_pos.y * c);

    out.clip_position = vec4<f32>(p.pos.x * 2.0 - 0.5 + rot_pos.x, 1.0 - (p.pos.y * 2.0) + rot_pos.y, 0.0, 1.0);
    out.uv = quad_pos * 0.5 + 0.5;
    
    let fade_in = min(1.0, (p.pos.y + 0.2) * 5.0);
    let depth_fade = smoothstep(0.0, 1.0, p.scale); // Distant particles are more transparent
    out.opacity = clamp(p.lifetime, 0.0, 1.0) * clamp(fade_in, 0.0, 1.0) * depth_fade * 0.8;
    out.is_rain = select(0.0, 1.0, is_fast);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let cuv = in.uv * 2.0 - 1.0;
    
    var alpha = 0.0;
    if in.is_rain > 0.5 {
        let dx = abs(cuv.x);
        if dx > 1.0 { discard; }
        
        // Teardrop profile: fade out heavily at the top tail, sharper drop at the bottom tip
        let vertical_profile = smoothstep(-1.0, 0.2, cuv.y) * smoothstep(1.0, 0.6, cuv.y);
        
        // Gaussian blur for a soft horizontal width profile
        let horizontal_profile = exp(-6.0 * dx * dx); 
        
        alpha = horizontal_profile * vertical_profile * in.opacity;
        
        return vec4f(0.85, 0.9, 0.95, alpha); // More natural water color
    } else {
        let dist = length(cuv);
        if dist > 1.0 { discard; }
        // Soft fluffy snow
        alpha = smoothstep(1.0, 0.0, dist) * in.opacity;
        let highlight = smoothstep(0.6, 0.0, length(cuv - vec2(-0.2, -0.2))) * 0.8;
        return vec4f(0.9 + highlight, 0.95 + highlight, 1.0 + highlight, alpha);
    }
}