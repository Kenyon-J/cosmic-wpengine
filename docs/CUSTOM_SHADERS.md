# Writing a custom visualiser shader

A theme's `[visualiser]` table can point `shader = "my_shader.wgsl"` at a
file of your own next to it in `~/.config/cosmic-wallpaper/shaders/`,
completely replacing the built-in bar/ring rendering with your own WGSL.
This doc is the full contract that shader has to meet, plus a tour of the
shipped shader's own techniques so you have something concrete to start
from.

If you just want to *use* the existing shapes and effects (capsule bars,
reflection, LED segments, peak-hold, ...), you don't need any of this -
see [THEMES.md](THEMES.md). This doc is for replacing the shader itself.

## The pipeline, in one paragraph

Every frame, for the theme's `[visualiser]` element, the engine runs one
WGSL module with two entry points - `vs_main` (vertex) and `fs_main`
(fragment) - through a single render pass with alpha blending on. For the
three bar/ring shapes (`circular`, `linear`, `square`) it draws `N`
*instances* of the same 6-vertex quad, one per audio band (`N` doubled for
the two ring-like shapes - see below); for the special `waveform` style it
draws a single full-screen quad instead and does everything per-pixel in
the fragment shader. Your shader can do either, or something else entirely
- the engine only cares that `vs_main` and `fs_main` exist with the right
signatures and that the bind group below is honoured.

## Bind group layout

```wgsl
@group(0) @binding(0) var<uniform> uniforms: VisualiserUniforms;
@group(0) @binding(1) var<storage, read> bands: array<f32>;
@group(0) @binding(2) var<storage, read> peaks: array<f32>;
```

`bands` and `peaks` are both `audio.bands`-length (64 by default,
configurable) arrays indexed by *band number*, not by instance index -
see [Band indexing](#band-indexing-and-shape-folding) below for how the
two relate per shape. Binding 2 (`peaks`) is optional to use - the engine
always binds it, so a shader that doesn't declare it in its own `struct`
simply never reads it. This is also why old custom shaders never break: a
shader that only knows about binding 0 and 1 keeps working exactly as
before after an engine update adds more to the uniform struct or another
binding.

## The uniform struct

```wgsl
struct VisualiserUniforms {
    resolution: vec2<f32>,     // offset 0:  render target size in pixels
    band_count: u32,           // offset 8:  length of `bands`/`peaks`
    lyric_pulse: f32,          // offset 12: beat pulse, see Triggers below
    color_top: vec4<f32>,      // offset 16: [visualiser].color_top (or album palette)
    color_bottom: vec4<f32>,   // offset 32: [visualiser].color_bottom (or album palette)
    pos_size_rot: vec4<f32>,   // offset 48: (pos.x, pos.y, size, rotation_radians)
    amplitude: f32,            // offset 64: [visualiser].amplitude
    shape: u32,                // offset 68: 0=circular, 1=linear, 2=square
    time: f32,                 // offset 72: elapsed seconds, for scrolling/animated effects
    align: u32,                // offset 76: 0=left, 1=center, 2=right (linear band ordering)
    is_waveform: u32,          // offset 80: bool - see Triggers below
    bar_width_ratio: f32,      // offset 84: [visualiser].bar_width_ratio
    cap_radius: f32,           // offset 88: [visualiser].cap_radius
    reflection: f32,           // offset 92: [visualiser].reflection
    led_segments: u32,         // offset 96: [visualiser].led_segments
    peak_hold: u32,            // offset 100: bool, [visualiser].peak_hold
    glow_strength: f32,        // offset 104: [visualiser].glow_strength
    _pad: u32,                 // offset 108: padding to a 112-byte, 16-aligned struct
}
```

**This struct only ever grows by appending fields**, never by reordering or
removing them, specifically so old custom shaders don't break. If you only
care about the original handful of fields, you can declare a shorter
struct ending at `is_waveform` (with a fitting pad to keep the struct size
a multiple of 16 bytes) and ignore everything the engine has added since -
wgpu only requires the bind group's declared size be *at least* what your
struct asks for, so a short struct against a bigger buffer is fine.

`color_top`/`color_bottom` arrive as `vec4`, but only `.rgb` is meaningful
(alpha is always 1.0) - when the theme leaves them unset, these are the
adaptive colours sampled from the current album art/wallpaper, already
resolved by the time they reach the shader.

## Triggers

These are the values that actually change from frame to frame or track to
track - the things worth reacting to:

- **`lyric_pulse`** - snaps to `1.0` the instant the bass-band energy
  spikes above its own recent rolling average (a beat), then decays
  exponentially (`e^(-12·Δt)`, flushed to exact `0.0` once negligible).
  Scale a size/glow/rotation by this for a beat-reactive kick. The
  built-in shader uses it both as a size pulse and as a glow-intensity
  multiplier.
- **`bands[i]` / `peaks[i]`** - per-band energy, see the next section for
  exactly how these numbers are derived. `bands` is the live, smoothed
  value; `peaks` is the same but gravity-held (see below) - read it for a
  calmer "recent maximum" signal instead of the live value.
- **`time`** - elapsed seconds since the pass started, monotonic. Use for
  anything that should animate independent of audio (scrolling textures,
  ambient rotation).
- **`is_waveform`** - `1` when the *engine* is running in its special
  waveform audio style (`audio.style = "waveform"` in `config.toml`),
  not a per-theme shape choice. When it's `1`, `vs_main` is expected to
  draw a single full-screen quad (instance 0 only - see the shipped
  shader's `vs_main` for the exact pattern) and do the actual waveform
  line-drawing per-pixel in `fs_main`; the three bar/ring shapes below
  never run in this mode.
- **`@builtin(instance_index)`** in `vs_main` - which band (or, for
  ring-like shapes, which half of the ring) this instance is. This is
  your only way to know "which bar am I" - there's no separate per-band
  uniform array beyond `bands`/`peaks` themselves.

## How band energy is actually computed

Before any of this reaches the shader, `AudioAnalysis` (in
`src/modules/renderer/audio_analysis.rs`) turns a raw ~1024-bin FFT
spectrum into the `band_count`-length arrays the shader sees:

1. **Log-frequency bins**: `band_count` bands are spaced logarithmically
   from 40Hz to 16kHz (not linearly) - low bands cover a narrow slice of
   bass frequencies, high bands each cover a much wider slice of treble,
   matching how audio visualisers are conventionally banded.
2. **A-weighting**: each band's raw magnitude is scaled by a standard
   A-weighting curve before anything else - bass and extreme treble are
   attenuated relative to the mid-range the human ear is most sensitive
   to. A raw bass bin at full amplitude can still end up a small `bands[]`
   value after this - that's expected, not a bug, if you're feeding
   synthetic test data and wondering why low bands look quiet.
3. **Asymmetric smoothing**: a rising value snaps most of the way to its
   target in a single frame (80% per tick); a falling value eases down at
   the rate set by `audio.smoothing` in `config.toml` (`0.0` instant,
   `1.0` very slow). This is why bars punch up sharply on a hit but settle
   back down smoothly.
4. **Peak-hold gravity**: independently, `peaks[i]` snaps up to match
   `bands[i]` instantly whenever the live value reaches or exceeds it,
   otherwise falls under constant acceleration (`PEAK_GRAVITY = 3.2`
   units/s²) every render tick - continuously, not just when new audio
   arrives, so the fall stays smooth between FFT frames.

None of this is configurable per-shader - it's the same pipeline
regardless of which shader is loaded. If you want a fundamentally
different response curve (e.g. linear bands, no A-weighting), you'd need
to change `audio_analysis.rs` itself, which is outside a custom shader's
reach by design (that pipeline is shared with the beat/treble detectors,
not visualiser-specific).

## Band indexing and shape folding

For `linear`, instance `i` maps directly to band `i` (or its mirror/
reverse, depending on `align`). For `circular` and `square`, the engine
draws `band_count * 2` instances that walk the shape's full perimeter and
*fold* the band index in the middle, so band 0 sits at both ends of the
ring/square and the loudest available band-mapping sits opposite - this
gives the "ring that breathes symmetrically" look rather than a hard seam
where band 63 sits next to band 0. The shipped shader's `norm_angle`/
`f_band` calculation in `vs_main` is the reference implementation if
you're building your own ring-like shape.

## Geometry contract

`vs_main(@builtin(vertex_index) v_idx: u32, @builtin(instance_index) i_idx: u32) -> VertexOutput`
must set `@builtin(position) clip_position`. The shipped shader's
`VertexOutput` also carries `uv`, `local_uv`, `bar_val`, `band_energy` and
`peak_val` through to the fragment stage - your own struct can carry
whatever your `fs_main` needs, as long as the two agree.

A few things worth knowing before you reinvent them:

- The 6-vertex array `pos[6]` at the top of `vs_main` is just two
  triangles covering a unit quad (`0.0..1.0` in both axes) - every
  instance reuses it and repositions/rescales it in local space before
  the final rotation and `pos_size_rot` offset.
- `pos_size_rot.w` is rotation in **radians** (the TOML/GUI value is
  degrees; the engine converts before it reaches the shader).
- Aspect correction (`resolution.x / resolution.y`) is applied to the
  *horizontal* axis only - heights/amplitudes are already in "fraction of
  screen height" units and need no correction.

## Fragment contract

`fs_main(in: VertexOutput) -> @location(0) vec4<f32>` - a straight
(non-premultiplied) RGBA colour; the pipeline's blend state is standard
source-over alpha blending. `discard;` is fine and expected for the empty
majority of each instance's bounding quad (the quad is sized generously to
fit each bar's glow/reflection halo, not just its solid body).

## A minimal custom shader

This ignores almost everything above and just draws a plain rectangle per
band, to show the smallest shader that actually works - copy the shipped
`src/modules/visualiser.wgsl` instead if you want capsule caps, glow,
reflection, etc. as a starting point rather than from scratch.

```wgsl
struct VisualiserUniforms {
    resolution: vec2<f32>,
    band_count: u32,
    lyric_pulse: f32,
    color_top: vec4<f32>,
    color_bottom: vec4<f32>,
    pos_size_rot: vec4<f32>,
    amplitude: f32,
    shape: u32,
    time: f32,
    align: u32,
    is_waveform: u32,
    _pad: u32, // ignoring every field this engine has added since - see above
}

@group(0) @binding(0) var<uniform> uniforms: VisualiserUniforms;
@group(0) @binding(1) var<storage, read> bands: array<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) height: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) v_idx: u32, @builtin(instance_index) i_idx: u32) -> VertexOutput {
    var out: VertexOutput;
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let p = pos[v_idx];

    let norm_x = f32(i_idx) / f32(uniforms.band_count);
    // max(val, 0.02), not just val: see "Always floor your height above
    // zero" below - some bands legitimately read ~0.0 at any given moment,
    // and a literal zero height on even one of the many instances sharing
    // this draw call can blank out the whole batch on some drivers.
    let val = max(bands[i_idx], 0.02);
    let height = val * 0.3 * uniforms.amplitude;
    out.height = height;

    let bar_w = 1.0 / f32(uniforms.band_count);
    let x = uniforms.pos_size_rot.x - 0.5 + norm_x + p.x * bar_w;
    let y = uniforms.pos_size_rot.y - p.y * height;
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return uniforms.color_top;
}
```

## Iterating without a live desktop session

The engine binary has a hidden offscreen-render flag, handy for iterating
on a shader without needing to watch your actual desktop or have music
playing:

```bash
cosmic-wallpaper --render-frame out.png --style my_theme
```

It builds a fixed synthetic scene (a deterministic ramped spectrum, a
placeholder track and album art) and renders exactly one frame to
`out.png`, so you can inspect the result immediately after each edit.
Add `--compare baseline.png` to also get a pass/fail pixel-difference
report against a saved-off reference image.

## Troubleshooting

- **My shader isn't showing up / reverted to the default look.** A shader
  that fails to compile is logged (`tracing::error!` with the validation
  message) and the engine falls back to the previous working shader
  rather than crashing. Check the engine's log output for `Shader
  validation error`.
- **Edits aren't live-reloading.** The engine only recompiles the pipeline
  when the shader's *source bytes* actually changed (WGSL compilation is
  comparatively expensive) - make sure you saved, and that `shader =` in
  the theme's `.toml` points at the exact file name you're editing.
- **`fwidth`/`dpdx`/`dpdy` don't work in a function I called from
  `vs_main`.** These are fragment-stage-only in WGSL; they're fine to call
  from a plain `fn` as long as that function is only ever reached from
  `fs_main`.
- **All my bars vanished, and there's no error logged at all.** Always
  floor a band value above exactly zero before multiplying it into a
  position (`max(bands[i], 0.02)`, not the raw value) - confirmed on this
  project's own dev hardware (AMD RADV/Vulkan) that a literal `0.0` height
  on even a handful of the many instances sharing one instanced draw call
  can blank out the *entire* draw call, not just the degenerate instances,
  with no validation error to point at why. The shipped shader has always
  done this (`max(val, 0.02)`) for exactly this reason. Cheap insurance:
  floor every value you read out of `bands`/`peaks` before it touches
  `clip_position`, even if it looks harmless.
