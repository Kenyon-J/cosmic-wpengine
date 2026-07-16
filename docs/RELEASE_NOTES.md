# cosmic-wallpaper 1.0.0

First stable release. 🎉

## Highlights

- **COSMIC 1.3 frosted glass**: the settings app now frosts with the
  system-wide frosted-glass style, and the engine's own *Frosted Glass*
  background mode renders solid-colour and gradient desktop wallpapers —
  previously only image wallpapers showed through.
- **Clean exits**: fixed a segfault that fired on every graceful shutdown
  (tray *Quit Engine*, and every logout). The Wayland connection was torn
  down before the GPU surfaces that referenced it.
- **MPRIS reliability**: Firefox album art is no longer rejected by the
  art-path allowlist; a paused player keeps the watch instead of losing it
  to a background tab; and a short pause no longer wipes the scene (the
  inactivity reset went from 15 seconds to 2 minutes).

## Hardening (the V1 security plan)

- Releases are now signed: `SHA256SUMS.txt` carries a minisign signature,
  and the in-app updater verifies it against a key embedded in the binary
  before trusting any hash. Updater downloads are pinned to the approved
  release tag.
- SSRF validation on the Spotify Canvas video path; the canvas proxy is
  opt-in via `audio.canvas_proxy_url` (no more hardcoded localhost default).
- All remote downloads are size-capped; GPU uniform uploads use checked
  `bytemuck` casts (no `unsafe` left in the draw path); `cargo audit` runs
  as a hard CI gate.

## Known limitations

- A stale MPRIS watcher thread can linger until session-bus name activity
  wakes it: the upstream `mpris` crate's blocking event iterator cannot be
  interrupted. Watchers are reaped on player switches, capping the impact.
- Prebuilt binaries and the `.deb` target x86_64 Linux only.

## Install

Pop!_OS / Ubuntu 24.04: `sudo apt install ./cosmic-wallpaper_*.deb` — the
package includes both binaries and a session autostart entry. Other distros:
use the prebuilt binaries and verify them against the signed `SHA256SUMS.txt`.
