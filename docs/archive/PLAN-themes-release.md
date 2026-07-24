# PLAN: 1.2 "The Themes Release"

**Status: Archived — shipped.** All 5 work items verified present in code
(`TextLayout.size`, `THEME_TEMPLATE`, `themes/` gallery, engine status row,
DnD import) as of 2026-07-24, several releases past the 1.2 this plan
targeted. Kept for history.

Graduated from [ROADMAP.md](ROADMAP.md) on 2026-07-18 (Joshua's go).
Interactive mockup exists (artifact; ask Joshua). Keep this file updated —
it is the handoff document.

## Verified plumbing (2026-07-18)

- Engine watches `shaders/` (`Config::watch`, config/mod.rs:179-193): any
  theme file write triggers ConfigUpdated → renderer reloads the layout.
  The live editor loop needs zero engine changes.
- `ThemeLayout::load(style)` parses `shaders/{style}.toml`, falling back to
  defaults per-field (serde defaults) — partial themes are valid.
- `toml = "0.8"` serializes (ThemeLayout derives Serialize).
- GUI already has: debounce machinery, dropdowns, settings rows, DnD
  plumbing (Live Wallpapers), colour picker.

## Work items

### 1. Engine: `TextLayout.size` (scale multiplier)

- [ ] `size: f32` on TextLayout, `#[serde(default = "default_text_size")]`
      (1.0); update the `default_*_layout()` constructors
- [ ] draw.rs: multiply into lyric `base_font_size` (~line 603), track
      `info_scale` (~757), weather `weather_scale` (~789)

### 2. GUI: live theme editor (Layout Themes page)

- [ ] State: `edit_theme: Option<ThemeLayout>` loaded on theme selection;
      `theme_element` selector state; `theme_save_generation` debounce
- [ ] `ThemeEdit(ThemeEditMsg)` message enum: PosX/PosY/Size/Rotation/
      Amplitude/Bounce/Stiffness/Damping/BeatPulse(f32),
      Shape/Align(usize)
- [ ] Serialise + debounce-write to `shaders/{name}.toml`; status line
      confirms save; engine reload is automatic (watcher above)
- [ ] Element tabs (Album Art / Track Info / Lyrics / Visualiser /
      Weather / Effects) — native widgets only, same vocabulary as the
      rest of the app
- [ ] Editing the active theme = live desktop feedback; editing inactive
      themes just writes the file

### 3. Starter template + docs

- [ ] `Create Theme` writes a complete commented default layout
      (hand-maintained template const — comments can't come from serde)
- [ ] `docs/THEMES.md`: every field, range, default, annotated examples

### 4. Import + gallery

- [ ] DnD a `.toml` onto the Layout Themes page: parse-validate, copy into
      `shaders/`, refresh list (reuses DroppedFiles)
- [ ] `themes/` gallery directory in the repo with example layouts

### 5. Engine status row (General page)

- [ ] `find_engine_pid()` (scan /proc/*/comm for cosmic-wallpaper)
- [ ] Row shows Running/Stopped; Start (spawn detached) / Stop (tray quit
      via busctl, the tested recipe); status re-checked after actions

## Status log

- 2026-07-18: plan written, plumbing verified. Implementation starting
  with item 1 (engine size field), then 2.
- 2026-07-18 (later): ALL five items code-complete in one pass:
  - `TextLayout.size` (clamped 0.25-4.0 in draw.rs at the three font-size
    sites; theme reloads already clear the text cache, so it applies live)
  - Theme editor on Layout Themes: element tab row (card_class buttons),
    per-element `settings::section` rows via `theme_slider` helper, every
    row's description shows its TOML key; `ThemeEdit(ThemeEditMsg)` →
    `apply_theme_edit` → debounced `write_theme_file`
    (`toml::to_string_pretty`); status line says whether the theme is live
  - Apply button becomes an "Active" marker when the edited theme is live
  - Create Theme writes `THEME_TEMPLATE` (fully commented defaults);
    tests assert the template parses AND matches `ThemeLayout` defaults
  - Import: whole Themes page is a dnd_destination; dropped .toml files
    are parse-validated before copying into shaders/
  - Engine row on General: `find_engine_pid()` via /proc cmdline (comm
    truncates at 15 chars and cannot distinguish engine from GUI!),
    Start spawns from PATH, Stop uses the tray-quit busctl recipe
  - `docs/THEMES.md` + `themes/` gallery (centre-stage, minimal) with a
    CI test that parses every gallery file
  - Build + clippy clean, 16 gui tests pass; binaries installed (engine
    left stopped - Joshua had quit it; GUI running)
  - Awaiting Joshua's interactive pass: slider → desktop live loop with
    the engine running, theme drop-import, engine Start/Stop buttons.
