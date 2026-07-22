# cosmic-wallpaper 1.5.1

Five bugs found in a full-codebase sweep, fixed.

## Fixed

- **The theme editor could silently lose or misfile an edit.** Switching
  themes (dropdown, pack import, theme creation, or applying a saved pack)
  within about 300ms of the last slider/toggle change used to write the
  *newly-selected* theme's own unedited layout over its own file, while
  quietly discarding whatever was actually pending for the theme you'd
  just left - no error, nothing in the status line. Switching themes now
  always flushes the outgoing one first.
- **Dropping an oversized or crafted `.cwtheme` pack could exhaust memory.**
  A dropped pack was read fully into memory and parsed before the
  shader-review gate - or any validation - ever ran, with no cap on total
  entry count or cumulative size (only any *one* entry was bounded). Now
  checked by file size before it's ever read, and by cumulative
  entry size/count while parsing.
- **A looping background video could desync after a failed seek-to-start.**
  Reopening the file kept the previous decoder/scaler/stream index, which
  aren't guaranteed valid against a fresh handle - now everything needed
  to decode is rebuilt together.
- Fixed a blocking-filesystem call on the MPRIS async task that could
  stall track-change processing for every media player on a slow disk.
- Closed a theme-name edge case (`CreateTheme` checked the untrimmed name
  for emptiness) that a whitespace-only or exactly-`.toml` name could slip
  past, though the settings UI already prevented reaching it in practice.

---

# cosmic-wallpaper 1.5.0

Visualiser bar polish: capsule-shaped bars, a "glass floor" reflection,
LED-segmented mode, and gravity peak-hold caps — all themeable, and all
exposed as sliders/toggles on the theme editor's Visualiser tab, not just
TOML keys. Plus a Monstercat theme that actually looks like Monstercat's,
and a full custom-shader-writing guide.

## Added

- **Capsule bars with real anti-aliasing.** Bars now render as a rounded-box
  SDF with proper antialiased edges instead of a hard per-pixel cutoff;
  `cap_radius` controls the rounding, from a crisp rectangle to a full
  pill/capsule.
- **Mirrored "glass floor" reflection** below the baseline, fading with
  depth (`reflection`).
- **LED/segmented mode** (`led_segments`) chops each bar into discrete
  VU-meter-style chunks.
- **Peak-hold caps** that hold each bar's recent maximum and fall back
  under gravity (`peak_hold`) - off by default (see Changed below), but
  available for anyone who wants the classic look.
- **Glow now scales with each bar's own volume**, not just the beat pulse
  (never brighter than before, only dimmer for quiet bars - no visual
  regression on existing themes).
- **Bar width is themeable** (`bar_width_ratio`, was a hardcoded constant).
- All six of the above are now sliders/toggles on the Layout Themes editor's
  Visualiser tab.
- **`docs/CUSTOM_SHADERS.md`**: a full guide to writing a custom visualiser
  shader - the complete uniform/storage-buffer layout, what audio-reactive
  values are available and how they're derived, a minimal working example,
  and how to iterate on one with the engine's headless `--render-frame`
  flag instead of a live desktop session.

## Changed

- **Monstercat's shipped theme now actually looks like Monstercat's real
  visualiser**: flat rectangular bars, no glow, tightly packed - measured
  directly off a reference image rather than eyeballed. Colour is left
  adaptive to the album/wallpaper palette, same as before (real
  Monstercat-style visualisers vary their fill colour by genre, so this
  theme's fix is about bar *shape*, not a fixed hue).
- Peak-hold caps ship **off** by default: tried live against the built-in
  themes and the marker read as a visually disconnected floating mark
  rather than a clean cap. Kept as an opt-in toggle rather than dropped,
  since the underlying mechanism works fine - it's a look-and-feel call,
  not a bug.

---

# cosmic-wallpaper 1.4.1

Fixes the "Square" visualiser shape hidden in v1.4.0.

## Fixed

- **"Square" now actually draws a square.** It's back in the Layout
  Themes visualiser Shape picker, now drawing a real square perimeter of
  bars instead of the mislabeled circular ring it used to silently fall
  back to.

---

# cosmic-wallpaper 1.4.0

Theme packs: share a full look — layout, background video, and a custom
visualiser shader — as one file. Plus a couple of longstanding bugs found
and fixed along the way.

## Added

- **Theme packs.** A new Packs page lets you bundle a theme's layout with
  your background video and custom visualiser shader into one `.cwtheme`
  file (`Export Pack`), and import one someone else made by dropping it
  onto the page. It's a plain gzipped tar — `tar tzf` opens it like any
  other archive, so you can inspect a shader before ever pointing this app
  at it. A pack with a custom shader stops first and shows you the actual
  source, since that's arbitrary GPU code from whoever made it — nothing
  is written to disk until you review it and click "Enable anyway".
  Every import also lands in a "Your Packs" gallery on the same page with
  a one-click Apply that sets the layout and background video together.

## Fixed

- **Album art could occasionally lag one track behind.** The check
  deciding whether a slow-arriving art fetch still belonged to what's on
  screen compared only title/artist/album — two distinct plays (a
  remaster, a repeat-mode replay some players re-announce under a fresh
  identity) can share that verbatim, so the fetch for the track actually
  showing could get matched against the wrong one and dropped, leaving
  the previous track's art stuck on screen. Now compared by a stable
  per-track identity instead.
- **"Square" in the Layout Themes visualiser editor didn't do anything.**
  It rendered as an oversized circular ring — the shader never actually
  implemented it. Removed from the picker until it gets a real
  implementation, rather than leaving a broken option selectable.

---

# cosmic-wallpaper 1.3.2

Fixes the release binaries themselves, again: v1.3.1's standalone downloads
crash on launch on any CPU without AVX-512.

## Fixed

- **The release binaries no longer crash with "Illegal instruction" on CPUs
  without AVX-512.** The statically-linked FFmpeg build baked in whatever
  SIMD instructions the GitHub Actions build runner happened to support, with
  no fallback for the machine actually running it — so a build made on an
  AVX-512-capable runner reliably crashed on launch (`cosmic-wallpaper-gui`)
  or on first video decode (`cosmic-wallpaper`) for anyone without it. If you
  updated to v1.3.1's standalone binaries and Settings stopped launching,
  this is the fix. (The `.deb` was never affected.)

---

# cosmic-wallpaper 1.3.1

Patch release for a regression found testing 1.3.0.

## Fixed

- **Live Wallpaper videos could get stuck on a frame after switching back
  to Frosted Glass.** A video frame already in flight when you switched
  away could still land just after the background reloaded, clobbering it
  right back with the stale frame. Fixed by dropping any video frame that
  arrives after video's been turned off, instead of trusting it was still
  wanted.

---

# cosmic-wallpaper 1.3.0

Diagnostics and quality-of-life additions, plus a handful of bugs found
and fixed along the way.

## Added

- **Copy Diagnostics, and pre-filled bug reports.** The General page gets
  a "Copy Diagnostics" button (version, distro, engine status, GPU
  adapter, recent log tail) for pasting into a report, and "Report an
  Issue" now opens a GitHub issue pre-filled with your version and recent
  errors instead of just the bare issues page.
- **Patch notes render as actual Markdown.** Headings, bold text and
  links on the General page now render properly instead of showing raw
  `##`/`**` syntax.
- **"Use my location" on the Weather page.** Estimates your latitude and
  longitude from your IP address instead of requiring manual entry.
- **"Reset to defaults" in the theme editor.** Each element — album art,
  lyrics, visualiser, and so on — can now be reset to its default layout
  individually.
- **A persistent log file.** Both the engine and Settings now write
  rotating daily logs to `~/.config/cosmic-wallpaper/logs/` in addition
  to the terminal — this is what powers Copy Diagnostics and pre-filled
  bug reports above.
- **A notice when the app isn't in your launcher.** Surfaces
  automatically on the General page if the desktop entry failed to
  install, rather than leaving you to wonder why it's tray/terminal-only.
- **Simpler install instructions.** The README's install steps are now
  copy-pasteable one-liners (`wget` + `sudo apt install`, or `wget` +
  `chmod` for other distros) instead of a trip to the Releases page.

## Fixed

- **The Settings window now shows the correct icon in your taskbar.** It
  was using a leftover template identifier that didn't match the
  installed desktop entry, so the taskbar fell back to a generic icon
  (the app launcher itself was unaffected, since it reads desktop files
  directly).
- **Two harmless-but-noisy upstream log lines are now filtered out**, so
  they don't drown out real errors in the new diagnostics/bug-report
  tooling above.
- **Two small sources of files that could accumulate over time are now
  cleaned up**: orphaned video thumbnails left behind after deleting a
  video, and a leftover temp file if an update ever fails partway
  through.
- **The statically-linked standalone binaries now ship a proper
  third-party license notice for the bundled FFmpeg**, as required by
  its license — see `THIRD-PARTY-LICENSES.md`.

---

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
