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
