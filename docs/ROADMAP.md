# Roadmap

Post-1.0 ideas and deferred work, roughly in intended order. Items graduate
into a `PLAN-*.md` when they're actually scheduled.

## Settings GUI redesign

The settings window has accumulated enough toggles to feel cluttered; rework
the layout (grouping/pages, progressive disclosure of advanced options)
before adding more controls to it.

Fold into the redesign:

- **Text colour picker** — manual override for the adaptive text colour. The
  automatic logic samples the wallpaper's mean colour, which picks a poor
  compromise on high-contrast wallpapers (e.g. half black / half white
  averages to grey wherever the text sits). A region-aware sample under the
  theme's text positions is a possible alternative, but a user override is
  the predictable escape hatch.

## Live wallpaper feature additions (planned — see PLAN-live-wallpaper-features.md)

Four features identified 2026-07-20 that fit this project's existing
architecture without new infrastructure: wallpaper rotation/playlist, an
independent custom-image picker, pause (throttle) on battery via UPower,
and per-monitor wallpaper identification (groundwork only - the full
per-monitor feature is bigger and gets its own plan once the identification
groundwork lands). See
[PLAN-live-wallpaper-features.md](PLAN-live-wallpaper-features.md) for
phases, file anchors, and why system76-power was considered and rejected
for the battery detection (not running on non-Pop!_OS COSMIC installs;
confirmed absent on this project's own dev machine).

## Renderer decomposition (done — see PLAN-renderer-decomposition.md)

The ~100-field `Renderer` + ~850-line `draw_frame` split (Phase 9 of
[PLAN-v1-hardening.md](PLAN-v1-hardening.md)). Graduated to
[PLAN-renderer-decomposition.md](PLAN-renderer-decomposition.md) 2026-07-19:
five phases, frame-capture harness in phase 4, each phase one green commit.

Phases 1 (`FrameParams`, `825c108`) and 2 (`AudioAnalysis`, `899979b`)
shipped right after v1.2.2. Phase 3 (`ArtLayer`/`BackgroundLayer`, `96c7d65`)
landed 2026-07-21: both album-art and custom-background
texture/blur-chain/bind-group state moved off the `Renderer` god-struct into
their own structs, each with `set_texture()` as the sole way to install a new
source texture — the private-constructor invariant the plan called for, so
the 2026-07-19 stale-blur-chain bug class can't recur structurally. Verified
with `cargo fmt`/`clippy -D warnings`/`test` (all green) plus a live smoke
test on the real desktop (real MPRIS track, album art, lyrics, visualiser,
weather all rendering correctly - no offscreen harness exists yet, so this
was eyeballed via `cosmic-screenshot`, not a pixel diff).

Phase 4 (render-target abstraction + offscreen harness) landed 2026-07-21:
`draw.rs`'s per-output loop body is now two standalone functions -
`write_frame_uniforms()` and `encode_frame()` - taking individual GPU
resources and a bare `&wgpu::TextureView`/`(width, height)` instead of
`&Renderer` and a live surface (same borrow-checker reason as
`prepare_text_buffer`: the per-output loop already holds `renderer.outputs`
mutably borrowed via its iterator). `Renderer::new_headless()` builds a
renderer with no Wayland surfaces at all (adapter requested with no
`compatible_surface`, a fixed render-target format); the new hidden
`--render-frame <out.png> [--compare <baseline.png>]` engine flag
(`modules::renderer::render_frame_to_png`, `renderer/harness.rs`) uses it to
render one frame against a fixed synthetic scene (a known track with a
synthetic checkerboard album art, a fixed audio spectrum) and writes it to a
PNG, diffing against a baseline when `--compare` is given
(mean-absolute-difference per channel, matching the live-verification
threshold this replaces). Verified for real: the harness's own rendered PNG
shows the expected scene (checkerboard art + visualiser bars over the
ambient sky - text isn't drawn yet, since `TextRenderer::prepare_text` is
still only called from the live per-output loop), a self-compare passes
(mean 0.0000/255), and a compare against a deliberately-different image
fails with a nonzero exit code - plus `fmt`/`clippy`/`test` all green and a
live smoke test on the real desktop.

Phase 5 (split the per-output loop) landed 2026-07-21, completing the plan:
text shaping/caching/rendering state (`font_system`, `swash_cache`,
`text_renderer`, the buffer cache, and the pending-buffers list) is now its
own `TextSubsystem` (new `core/text_subsystem.rs`), replacing
`prepare_text_buffer`'s three-separate-fields workaround with `&mut
TextSubsystem` as the plan called for. The ~230-line lyric/track-info/
weather shaping block that used to live inline in `draw_frame` is now
`TextSubsystem::prepare()`; `draw_frame`'s per-output loop calls it
alongside `write_frame_uniforms()`/`encode_frame()` rather than containing
the logic itself. Verified the strongest way available: rendered a frame
with the harness *before* this change, rendered again *after*, and
`--compare`d them - mean 0.0000/255, zero pixels different - plus
`fmt`/`clippy`/`test` all green and a live smoke test (steady operation
through real MPRIS track changes, which exercise the exact code path
feeding `TextSubsystem::prepare()`, though a visual screenshot of the text
itself was blocked both attempts by occluding windows on the live desktop -
worth a manual glance).

Not fully hit: the plan's line-count targets (`draw.rs` under ~300 lines,
no function over ~100). It landed at 655 lines - `draw_frame` itself is
~330 and `write_frame_uniforms` ~170, both still above the ~100 target.
Splitting `write_frame_uniforms` into per-buffer-kind functions
(visualiser/album-art/background) would close most of that gap; not done
here for risk/effort reasons given the architectural goals (borrow-checker-
friendly decomposition, the offscreen harness, `TextSubsystem`) were already
achieved and pixel-verified. Fair game for a future pass if it's worth
doing on its own.

With all five phases done, the renderer's own architecture no longer blocks
anything on the "theme packs" roadmap below - though see the correction in
that section: runtime shader loading turned out to already exist
independently of this work, so it was never actually the blocker it was
first thought to be.

## 1.2.x — first-run desktop integration (in progress)

Manual installs (release tarball, and every install thereafter kept alive by
the self-updater) are just two binaries in `~/.local/bin`: nothing installs
the `.desktop` entry or icons, so the app never appears in COSMIC's app
library/launcher — tray and terminal are the only ways in (gap found
2026-07-19 while adding the app icon). On GUI startup, bootstrap the
integration for exactly those installs:

- Skip entirely under Flatpak (`/.flatpak-info`) and for package-managed
  binaries under `/usr` — those installs ship the files themselves
- Write `~/.local/share/applications/<app-id>.desktop` from the repo's
  canonical entry with an absolute `Exec` (launcher sessions don't reliably
  have `~/.local/bin` on PATH) when it's missing, or when its `Exec` points
  at a binary that no longer exists (healing moved installs without
  clobbering user edits)
- Install the embedded icon set into `~/.local/share/icons/hicolor/...`,
  rewriting on content change so icon updates propagate

## 1.2 — "The Themes Release" (SHIPPED as v1.2.0/v1.2.1, 2026-07-19)

Turn the engine's live TOML reload into the product's signature feature:
the desktop itself is the theme editor's preview. Approved direction
2026-07-18; interactive mockup exists (ask Joshua for the artifact link).

1. **Live theme editor** — the Layout Themes page grows per-element
   controls (album art, track info, lyrics, visualiser, weather) mapped
   1:1 onto `ThemeLayout` fields: position sliders, size/rotation/
   amplitude, shape and align toggles, effects (lyric bounce/spring,
   beat pulse). Every change debounce-writes the theme TOML; the engine's
   live reload shows it on the real desktop instantly. Built with the
   same libcosmic widget vocabulary as the rest of the app
   (`settings::section` rows, dropdowns, native sliders) — the HTML
   mockup is a wireframe, not a styling target.
   - Includes a NEW `TextLayout.size` scale field (serde-default 1.0):
     lyric/track/weather font sizes are currently hardcoded in draw.rs
     (`logical_height * 0.04` for lyrics); the theme value multiplies in,
     giving each text element a Size slider.
2. **Full starter template** — `Create Theme` writes the complete default
   layout with every key commented, not the current 6-line stub.
3. **`docs/THEMES.md`** — every field, range, default; annotated examples.
4. **Sharing** — "Import Theme..." button; `themes/` gallery directory in
   the repo for community layouts.
5. **Engine status row** — Settings → General shows running/stopped with a
   Start/Stop button (gap found 2026-07-18: GUI had no way to restart a
   quit engine).

## Theme packs (bundle sharing) — SHIPPED 2026-07-21

Extends the 1.2 Themes Release's "Import Theme" / gallery model from a bare
layout TOML into a full pack: background video, visualiser layout, and
(optionally) a custom `.wgsl` shader, bundled so a pack works like a
Wallpaper Engine workshop item — one file to install a whole look. Idea
from Joshua 2026-07-21; built the same day, full shader support from the
start (see the correction below on why no staging was needed).

Lives on its own **Packs** page (Joshua asked for this mid-implementation
rather than folding it into Layout Themes, to keep that page from getting
cluttered) with an Export section (pick a theme, `Export Pack` writes
`~/.config/cosmic-wallpaper/packs/<name>.cwtheme`) and an Import drop
zone. `src/modules/config/pack.rs` holds the format: `PackManifest`/
`PackContents`/`ParsedPack`, `build()`/`parse()`. A pack with a shader
routes through `pending_pack_import` and `Application::dialog()` - a
native modal showing the actual WGSL source with Cancel/"Enable anyway" -
before anything is written; a pack without one imports immediately, same
as a plain theme-file drop.

One deviation from the original design notes below: the video is embedded
directly (bytes in the archive) rather than left as a path/URL reference,
since a self-contained one-file share was worth the size tradeoff for a
first version.

- **Format: plain tar, not a custom container.** A manifest TOML (reusing
  the existing `ThemeLayout` schema) plus the media file(s) alongside.
  Deliberately *not* obfuscated/compressed-only in a way that hides
  contents — a plain tar means a user can `tar tf`/extract a pack outside
  the engine and read the `.wgsl` before ever running it, which is the
  whole point given the point below.
- **Custom `.wgsl` is arbitrary GPU code from a stranger.** Packs
  containing a shader must be flagged as such (manifest field, e.g.
  `custom_shader = true`) and the import flow must surface an explicit
  "this pack includes a custom shader — review it before enabling"
  prompt rather than compiling and running it silently. No sandboxing
  planned beyond that disclosure; the tar-not-hidden-container choice
  above is what makes "review before enabling" actually actionable.
- **Video weight**: consider letting the video be a reference
  (path/URL) rather than mandatory embedded bytes, so packs that only
  change layout/visualiser/shader stay small.
- **Extensible by construction.** `pack.toml` follows `ThemeLayout`'s own
  convention — every field/table optional, `#[serde(default)]` throughout —
  so a pack sharing just one thing stays a two-line manifest. `schema_version`
  is a floor, not a lock: the importer accepts anything at or below the
  version it understands and only warns on packs newer than that, and
  unknown fields/tables are ignored rather than rejected (no
  `deny_unknown_fields`), so a future field doesn't break older builds and
  an older pack never stops working. Archive extraction is driven by an
  explicit match over manifest-declared entries (one arm per asset kind —
  `background`, `shader`, ...), not a blind directory walk, so adding a new
  bundlable asset later (fonts, a colour palette, per-monitor variants...)
  is one new optional table plus one new match arm, not a format redesign.

**Correction (2026-07-21, later the same day):** the "every shader is
`include_str!`'d at compile time, no runtime loading exists" claim above
was wrong. `VisualiserPass` (`src/modules/visualiser_pass.rs`) has loaded
the visualiser shader from `ThemeLayout.visualiser.shader` at runtime since
March 2026 (`8ec4a2c` onward) - reads `shaders/<name>.wgsl` (path-traversal
hardened, `e3fefff`/`fec5e11`), falls back to the compiled-in default on a
missing file or a wgpu validation failure (never crashes on bad WGSL,
logs and keeps the previous/default pipeline instead), and `reload()`
already hot-swaps it on a live theme edit. Documented all along in
`docs/THEMES.md`'s `shader` field and the theme template's commented-out
`# shader = "my_custom_shader.wgsl"` line - missed during the original
investigation, which only checked `pipelines.rs` (album art/ambient/weather,
still compile-time-only) and never looked at `visualiser_pass.rs`.
Re-verified today with the offscreen harness's new `--style <name>` flag:
pointed a scratch theme at a deliberately-obvious custom shader (solid
magenta fill) and got exactly that back in the render; pointed another at
deliberately-invalid WGSL and got the logged validation error plus a clean
fallback to the default shader, no crash.

Net effect: custom-shader support for theme packs was **never actually
blocked on the renderer decomposition** - that work was valuable in its own
right (see above) but ran on a separate code path from
`visualiser_pass.rs`, which the decomposition didn't touch. The only
genuinely new work theme packs needed on the shader front was the
import-flow UX (the review-before-enabling prompt) - the loading mechanism
it hangs off of already existed and was already hardened.

## Known upstream issue — libcosmic's ColorPickerModel (pinned rev 6359a94)

Found 2026-07-20 while using the custom text-colour picker (Wallpaper →
Text → Custom). Both live in `~/.cargo/git/checkouts/.../src/widget/
color_picker/mod.rs`, not in this repo, so they can't be fixed here without
vendoring/patching libcosmic itself - tracked for whenever the pin next
gets bumped deliberately (see the `Cargo.toml` comment on why it's pinned
rather than tracking master).

- **Visual: status text clipped while the picker is expanded.** The
  status line at the bottom of the page (e.g. "Ready.") renders with its
  leading characters cut off whenever the color picker is active,
  as if something invisible is drawn over the top-left of that row. The
  widget's `layout()` fully delegates to an inner `Column`, and `draw()`
  separately paints the saturation/value canvas into one of that column's
  child slots - the two appear to disagree on the real painted height by
  roughly one text row.
- **Perf: dragging the hue strip visibly lags.** The saturation/value
  gradient square is rasterised with a per-pixel nested loop
  (`for column in 0..width { for row in 0..height { frame.fill_rectangle
  (1x1 px, color) } }` - tens of thousands of individual draw calls for a
  ~300px canvas). It's cached and only re-runs when the active hue
  changes, which is exactly what continuous dragging on the hue strip
  does, so that specific interaction visibly stutters.

Neither is masked by anything in our own view.rs (confirmed: the picker is
just one more section in a plain stacked `Column`, no overlay/absolute
positioning on our side).

## 1.3 candidate — i18n groundwork

Adopt the COSMIC-native translation stack before the string count grows
further (~80-100 user-facing strings as of 1.2):

- Fluent catalogs under `i18n/<lang>/io.github.kenyon_j.cosmic_wpengine.ftl`,
  embedded via `i18n-embed`/`rust-embed`; language follows the desktop's
  locale automatically (`DesktopLanguageRequester`), per-string English
  fallback
- Mechanical `fl!` sweep over the GUI's literals (view.rs + status
  messages) and the engine's tray menu labels
- Move hand-rolled plural strings ("Imported {n} video{s}") into Fluent
  selectors
- Contribution flow: `.ftl` files as PRs, then Weblate registration once
  there's translator interest
- Out of scope: docs/THEMES.md and release notes; RTL layout mirroring is
  an upstream iced limitation

## Visualiser bar polish

One coherent visual pass over the bars (deferred 2026-07-18 - they still
look good, so no urgency):

- Capsule SDF with smoothstep edges: rounded caps plus real anti-aliasing
  (`eval_shape` in visualiser.wgsl currently hard-cuts at the bar edge)
- Mirror reflection below the baseline ("glass floor", fits the frosted
  identity)
- Glow scaled by the bar's own band energy, not just `lyric_pulse`
- Peak-hold caps that fall with gravity (needs a per-band peak array
  alongside the existing smoothed bands)
- Expose bar width ratio (hardcoded 0.85), cap radius, reflection, and an
  LED/segmented mode as `ThemeLayout` options so themes opt in

## Unscheduled ideas

- Interactive mouse-reactive wallpaper effects
- Plugin API for custom data sources
- Weather widget position/font/style customization (today `ThemeLayout.weather`
  is a plain `TextLayout` — no independent font override) plus new widgets
  beyond the current five, starting with a Time widget (2026-07-21, likely
  sooner than the item below)
- Rudimentary scene builder: visual z-ordering so elements (visualiser bars,
  other objects) can be layered/hidden behind one another (2026-07-21,
  explicitly far off — not to be designed for yet, just not foreclosed)
