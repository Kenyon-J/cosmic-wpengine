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

## 1.3 candidate — i18n groundwork (code complete 2026-07-23, unreleased)

Adopted the COSMIC-native translation stack (the same `i18n-embed` +
`i18n-embed-fl` + `rust-embed` combination libcosmic itself uses internally,
mirroring its own `src/localize.rs`) before the string count grew further -
around 190 `fl!` call sites landed in one pass:

- `i18n.toml` + `i18n/en/io.github.kenyon_j.cosmic_wpengine.ftl`, embedded
  via `src/modules/i18n.rs`'s `LANGUAGE_LOADER`; language follows the
  desktop's locale automatically (`DesktopLanguageRequester`), with English
  as the fallback for every key - `fl!` initializes it lazily on first call,
  so no separate startup wiring was needed in either binary
- Mechanical `fl!` sweep over the GUI's literals (`view.rs`'s ~185 strings,
  every `status_msg` assignment in `mod.rs`, `bootstrap.rs`'s launcher-issue
  text) and the engine's tray menu labels (`tray.rs`)
- Hand-rolled plurals ("Imported {n} video{s}") moved into Fluent `[one]`/
  `*[other]` selectors (imported videos/themes/packs, importing files)
- **Found and fixed two latent locale bugs the sweep would otherwise have
  introduced**: `Message::TemperatureUnitSelected` compared its dropdown
  payload against the literal string `"Fahrenheit"` (would silently break
  once that label was translated) - changed to an index like
  `PollIntervalSelected` already was; three `status_msg.starts_with("Ready"/
  "Engine start"/"Engine stop")` sentinel checks compared against
  now-localized text - changed to compare against the same `fl!(...)` call
  used to set them, so the check stays correct in whatever language is
  active
- Verified: `cargo test --workspace` (131 tests, added two covering
  fallback-to-English and named-arg interpolation), `cargo clippy`,
  `cargo fmt`, and a live launch on this machine's COSMIC session
  (screenshot of the General page) confirming real rendering - no
  message-id leaking through, `{ $pid }` interpolation correct
- **Six community-drafted catalogs added the same day**: `es`, `fr`, `de`,
  `it`, `nl`, `pt` - AI-translated in one pass at the user's request as a
  "99% done, needs a native speaker's pass" starting point, not a
  substitute for real translator review (see the "AI-translated,
  unreviewed" caveat below). Each has exact key parity with `en` (guarded
  by a test) and its own `[one]`/`*[other]` plural forms per its own CLDR
  rule, not copies of the English split. Verified: every catalog parses as
  valid Fluent (`fluent-bundle` dev-dep, directly - `fluent_bundle` logs
  parse errors via the `log` crate, which this project never bridges to
  `tracing`, so a broken message would otherwise fail silently instead of
  failing a test), every catalog actually gets selected over the English
  fallback when requested, every plural message resolves both categories
  without panicking in every locale, and a live launch with
  `LANGUAGE=es:en` (screenshot) confirmed real rendering - accented
  characters, no tofu, no leaked message IDs.
  - **AI-translated, unreviewed by a native speaker.** Confidently within
    the assistant's strongest language tier, but still worth flagging
    explicitly here rather than silently treating as production-quality:
    should get an actual native-speaker pass (via the PR flow below)
    before being pointed at from anywhere user-facing (a language picker,
    a release note claiming "N languages supported").
  - Found one real bug while writing these, not just translating: the
    `TemperatureUnitSelected`/`starts_with("Ready"/"Engine start"/"Engine
    stop")` fixes above were driven by actually trying to translate the
    strings those checks compared against, not by reading the Rust
    separately - localizing surfaces this class of bug for free.
- Contribution flow open next: the six drafts above as a starting point
  for `.ftl` files as PRs (real native-speaker review still needed on all
  six), then Weblate registration once there's translator interest -
  further languages beyond these six also welcome
- **Manual language picker added on General**, at the user's request:
  COSMIC's own locale list won't include every language a community
  catalog might target (e.g. Kernewek/Cornish has no COSMIC-level locale
  support), so a "follow the desktop" default alone would leave such a
  catalog unreachable however good the translation. `Config.language:
  Option<String>` (`None` = follow the desktop, same as before) persists
  the choice; the picker itself is built from
  `modules::i18n::AVAILABLE_LANGUAGES`, which asks every embedded catalog
  for its own `language-name` message rather than a hardcoded list - a
  future `i18n/kw/*.ftl` needs no code change to appear. Applies live in
  the GUI; the engine (tray labels) picks up a saved override on its next
  start, not live, since it isn't already watching config for this the way
  it does for a few other hot-reloaded settings.
- Out of scope (unchanged): docs/THEMES.md and release notes; RTL layout
  mirroring is an upstream iced limitation

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
- **Solid fill-colour override as a GUI control.** `color_top`/
  `color_bottom` already exist as `ThemeLayout` fields (TOML-only today -
  see THEMES.md) and can be hardcoded by hand for a pack that wants one,
  so there's no functional gap, just a missing theme-editor control.
  Worth adding for pack creators who want a one-click fixed colour instead
  of the adaptive album palette, but it needs a colour-picker widget in
  the Visualiser tab, and the project's existing text-colour picker has
  already surfaced a couple of upstream `libcosmic` `ColorPickerModel`
  bugs (see "Known upstream issue" above) worth weighing before adding a
  second picker instance. (Deferred out of the v1.5.0 visualiser bar
  polish pass.)
- **Further split `write_frame_uniforms`** (~170 lines, one function per
  buffer kind) - the one target the renderer decomposition didn't fully
  hit; `draw.rs` landed at 655 lines against a ~300 line goal. Fair game
  for a future pass on its own, not urgent.
