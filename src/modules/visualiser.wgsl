struct VisualiserUniforms {
    resolution: vec2<f32>,
    band_count: u32,
    lyric_pulse: f32,
    color_top: vec4<f32>,
    color_bottom: vec4<f32>,
    pos_size_rot: vec4<f32>,
    amplitude: f32,
    shape: u32, // 0=circular, 1=linear
    time: f32,
    align: u32, // 0=left, 1=center, 2=right
    is_waveform: u32, // bool
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
}

@group(0) @binding(0) var<uniform> uniforms: VisualiserUniforms;
@group(0) @binding(1) var<storage, read> bands: array<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) local_uv: vec2<f32>,
    @location(2) bar_val: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) v_idx: u32, @builtin(instance_index) i_idx: u32) -> VertexOutput {
    var out: VertexOutput;
    
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0)
    );
    let p_quad = pos[v_idx];

    if (uniforms.is_waveform == 1u) {
        if (i_idx == 0u) {
            out.clip_position = vec4<f32>(p_quad.x * 2.0 - 1.0, 1.0 - p_quad.y * 2.0, 0.0, 1.0);
            out.uv = p_quad;
        } else {
            out.clip_position = vec4<f32>(0.0);
        }
        return out;
    }

    if (uniforms.shape == 1u) {
        let norm_x = f32(i_idx) / f32(uniforms.band_count);
        var mapped_x = norm_x;
        if uniforms.align == 1u {
            mapped_x = abs(norm_x - 0.5) * 2.0;
        } else if uniforms.align == 2u {
            mapped_x = 1.0 - norm_x;
        }
        
        let band_idx = min(u32(mapped_x * f32(uniforms.band_count)), uniforms.band_count - 1u);
        let val = bands[band_idx];

        let aspect = uniforms.resolution.x / uniforms.resolution.y;
        let total_width = uniforms.pos_size_rot.z * aspect;
        let bar_width = total_width / f32(uniforms.band_count);
        let max_height = uniforms.amplitude * 0.25 * (1.0 + uniforms.lyric_pulse * 1.5) + 0.05;
        let height = max(val, 0.02) * 0.25 * uniforms.amplitude * (1.0 + uniforms.lyric_pulse * 1.5);

        out.local_uv = p_quad;
        out.bar_val = height;
        out.uv = vec2<f32>(norm_x, 0.0);

        let glow_pad_x = bar_width * 1.5;
        let glow_pad_y = 0.1 + uniforms.lyric_pulse * 0.05; 
        
        let quad_w = bar_width + glow_pad_x * 2.0;
        let quad_h = max_height + glow_pad_y * 2.0;
        
        let local_x = (p_quad.x * quad_w) - (quad_w * 0.5);
        let local_y = (p_quad.y * quad_h) - glow_pad_y;
        
        let offset_x = (norm_x - 0.5) * total_width + (bar_width * 0.5);
        let offset_y = 0.0;
        
        let p = vec2<f32>(offset_x + local_x, offset_y - local_y);
        
        let s = sin(uniforms.pos_size_rot.w);
        let c = cos(uniforms.pos_size_rot.w);
        let p_rot = vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
        
        let screen_p = vec2<f32>(p_rot.x / aspect, p_rot.y);
        
        let final_uv = screen_p + uniforms.pos_size_rot.xy;
        out.clip_position = vec4<f32>(final_uv.x * 2.0 - 1.0, 1.0 - final_uv.y * 2.0, 0.0, 1.0);
        return out;

    } else {
        let norm_angle = f32(i_idx) / f32(uniforms.band_count * 2u);
        let angle = norm_angle * 6.2831853 - 3.14159265;
        
        var f_band = norm_angle * 2.0;
        if f_band > 1.0 { f_band = 2.0 - f_band; }
        
        let band_idx = min(u32(f_band * f32(uniforms.band_count)), uniforms.band_count - 1u);
        let val = bands[band_idx];

        let base_radius = uniforms.pos_size_rot.z + (uniforms.lyric_pulse * 0.02);
        let max_height = uniforms.amplitude * 0.25 * (1.0 + uniforms.lyric_pulse * 1.5) + 0.05;
        let height = max(val, 0.02) * 0.25 * uniforms.amplitude * (1.0 + uniforms.lyric_pulse * 1.5);
        
        let circumference = 6.2831853 * base_radius;
        let bar_width = circumference / f32(uniforms.band_count * 2u);
        
        let glow_pad_x = bar_width * 1.5;
        let glow_pad_y = 0.1 + uniforms.lyric_pulse * 0.05;
        
        let quad_w = bar_width + glow_pad_x * 2.0;
        let quad_h = max_height + glow_pad_y * 2.0;
        
        out.local_uv = p_quad;
        out.bar_val = height;
        out.uv = vec2<f32>(f_band, 0.0);
        
        let local_x = (p_quad.x * quad_w) - (quad_w * 0.5); 
        let local_y = (p_quad.y * quad_h) - glow_pad_y; 
        
        let r = base_radius + local_y;
        
        let p = vec2<f32>(
            r * cos(angle) - local_x * sin(angle),
            r * sin(angle) + local_x * cos(angle)
        );
        
        let s = sin(uniforms.pos_size_rot.w);
        let c = cos(uniforms.pos_size_rot.w);
        let p_rot = vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
        
        let aspect = uniforms.resolution.x / uniforms.resolution.y;
        let screen_p = vec2<f32>(p_rot.x / aspect, p_rot.y);
        let final_uv = screen_p + uniforms.pos_size_rot.xy;
        out.clip_position = vec4<f32>(final_uv.x * 2.0 - 1.0, 1.0 - final_uv.y * 2.0, 0.0, 1.0);
        return out;
    }
}

fn get_vis_waveform(uv: vec2<f32>, s: f32, c: f32, aspect: f32) -> vec4<f32> {
    let p = vec2<f32>((uv.x - uniforms.pos_size_rot.x) * aspect, uv.y - uniforms.pos_size_rot.y);
    
    let base_radius = uniforms.pos_size_rot.z + (uniforms.lyric_pulse * 0.02);
    let inner_bound = base_radius - 0.2;
    let d_sq = dot(p, p);
    if inner_bound > 0.0 && d_sq < inner_bound * inner_bound {
        return vec4<f32>(0.0);
    }
    let d = sqrt(d_sq);

    let p_rot = vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
    let f_band = 1.0 - (abs(atan2(p_rot.y, p_rot.x)) / 3.14159265);

    let band_idx = min(u32(f_band * f32(uniforms.band_count)), uniforms.band_count - 1u);
    let next_idx = min(band_idx + 1u, uniforms.band_count - 1u);
    let fract_band = fract(f_band * f32(uniforms.band_count));

    let val1 = bands[band_idx];
    let val2 = bands[next_idx];
    
    let smooth_fract = smoothstep(0.0, 1.0, fract_band);
    let val = mix(val1, val2, smooth_fract);

    let wave_offset = val * uniforms.amplitude * 0.1;
    let displaced_radius = base_radius + (wave_offset * 0.5);
    let dist_to_line = abs(d - displaced_radius);
    let thickness = abs(wave_offset * 0.75) + 0.003 + (uniforms.lyric_pulse * 0.005);
    let edge = smoothstep(thickness + 0.005, thickness - 0.005, dist_to_line);
    
    let gradient_factor = (p_rot.y + uniforms.pos_size_rot.z) / (uniforms.pos_size_rot.z * 2.0);
    let base_color = mix(uniforms.color_bottom.rgb, uniforms.color_top.rgb, clamp(gradient_factor, 0.0, 1.0));
    let core = smoothstep(0.005, 0.0, dist_to_line) * 0.6;
    let glow = exp(-dist_to_line * 20.0) * 0.5;
    
    let final_color = base_color * edge + vec3<f32>(core) + (base_color * glow);
    let final_alpha = max(edge, glow);

    return vec4<f32>(final_color, final_alpha);
}

fn eval_shape(lx: f32, ly: f32, half_w: f32, height: f32, glow_intensity: f32, pulse_mult: f32) -> vec2<f32> {
    if (abs(lx) > half_w) { return vec2<f32>(0.0, 0.0); }
    if (ly < 0.0) { return vec2<f32>(0.0, 0.0); }
    
    if (ly <= height) {
        return vec2<f32>(1.0, 0.0);
    }
    
    let glow_dist = ly - height;
    let glow = clamp(0.005 / (glow_dist * glow_dist * glow_intensity + 0.005) - 0.1, 0.0, 1.0) * (1.0 + uniforms.lyric_pulse * pulse_mult);
    
    return vec2<f32>(0.0, glow);
}

fn eval_shadow(lx: f32, ly: f32, half_w: f32, height: f32, blur: f32) -> f32 {
    let cx = abs(lx) - half_w;
    let cy = abs(ly - height * 0.5) - height * 0.5;
    let d = length(max(vec2<f32>(cx, cy), vec2<f32>(0.0))) + min(max(cx, cy), 0.0);
    return smoothstep(blur, -blur, d);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let aspect = uniforms.resolution.x / uniforms.resolution.y;
    let s = sin(uniforms.pos_size_rot.w);
    let c = cos(uniforms.pos_size_rot.w);

    if (uniforms.is_waveform == 1u) {
        let bg = get_vis_waveform(in.uv, s, c, aspect);
        let shadow_offset = vec2<f32>(0.005, 0.005) * uniforms.pos_size_rot.z;
        let shadow_bg = get_vis_waveform(in.uv - shadow_offset, s, c, aspect);
        
        let shadow_alpha = shadow_bg.a * 0.6;
        if (bg.a < 0.01 && shadow_alpha < 0.01) { discard; }
        
        let shadow_color = vec4<f32>(0.0, 0.0, 0.0, shadow_alpha);
        return mix(shadow_color, vec4<f32>(bg.rgb, 1.0), bg.a);
    }

    // --- INSTANCED BARS ---
    let height = in.bar_val;
    let is_linear = uniforms.shape == 1u;
    
    var bar_width = 0.0;
    
    if (is_linear) { // Linear
        let total_width = uniforms.pos_size_rot.z * aspect;
        bar_width = total_width / f32(uniforms.band_count);
    } else { // Circular
        let base_radius = uniforms.pos_size_rot.z + (uniforms.lyric_pulse * 0.02);
        let circumference = 6.2831853 * base_radius;
        bar_width = circumference / f32(uniforms.band_count * 2u);
    }
    
    let glow_pad_x = bar_width * 1.5;
    let glow_pad_y = 0.1 + uniforms.lyric_pulse * 0.05; 
    
    let max_height = uniforms.amplitude * 0.25 * (1.0 + uniforms.lyric_pulse * 1.5) + 0.05;
    let quad_w = bar_width + glow_pad_x * 2.0;
    let quad_h = max_height + glow_pad_y * 2.0;

    let local_x = (in.local_uv.x * quad_w) - (quad_w * 0.5);
    let local_y = (in.local_uv.y * quad_h) - glow_pad_y;

    let half_w = bar_width * 0.85 * 0.5;
    
    let glow_intensity = select(1.0, 10.0, is_linear);
    let pulse_mult = select(2.0, 1.0, is_linear);
    
    let fg = eval_shape(local_x, local_y, half_w, height, glow_intensity, pulse_mult);
    
    let shadow_screen = vec2<f32>(-0.005, -0.005) * uniforms.pos_size_rot.z;
    let shadow_local_x = select(
        -0.005 * uniforms.pos_size_rot.z,
        shadow_screen.x * aspect * c + shadow_screen.y * s,
        is_linear
    );
    let shadow_local_y = select(
        -0.005 * uniforms.pos_size_rot.z,
        -shadow_screen.x * aspect * s + shadow_screen.y * c,
        is_linear
    );
    
    // Soft SDF drop shadow exclusively for the solid bars (ignoring the glow)
    let shadow_alpha = eval_shadow(local_x + shadow_local_x, local_y + shadow_local_y, half_w, height, 0.015) * 0.6;
    let fg_alpha = fg.x + fg.y;
    
    if (fg_alpha < 0.01 && shadow_alpha < 0.01) { discard; }
    
    let gradient = clamp(local_y / height, 0.0, 1.0);
    let bar_color = mix(uniforms.color_bottom.rgb, uniforms.color_top.rgb, gradient);
    
    let final_fg_color = mix(uniforms.color_top.rgb * fg.y, bar_color, fg.x);
    let solid_alpha = select(1.0, 0.95, is_linear);
    let final_fg_alpha = min(fg.x * solid_alpha + fg.y, 1.0);
    
    let shadow_color = vec4<f32>(0.0, 0.0, 0.0, shadow_alpha);
    return mix(shadow_color, vec4<f32>(final_fg_color, 1.0), final_fg_alpha);
}