# cosmic-wallpaper 1.2.0

The Themes Release. Your desktop is now the theme editor's preview.

## Build themes by dragging sliders

- **Live theme editor** — Settings → Layout Themes. Pick an element (album
  art, track info, lyrics, visualiser, weather, effects) and adjust its
  position, size, shape, alignment and motion with native controls. Every
  change saves the theme's TOML instantly and the engine reloads it live,
  so you watch your actual desktop rearrange as you drag. Each control is
  labelled with its TOML key — themes built by slider and themes written by
  hand are the same files.
- **Text sizing** — lyrics, track info and weather each gain a `size`
  scale, adjustable in the editor. Long-requested, long hardcoded.
- **Fine tuning** — every slider is paired with a stepper for exact
  single-increment nudges (also on blur amount, visualiser bands,
  smoothing and the frame-rate limit).

## Themes are for sharing

- **Import** — drop a theme's `.toml` onto the Layout Themes page; it's
  validated and added to your library.
- **Gallery** — the repository now has a [themes/](../themes/) directory
  with ready-made layouts (*centre-stage*, *minimal*) — contributions
  welcome, every file is parse-checked in CI.
- **Docs** — [THEMES.md](THEMES.md) documents every key, range and
  default; *Create Theme* now writes a fully-commented starter file
  instead of a bare stub.

## Engine control from Settings

- Settings → General shows whether the engine is running, with Start and
  Stop buttons — no more hunting for the tray after quitting it.
- Fixed: the *Start on login* toggle managed a different autostart file
  than the one the package installs, so it displayed the wrong state — and
  enabling it could have started two engines at login. It now manages the
  canonical entry with an absolute path.

## Install

Pop!_OS / Ubuntu 24.04: `sudo apt install ./cosmic-wallpaper_*.deb`. Other
distros: use the prebuilt binaries and verify them against the signed
`SHA256SUMS.txt`. Upgrading: Settings → General offers the update in-app.
