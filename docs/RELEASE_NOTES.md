# cosmic-wallpaper 1.2.2

Fixes the release binaries themselves: v1.2.0 and v1.2.1's downloads were
built on Ubuntu 24.04 and linked its FFmpeg 6 dynamically, so any distro on
a different FFmpeg major (Arch, and other rolling releases, bump FFmpeg
sonames on every major) made both binaries die at the dynamic linker before
even reaching `main` — a silent exit 127 with no error visible anywhere.
Also ships the app launcher/icon fixes that were sitting on top of that,
plus a v1.2.1 bug fix (frosted-glass staleness) that had gone out on
binaries this affected.

## Fixed

- **The release binaries no longer depend on the host's FFmpeg version.**
  `cosmic-wallpaper` and `cosmic-wallpaper-gui` now statically link FFmpeg
  at build time instead of linking the CI runner's system libraries, so
  they run on any distro regardless of its installed FFmpeg. If you
  installed v1.2.0 or v1.2.1's standalone binaries on a system whose
  FFmpeg has since moved past major version 6, this is the fix. (The `.deb`
  was never affected — Pop!_OS/Ubuntu package dependencies always pinned
  the right FFmpeg.)
- **A stuck engine now says why.** When the wallpaper engine fails to start
  — including the exact silent failure above — the Settings app's General
  page explains it instead of just saying "Not running": it checks the
  login-autostart failure state and, for a manual Start, captures the
  engine's own error output.

## Added

- **The app now appears in your launcher.** Manual installs (the release
  tarball, and anything the in-app updater keeps current afterwards) never
  registered a desktop entry or icon, so COSMIC's app library had nothing
  to show — the system tray and a terminal were the only ways in. Settings
  now installs its launcher entry and icon set on first run. Packaged
  installs (`.deb`, Flatpak) are unaffected; they already handled this
  themselves.

---

# cosmic-wallpaper 1.2.1

Patch release for a theme-editor blind spot found minutes after 1.2.0, plus
a frosted-glass staleness bug caught in review.

## Added

- **An app icon.** The Settings launcher now has its own neon-gear icon
  instead of borrowing the system wallpaper icon. The release tarball ships
  it under `icons/` — copy that folder's contents into
  `~/.local/share/icons` for manual installs.

## Fixed

- **Frosted glass no longer shows the previous artwork or wallpaper.**
  The cached blur behind the frosted-glass effect was only rebuilt when the
  source's *dimensions* changed. Album art is almost always the same size
  track to track (streaming services serve fixed-size covers), and desktop
  backgrounds usually share your monitor's resolution — so the frost kept
  blurring the previous track's art, or the wallpaper you just switched
  away from. The blur now rebuilds whenever the underlying image is
  replaced. Also hardened the blur chain against extreme aspect-ratio
  sources and themes with a hand-edited `size = 0.0`.
- **Album art position and size now work with circular visualisers.**
  Circular visualisers have always captured the album art into their ring
  while music plays — art follows the ring's position and size. That's a
  nice default, but it silently ignored the Album Art sliders in the new
  theme editor. The behaviour is now a theme setting: **dock_art**
  (on by default, so existing themes look identical), with a toggle on the
  editor's Visualiser tab. While docked, the Album Art tab says so and
  points at the toggle instead of offering sliders that do nothing.

See the [1.2.0 notes](https://github.com/Kenyon-J/cosmic-wpengine/releases/tag/v1.2.0)
for the Themes Release itself: the live theme editor, text sizing, theme
import and gallery, and engine controls in Settings.
