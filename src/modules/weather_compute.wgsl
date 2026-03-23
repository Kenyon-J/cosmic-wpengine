struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    lifetime: f32,
    scale: f32,
    padding: vec2<f32>,
}

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;

struct WeatherUniforms {
    delta_time: f32,
    wind_x: f32,
    gravity: f32,
    padding: f32,
}
@group(0) @binding(1) var<uniform> uniforms: WeatherUniforms;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if index >= arrayLength(&particles) { return; }

    var p = particles[index];

    // Apply simple physics using frame delta
    p.pos.x += (p.vel.x + uniforms.wind_x) * uniforms.delta_time;
    p.pos.y += (p.vel.y + uniforms.gravity) * uniforms.delta_time;
    p.lifetime -= uniforms.delta_time;

    // If the particle falls off the bottom or dies, wrap it back to the top
    if p.pos.y > 1.2 || p.lifetime <= 0.0 {
        p.pos.y = -0.2;
        
        // Generate a pseudo-random X position to respawn it at
        let seed = f32(index) * 12.9898 + p.lifetime;
        p.pos.x = fract(sin(seed) * 43758.5453) * 2.0 - 0.5;
        p.lifetime = 5.0 + fract(sin(seed * 1.1) * 12345.0) * 2.0; 
        p.vel.x = (fract(sin(seed * 1.2) * 54321.0) - 0.5) * 0.2; // Slight wind deviation
    }

    // Write updated state back into the buffer
    particles[index] = p;
}