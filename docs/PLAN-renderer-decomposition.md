# Plan: Renderer decomposition

Goal: split the ~100-field `Renderer` god struct and the ~850-line
`draw_frame` into owned subsystems with local invariants, so cross-cutting
state bugs (like the 2026-07-19 stale blur chain, which lived in the gap
between `current_album_texture` and `album_blur_chain`) become structurally
hard to write. **Each phase is one self-contained commit** and leaves the
tree green (`cargo fmt --all --check && cargo clippy --all-targets --
-D warnings && cargo test`), plus the live-capture check below.

Anchors verified against master @ `f6fe54c` (post-1.2.1). Re-grep before
editing if the tree has moved — line numbers drift, the code names won't.

Phases are ordered by risk: 1–2 are pure-code, testable extractions; 3 moves
GPU resource ownership; 4–5 restructure the draw path itself. Do them in
order — phase 5 assumes the subsystem structs from 2–3 exist, and phase 4's
offscreen harness is what makes 5 safe to verify.

---

## Acceptance harness (used after every phase)

Until phase 4 lands an offscreen renderer, verification is the live-session
recipe proven during the Kawase port (2026-07-18):

1. Quit the running engine via the tray's busctl recipe (StatusNotifierItem
   menu Event, id 3 — see the nightly-SIGSEGV notes / gui `StopEngine`).
2. Run the debug build against a fixed config: frosted glass on, a known
   track playing (fixed art), weather off, fps 30.
3. `cosmic-screenshot` before/after the refactor commit; compare
   wallpaper-only regions (numpy mean-absolute-diff < 1/255 per channel —
   text antialiasing and the animated visualiser regions are masked out).
4. Exercise the event paths that reconfigure GPU state: track skip
   (same-size art), wallpaper colour swap, blur slider drag, monitor
   DPMS off/on.

Phase 4 replaces steps 2–3 with deterministic offscreen PNG comparison.

---

## Context primer (regions this plan touches)

| File | What it is |
|---|---|
| `renderer/core/mod.rs:90-193` | The `Renderer` struct: ~100 fields spanning GPU core, text, visualiser/audio analysis, album art, background, weather, lyrics scroll, cached colours/strings, and loop bookkeeping. **Phases 2–3 hollow it out.** |
| `renderer/draw.rs:112-399` | `draw_frame`'s per-frame precompute: ~280 lines of locals (`has_audio`, colour lerps, `vis_pos_size_rot`, fg-art transform constants, lyric window indices...). Pure data derivation. **Phase 1.** |
| `renderer/draw.rs:401-960` | The per-output loop: surface acquire, uniform writes gated by `last_uniform_res`, text shaping gated by `last_text_params`, then the encode block (`:879-959`). **Phases 4–5.** |
| `renderer/draw.rs:22-27` | The comment explaining why `prepare_text_buffer` takes three fields instead of `&mut Renderer` — the borrow-checker friction that subsystem structs dissolve. |
| `renderer/core/events.rs:245-380` | `Event::AudioFrame` handling: beat/treble detection, band smoothing, waveform peaks — pure math over 8 `Renderer` fields. **Phase 2.** |
| `renderer/core/updates.rs` | Texture upload + bind-group/blur-chain rebuild methods (`update_album_art_texture`, `rebuild_album_bind_groups`, `load_custom_background_from_image`, ...) and colour/weather caches. **Phase 3 moves these onto the layer structs.** |
| `renderer/blur.rs` | `BlurChain`/`KawaseBlur` — already the model: owned resources, constructor invariant, documented contract (`matches_source`). |
| `renderer/text.rs`, `visualiser_pass.rs` | Already-extracted subsystems; phases 2–3 follow their shape. |
| `renderer/core/mod.rs:195-495` | The run loop: dirty-tracking, physics decay, lyric index tracking, art-fade deadline. Stays put; shrinks as field owners move. |

Notes that save dead ends:

- **`last_uniform_res` is not per-monitor state.** It exists so same-resolution
  monitors share one uniform write per frame (`draw.rs:429`, `:452`, `:517`).
  When phase 5 splits the loop, this dedup must stay frame-scoped, not move
  into per-output state, or multi-monitor uniform writes silently double.
- **Output index = window index.** `is_frame_pending(i)` and the scale-factor
  lookup couple `renderer.outputs` order to `wayland_manager.app_data.windows`
  order; the coupling is re-established on every `configuration_serial` bump
  (`core/mod.rs:253-290`). Don't "fix" it mid-refactor; it is load-bearing
  and correct.
- **`kawase_blur` (the pipelines) is shared** by both the album and custom-bg
  chains. It stays on `Renderer` (or a small `GpuCore`); the layer structs
  borrow it per call, as `BlurChain::run` already does.
- **The empty 1x1 `empty_texture`** from `create_album_art_pipeline` seeds
  `current_album_texture` at init (`core/init.rs:161`) but `current_album_size`
  stays `None` — `rebuild_album_bind_groups` early-returns on that asymmetry.
  Preserve it or replace it deliberately in phase 3, not by accident.
- **CI has no GPU.** The phase-4 harness is a local tool (llvmpipe /
  `VK_ICD_FILENAMES` software Vulkan works with wgpu); do not wire it into
  pr-build.yml — document the invocation in the harness module instead.

---

## Phase 1: Extract `FrameParams`

`draw.rs:112-399` becomes `FrameParams::compute(&Renderer, delta) ->
FrameParams`: a plain-data struct holding everything the per-output loop
reads (`has_audio`, `audio_energy`, colour pairs, `vis_*` uniforms input,
fg-art transform constants, lyric window bounds, text colours, alignment
mappings). `draw_frame` shrinks to `let p = FrameParams::compute(..)` plus
the loop. No behaviour change; unit tests cover the trickier derivations
(dock-art override, `has_audio` gating, lyric window clamping) by
constructing a `FrameParams` from synthetic field values.

## Phase 2: Extract `AudioAnalysis`

New module owning: `bass/treble_bin_range`, both moving averages, both
pulses, both cooldown instants, `audio_processing_bins`,
`waveform_bin_ranges`, `inv_smoothing`, `inv_target_len`,
`audio_max_energy`, `audio_base_energy`, plus the smoothed
`audio_bands`/`audio_waveform` buffers (currently on `AppState`).
`events.rs`'s AudioFrame arm becomes `analysis.ingest(&bands, &waveform)`;
the run loop's pulse decay becomes `analysis.decay(delta)`. This is the
most unit-testable chunk in the renderer: synthetic bass impulse → one beat
pulse with cooldown; silence → decay to exact 0.0 (the subnormal flush);
band count change → `reconfigure(bands)`.

## Phase 3: Extract `ArtLayer` and `BackgroundLayer`

`ArtLayer`: album texture + size + aspect + sampler + uniform buffers +
bind groups + blur chain + `art_fade`/`pending_art_deadline` + target/prev
colours + pad buffer. `BackgroundLayer`: custom-bg equivalents + ambient
pipeline resources + `current_bg` + avg colour. The `updates.rs` methods
move onto them; the texture-swap-drops-chain invariant from the 2026-07-19
fix becomes a private constructor rule (`set_texture()` is the *only* way
in, and it rebuilds chain + bind groups atomically). `update_text_colors`
stays on `Renderer` but reads through the layers' getters.

## Phase 4: Render-target abstraction + offscreen harness

Split the encode block (`draw.rs:879-959`) into a function taking
`&wgpu::TextureView` + `(width, height)` instead of acquiring from a
surface. Add a dev-only `--render-frame <out.png>` path (hidden flag or
example binary) that builds the renderer without Wayland, feeds a synthetic
scene (fixed art bytes, fixed bands, fixed clock), renders one frame to an
offscreen texture, and writes the PNG. Golden images live out-of-repo
(they're driver-sensitive); the harness prints a perceptual diff against a
`--compare` baseline. From here on, the acceptance harness is deterministic.

## Phase 5: Split the per-output loop

With phases 1–4 in place, `draw_frame` becomes orchestration:
`params.write_uniforms(queue, output)` (keeping the frame-scoped
`last_uniform_res` dedup), `text.prepare(params, output)` (keeping
`last_text_params`), `encode(view, params, ...)`. Target: draw.rs under
~300 lines, no function over ~100, and `prepare_text_buffer`'s
three-field workaround deleted in favour of `&mut TextSubsystem`.

---

## Explicitly out of scope

- Behaviour changes of any kind (bar polish has its own roadmap entry).
- The run loop's dirty-tracking/heartbeat logic (`core/mod.rs:221-494`) —
  it is subtle, recently touched, and orthogonal to the struct split.
- Multi-GPU / per-output device work.
