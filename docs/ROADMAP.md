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

The ~100-field `Renderer` + ~850-line `draw_frame` split into five phases
(`FrameParams`, `AudioAnalysis`, `ArtLayer`/`BackgroundLayer`, a headless
render-target + offscreen `--render-frame` harness, and a `TextSubsystem`
carved out of the per-output loop) — all landed 2026-07-19/21, each phase
pixel-verified via the harness against its own before/after baseline. Full
phase-by-phase history is in the git log and
[PLAN-renderer-decomposition.md](PLAN-renderer-decomposition.md); the one
thing not fully hit was the plan's line-count target (`draw.rs` landed at
655 lines against a ~300 target - `write_frame_uniforms` in particular is
still ~170 lines and could be split further per buffer kind, fair game for
a future pass on its own).

The harness (`modules::renderer::render_frame_to_png`, a hidden
`--render-frame <out.png> [--compare <baseline.png>] [--style <name>]`
engine flag) turned out to be broadly useful beyond this refactor - it's
now the standard way to verify any renderer change without a live desktop
session, and backed the theme-packs shader-loading investigation below.

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

## Theme packs (bundle sharing) — SHIPPED v1.4.0

Extends the 1.2 Themes Release's "Import Theme" / gallery model from a bare
layout TOML into a full pack: background video, visualiser layout, and
(optionally) a custom `.wgsl` shader, bundled so a pack works like a
Wallpaper Engine workshop item — one file to install a whole look. Idea
from Joshua 2026-07-21; built and hardened the same day.

Lives on its own **Packs** page (kept separate from Layout Themes so that
page doesn't get cluttered), `src/modules/config/pack.rs` holds the
format (`PackManifest`/`PackContents`/`ParsedPack`, `build()`/`parse()`):

- **Plain gzipped tar, extension `.cwtheme`.** Deliberately not a custom
  container — `tar tzf` (or an archive manager) opens one like any other,
  so a shader can be inspected before this app ever reads it.
- **A custom shader gates on an explicit review.** A pack that bundles one
  stashes everything in `pending_pack_import` and blocks on a native
  `Application::dialog()` modal showing the actual WGSL source
  (Cancel/"Enable anyway") before anything is written; a pack without one
  imports immediately, same as a plain theme-file drop.
- **Extensible by construction**, mirroring `ThemeLayout`'s own convention:
  every table optional (`#[serde(default)]`, no `deny_unknown_fields`), so
  a pack sharing one thing stays a two-line manifest and a future asset
  kind is one new table plus one new match arm in extraction, not a format
  redesign. `schema_version` gates the pack *format*; `app_version`
  (the exporting build's version) rides along purely so a `theme.toml`
  parse failure — e.g. a newer `VisShape` variant an older build has never
  heard of — can name which version made the pack instead of surfacing a
  bare TOML error.
- **A "Your Packs" gallery** on the same page lists every import with a
  one-click Apply (sets the layout and, if the pack bundled one, the
  background video together) — added after the first cut left an
  imported video's file copied to disk but otherwise inert.
- **Collision-safe writes.** A theme, shader, or video sharing a file name
  with an unrelated pack's asset lands under a numbered suffix instead of
  silently overwriting it (or being silently shadowed by it); re-importing
  a pack you already have (byte-identical content) is a no-op rather than
  piling up duplicates.
- **Video is embedded directly** (bytes in the archive), not left as a
  path/URL reference — a self-contained one-file share was worth the size
  tradeoff over a smaller-but-dependent pack.

Runtime shader loading (`src/modules/visualiser_pass.rs`, reads
`ThemeLayout.visualiser.shader` at runtime, path-traversal hardened, falls
back cleanly on a bad/missing shader) turned out to already exist since
March 2026 — the renderer decomposition above was valuable in its own
right but was never actually a prerequisite for this feature, since it
ran on a separate code path the decomposition didn't touch.

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

## 1.4.1 candidate — WGSL shader pass

The shaders (`visualiser.wgsl` especially) are the least-revisited part of
the codebase and it's starting to show. Triggered by a bug hunt
2026-07-21 that found "Square" (a real `VisShape` variant, selectable in
the theme editor) silently rendering as an oversized circular ring - the
shader only ever branches on `shape == 1u` (Linear) vs. an "else"
catch-all it built for Circular, so Square was never actually
implemented. Hidden from the picker in v1.4.0 rather than shipped broken
(see the Theme packs section above); this is where it gets a real fix.

- **Implement `VisShape::Square` properly** - bars around a square
  perimeter, following the same instanced-bar approach `eval_shape`/
  `eval_shadow` already use for Linear/Circular, verified pixel-by-pixel
  with the offscreen harness's `--style` flag before re-enabling the
  picker option.
- **Fix the known formatting issues** Joshua has already flagged in the
  `.wgsl` source (not yet catalogued in this doc - pull the specifics from
  him when this starts).
- Fold in the deferred visual-polish pass while the file is open anyway
  (no urgency on these individually, but cheap to bundle):
  - Capsule SDF with smoothstep edges: rounded caps plus real
    anti-aliasing (`eval_shape` currently hard-cuts at the bar edge)
  - Mirror reflection below the baseline ("glass floor", fits the frosted
    identity)
  - Glow scaled by the bar's own band energy, not just `lyric_pulse`
  - Peak-hold caps that fall with gravity (needs a per-band peak array
    alongside the existing smoothed bands)
  - Expose bar width ratio (hardcoded 0.85), cap radius, reflection, and
    an LED/segmented mode as `ThemeLayout` options so themes opt in

## Unscheduled ideas

- Interactive mouse-reactive wallpaper effects
- Plugin API for custom data sources
- **Editable widgets** (2026-07-21, next up after the WGSL pass): Weather
  widget position/font/style customization (today `ThemeLayout.weather` is
  a plain `TextLayout` — no independent font override); new widgets beyond
  the current five, starting with a Time widget; further out, custom
  widgets backed by arbitrary data sources (webhooks?) — floated but not
  designed, main open question is how a widget ships/bundles at all (a
  `.toml` "scene" file akin to today's theme packs, selectable like a
  theme, is the leading idea)
- Rudimentary scene builder: visual z-ordering so elements (visualiser bars,
  other objects) can be layered/hidden behind one another (2026-07-21,
  explicitly far off — not to be designed for yet, just not foreclosed)
