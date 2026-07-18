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
