struct VisualiserUniforms {
    resolution: vec2<f32>,
    band_count: u32,
    lyric_pulse: f32,
    color_top: vec4<f32>,
    color_bottom: vec4<f32>,
    pos_size_rot: vec4<f32>,
    amplitude: f32,
    shape: u32, // 0=circular, 1=linear, 2=square
    time: f32,
    align: u32, // 0=left, 1=center, 2=right
    is_waveform: u32, // bool
    bar_width_ratio: f32, // bar width as a fraction of its allotted slot (was hardcoded 0.85)
    cap_radius: f32, // 0=hard rectangle, 1=full capsule/pill
    reflection: f32, // "glass floor" mirror strength below the baseline, 0=off
    led_segments: u32, // >0 chops each bar into this many LED-style segments
    peak_hold: u32, // bool: draw a gravity-falling peak cap per bar
    glow_strength: f32, // multiplier on the tip glow, 0=flat/crisp, 1=full glow
    _pad1: u32,
}

@group(0) @binding(0) var<uniform> uniforms: VisualiserUniforms;
@group(0) @binding(1) var<storage, read> bands: array<f32>;
@group(0) @binding(2) var<storage, read> peaks: array<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) local_uv: vec2<f32>,
    @location(2) bar_val: f32,
    @location(3) band_energy: f32, // this bar's own 0..1 band value, for energy-scaled glow
    @location(4) peak_val: f32, // this bar's gravity-held peak height, in the same units as bar_val
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

    if uniforms.is_waveform == 1u {
        if i_idx == 0u {
            out.clip_position = vec4<f32>(p_quad.x * 2.0 - 1.0, 1.0 - p_quad.y * 2.0, 0.0, 1.0);
            out.uv = p_quad;
        } else {
            out.clip_position = vec4<f32>(0.0);
        }
        return out;
    }

    if uniforms.shape == 1u {
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
        out.band_energy = val;
        out.peak_val = peaks[band_idx] * 0.25 * uniforms.amplitude;
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

    } else if uniforms.shape == 2u {
        // Square: bars walk the perimeter of a square instead of a ring,
        // reusing Circular's band-folding (norm_angle/f_band) so the two
        // "ring-like" shapes share the same audio-mapping feel and differ
        // only in geometry.
        let norm_angle = f32(i_idx) / f32(uniforms.band_count * 2u);

        var f_band = norm_angle * 2.0;
        if f_band > 1.0 { f_band = 2.0 - f_band; }

        let band_idx = min(u32(f_band * f32(uniforms.band_count)), uniforms.band_count - 1u);
        let val = bands[band_idx];

        let half_size = uniforms.pos_size_rot.z + (uniforms.lyric_pulse * 0.02);
        let max_height = uniforms.amplitude * 0.25 * (1.0 + uniforms.lyric_pulse * 1.5) + 0.05;
        let height = max(val, 0.02) * 0.25 * uniforms.amplitude * (1.0 + uniforms.lyric_pulse * 1.5);

        let circumference = 8.0 * half_size;
        let bar_width = circumference / f32(uniforms.band_count * 2u);

        let glow_pad_x = bar_width * 1.5;
        let glow_pad_y = 0.1 + uniforms.lyric_pulse * 0.05;

        let quad_w = bar_width + glow_pad_x * 2.0;
        let quad_h = max_height + glow_pad_y * 2.0;

        out.local_uv = p_quad;
        out.bar_val = height;
        out.band_energy = val;
        out.peak_val = peaks[band_idx] * 0.25 * uniforms.amplitude;
        out.uv = vec2<f32>(f_band, 0.0);

        let local_x = (p_quad.x * quad_w) - (quad_w * 0.5);
        let local_y = (p_quad.y * quad_h) - glow_pad_y;

        // Which of the square's four sides this instance sits on, and how
        // far along it (0..1), walking clockwise from the top-left corner.
        let perimeter = norm_angle * 4.0;
        let side = min(u32(perimeter), 3u);
        let side_frac = perimeter - f32(side);

        var base_pos = vec2<f32>(0.0, 0.0);
        var normal = vec2<f32>(0.0, 0.0);
        if side == 0u {
            base_pos = vec2<f32>(side_frac * 2.0 - 1.0, -1.0);
            normal = vec2<f32>(0.0, -1.0);
        } else if side == 1u {
            base_pos = vec2<f32>(1.0, side_frac * 2.0 - 1.0);
            normal = vec2<f32>(1.0, 0.0);
        } else if side == 2u {
            base_pos = vec2<f32>(1.0 - side_frac * 2.0, 1.0);
            normal = vec2<f32>(0.0, 1.0);
        } else {
            base_pos = vec2<f32>(-1.0, 1.0 - side_frac * 2.0);
            normal = vec2<f32>(-1.0, 0.0);
        }
        base_pos = base_pos * half_size;

        // Tangent runs along the side, perpendicular to the outward
        // normal - the square counterpart of Circular's `-sin/cos` pair.
        let tangent = vec2<f32>(-normal.y, normal.x);
        let p = base_pos + tangent * local_x + normal * local_y;

        let s = sin(uniforms.pos_size_rot.w);
        let c = cos(uniforms.pos_size_rot.w);
        let p_rot = vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);

        let aspect = uniforms.resolution.x / uniforms.resolution.y;
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
        out.band_energy = val;
        out.peak_val = peaks[band_idx] * 0.25 * uniforms.amplitude;
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

// Signed distance to a box centred at (0, height/2) with half-extents
// (half_w, height/2) and uniform corner radius `radius`. At radius=0 this is
// an exact rectangle (bytewise the pre-capsule bar shape); as radius grows
// towards min(half_w, height/2) it rounds into a full capsule/pill once the
// bar is taller than it is wide. Standard "rounded box" SDF (Inigo Quilez).
fn sd_round_box(lx: f32, ly: f32, half_w: f32, height: f32, radius: f32) -> f32 {
    let p = vec2<f32>(lx, ly - height * 0.5);
    let b = vec2<f32>(half_w, height * 0.5) - vec2<f32>(radius);
    let q = abs(p) - b;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

// Body + glow for one bar, as a rounded-box SDF with real (derivative-based)
// anti-aliasing on every edge - corners, sides, and top alike - rather than
// the old hard per-axis cutoff. `glow_intensity`/`pulse_mult` shape how far
// and how brightly the glow reaches past the body; `energy` is this bar's
// own 0..1 band value, so a loud bar's glow reads brighter than a quiet
// bar's even under the same global lyric pulse.
fn eval_shape(lx: f32, ly: f32, half_w: f32, height: f32, cap_radius_ratio: f32, glow_intensity: f32, pulse_mult: f32, energy: f32) -> vec2<f32> {
    let radius = clamp(cap_radius_ratio, 0.0, 1.0) * min(half_w, height * 0.5);
    let d = sd_round_box(lx, ly, half_w, height, radius);

    let aa = max(fwidth(d), 1e-5);
    let body = smoothstep(aa, -aa, d);

    // Glow stays confined directly above the bar's own footprint (vertical
    // distance past its tip only, never sideways) - the same shape the
    // pre-capsule glow used. A full SDF-distance glow would bleed around
    // the rounded sides into neighbouring bars' gaps, washing a ring of
    // many thin bars into one smooth haze instead of distinct spikes.
    if abs(lx) > half_w || ly <= height {
        return vec2<f32>(body, 0.0);
    }

    let glow_dist = ly - height;
    // Capped at 1.0 (never brighter than the original, energy-agnostic
    // glow) rather than boosting loud bars above it: boosting would push
    // the glow's visible reach past `glow_pad_y`, the fixed quad padding
    // sized for the old always-on multiplier, and hard-clip at the quad's
    // own edge instead of fading out.
    let energy_boost = clamp(0.3 + energy * 0.7, 0.0, 1.0);
    let glow = clamp(0.005 / (glow_dist * glow_dist * glow_intensity + 0.005) - 0.1, 0.0, 1.0)
        * (1.0 + uniforms.lyric_pulse * pulse_mult) * energy_boost * uniforms.glow_strength;

    return vec2<f32>(body, glow);
}

fn eval_shadow(lx: f32, ly: f32, half_w: f32, height: f32, blur: f32) -> f32 {
    let cx = abs(lx) - half_w;
    let cy = abs(ly - height * 0.5) - height * 0.5;
    let d = length(max(vec2<f32>(cx, cy), vec2<f32>(0.0))) + min(max(cx, cy), 0.0);
    return smoothstep(blur, -blur, d);
}

// Carves evenly-spaced gaps into `body_alpha` for the LED/segmented mode:
// `segments` equal-pitch slots spanning the bar's full travel range
// (0..max_height, not just its current height), so gaps line up across
// bars regardless of how tall each one currently is - the classic VU-meter
// look. A no-op (returns `body_alpha` unchanged) when segments is 0.
fn apply_led_segments(body_alpha: f32, ly: f32, max_height: f32, segments: u32) -> f32 {
    if segments == 0u {
        return body_alpha;
    }
    let pitch = max_height / f32(segments);
    let gap = pitch * 0.22;
    let seg_local = ly - floor(ly / pitch) * pitch;
    let aa = max(fwidth(ly), 1e-5);
    let half_gap = gap * 0.5;
    let past_gap = smoothstep(half_gap - aa, half_gap + aa, seg_local);
    let before_gap = smoothstep(pitch - half_gap + aa, pitch - half_gap - aa, seg_local);
    return body_alpha * past_gap * before_gap;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let aspect = uniforms.resolution.x / uniforms.resolution.y;
    let s = sin(uniforms.pos_size_rot.w);
    let c = cos(uniforms.pos_size_rot.w);

    if uniforms.is_waveform == 1u {
        let bg = get_vis_waveform(in.uv, s, c, aspect);
        let shadow_offset = vec2<f32>(0.005, 0.005) * uniforms.pos_size_rot.z;
        let shadow_bg = get_vis_waveform(in.uv - shadow_offset, s, c, aspect);

        let shadow_alpha = shadow_bg.a * 0.6;
        if bg.a < 0.01 && shadow_alpha < 0.01 { discard; }

        let shadow_color = vec4<f32>(0.0, 0.0, 0.0, shadow_alpha);
        return mix(shadow_color, vec4<f32>(bg.rgb, 1.0), bg.a);
    }

    // --- INSTANCED BARS ---
    let height = in.bar_val;
    let is_linear = uniforms.shape == 1u;

    var bar_width = 0.0;

    if is_linear { // Linear
        let total_width = uniforms.pos_size_rot.z * aspect;
        bar_width = total_width / f32(uniforms.band_count);
    } else if uniforms.shape == 2u { // Square
        let half_size = uniforms.pos_size_rot.z + (uniforms.lyric_pulse * 0.02);
        let circumference = 8.0 * half_size;
        bar_width = circumference / f32(uniforms.band_count * 2u);
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

    let half_w = bar_width * clamp(uniforms.bar_width_ratio, 0.05, 1.0) * 0.5;

    let glow_intensity = select(1.0, 10.0, is_linear);
    let pulse_mult = select(2.0, 1.0, is_linear);

    var fg = eval_shape(local_x, local_y, half_w, height, uniforms.cap_radius, glow_intensity, pulse_mult, in.band_energy);
    fg.x = apply_led_segments(fg.x, local_y, max_height, uniforms.led_segments);
    // Which height this pixel's colour gradient should sample at - normally
    // its own local_y, but the mirrored height when it's inside the
    // reflection below, so the reflection shows the bar's real top-to-
    // bottom gradient flipped rather than flattening to solid color_bottom.
    var color_y = local_y;

    // Mirror reflection ("glass floor"): below the baseline, re-evaluate
    // the same bar shape as if reflected across ly=0 and fade it out with
    // depth and the theme's reflection strength. Reuses eval_shape as-is -
    // a point at depth d below the floor mirrors the bar's own appearance
    // at height d above it.
    if uniforms.reflection > 0.0 && local_y < 0.0 {
        let mirrored_y = -local_y;
        var refl = eval_shape(local_x, mirrored_y, half_w, height, uniforms.cap_radius, glow_intensity, pulse_mult, in.band_energy);
        refl.x = apply_led_segments(refl.x, mirrored_y, max_height, uniforms.led_segments);
        let depth_fade = clamp(1.0 - (mirrored_y / (height + 0.15)), 0.0, 1.0);
        let refl_alpha = uniforms.reflection * depth_fade * depth_fade;
        fg = vec2<f32>(refl.x * refl_alpha, refl.y * refl_alpha);
        color_y = mirrored_y;
    } else if local_y < 0.0 {
        fg = vec2<f32>(0.0, 0.0);
    }

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

    // Peak-hold cap: a thin bright sliver at this bar's gravity-held peak
    // height, slightly wider than the bar itself (classic VU-meter look).
    // Drawn after the shadow/glow test above so it can still appear in the
    // otherwise-empty glow region above the current bar height.
    var peak_alpha = 0.0;
    if uniforms.peak_hold == 1u && local_y >= 0.0 {
        let cap_half_thickness = max(glow_pad_y * 0.12, 0.004);
        let d_peak = abs(local_y - in.peak_val) - cap_half_thickness;
        let aa = max(fwidth(d_peak), 1e-5);
        let in_band = smoothstep(aa, -aa, d_peak);
        let in_width = smoothstep(half_w * 1.2 + aa, half_w * 1.2 - aa, abs(local_x));
        peak_alpha = in_band * in_width;
    }

    if fg_alpha < 0.01 && shadow_alpha < 0.01 && peak_alpha < 0.01 { discard; }

    let gradient = clamp(color_y / height, 0.0, 1.0);
    let bar_color = mix(uniforms.color_bottom.rgb, uniforms.color_top.rgb, gradient);

    let final_fg_color = mix(uniforms.color_top.rgb * fg.y, bar_color, fg.x);
    let solid_alpha = select(1.0, 0.95, is_linear);
    let final_fg_alpha = min(fg.x * solid_alpha + fg.y, 1.0);

    let shadow_color = vec4<f32>(0.0, 0.0, 0.0, shadow_alpha);
    let base = mix(shadow_color, vec4<f32>(final_fg_color, 1.0), final_fg_alpha);

    // Composite the peak cap on top, bright (biased toward white) so it
    // reads clearly against both the bar's own colour and the background.
    let peak_color = mix(uniforms.color_top.rgb, vec3<f32>(1.0), 0.5);
    return mix(base, vec4<f32>(peak_color, 1.0), peak_alpha);
}
