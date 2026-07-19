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

## Renderer decomposition

The ~120-field `Renderer` + ~900-line `draw_frame` split (Phase 9 of
[PLAN-v1-hardening.md](PLAN-v1-hardening.md)). Needs its own plan and ideally
a frame-capture harness first.

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

## Portable release binaries — stop dynamically linking ffmpeg

Found 2026-07-19: the CI release binaries (built on Ubuntu 24.04) link
ffmpeg 6's sonames dynamically (`libavutil.so.58`, ...). Any distro that
ships a different ffmpeg major — notably Arch and other rolling releases,
which bump sonames on every ffmpeg major — makes both binaries die at the
dynamic linker (exit 127) before `main`, so the engine's autostart fails
silently and the GUI "fails to launch" with no visible error. The
self-updater then faithfully keeps users on broken binaries.

- Static-link or vendor ffmpeg in the release build (ffmpeg-next's
  build-from-source feature, or prebuilt static libs in CI) so assets no
  longer depend on the host's ffmpeg version
- Until then the workaround is building from source; consider having the
  GUI surface a linker-level failure of the engine binary (exit 127 from
  the autostart unit) instead of failing silently

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
