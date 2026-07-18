# cosmic-wallpaper 1.1.0

The frosted glass and settings release.

## Frosted glass, rebuilt

- The *Frosted Glass* background now uses the same dual-Kawase blur as
  COSMIC's own frosted windows, with the Blur Amount slider mapped onto the
  compositor's real strength curve. The old single-pass blur's grain at high
  strengths is gone — and because the blur is rendered once and cached
  instead of recomputed every frame, the steady-state GPU cost dropped too.
- Text over the wallpaper now picks its colour from what is actually behind
  it (the wallpaper, dimmed by the glass) instead of the album palette, so
  lyrics stay readable on bright wallpapers.
- Prefer a fixed colour? There's now a **text colour picker**: Wallpaper →
  Text → Custom.

## A redesigned Settings app

- One long page became seven sidebar pages in the COSMIC System Settings
  style: Wallpaper, Live Wallpapers, Layout Themes, Now Playing, Visualiser,
  Weather, General. Options appear only when the style they belong to is
  selected.
- Background styles are now visual cards previewing your actual wallpaper,
  and Frosted Glass has a live preview that responds to the blur slider.
- **Live Wallpapers library**: drag video files from your file manager
  straight into the window to import them; the library shows first-frame
  thumbnails and durations, and clicking a tile sets it as your background.
- New in the GUI: visualiser bands and smoothing, weather units, location
  and update interval, a *Prefer Spotify Canvas* toggle, and a shortcut to
  the videos folder.
- The inline TOML editor is retired — *Open Folder* plus the engine's live
  reload covers hand-editing, without a second editor to maintain.

## Fixes

- Switching from a video background back to any other style no longer
  leaves the last video frame stuck on screen.
- Dropdown menus in Settings are opaque again on frosted/transparent system
  themes.
- Turning *Prefer Spotify Canvas* off stops Canvas loops immediately,
  mid-track.

## Install

Pop!_OS / Ubuntu 24.04: `sudo apt install ./cosmic-wallpaper_*.deb` — the
package includes both binaries and a session autostart entry. Other distros:
use the prebuilt binaries and verify them against the signed `SHA256SUMS.txt`.
Upgrading from 1.0.0: the in-app updater (Settings → General) verifies and
installs it for you.
