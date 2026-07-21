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

## Renderer decomposition (in progress — see PLAN-renderer-decomposition.md)

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

Phase 4 (render-target abstraction + offscreen harness) started 2026-07-21:
the render-target half is done — `draw.rs`'s encode block is now
`encode_frame()`, taking a bare `&wgpu::TextureView` and every GPU resource
it draws with individually (device/queue/pipelines/bind groups), not
`&Renderer` (same borrow-checker reason as `prepare_text_buffer`: the
per-output loop already holds `renderer.outputs` mutably borrowed via its
iterator). Presenting stays the caller's job, so the same function can
drive both the live per-monitor loop and a future offscreen path. Still
missing: the actual `--render-frame <out.png>` dev harness (building a
`Renderer` without Wayland, a synthetic scene, and the perceptual-diff
`--compare` mode) - that and phase 5 (split the per-output loop) are the
remaining work, the prerequisite for runtime shader loading (see "theme
packs" below), reprioritized 2026-07-21 to land before that feature rather
than after.

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

## 1.3 candidate — theme packs (bundle sharing)

Extend the 1.2 Themes Release's "Import Theme" / gallery model from a bare
layout TOML into a full pack: background image/video, visualiser settings,
and (optionally) a custom `.wgsl` shader, bundled so a pack works like a
Wallpaper Engine workshop item — one file to install a whole look. Idea
from Joshua 2026-07-21; agreed direction, not yet scheduled/planned.

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

**Sequencing decision (2026-07-21):** ship custom-shader support in the
first version of packs rather than staging a shader-less v1 first. Every
shader today is `include_str!`'d at compile time — there is no runtime
shader-loading path at all — so this depends on the renderer decomposition
above landing first (phases 3–5), followed by a runtime shader-loading
capability built on the decomposed structure, before pack format work
starts. Order: renderer decomposition (phases 3–5) → runtime shader loading
→ theme packs (full format, custom shader included from the start).

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
