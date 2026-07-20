# Plan: Live wallpaper feature additions

Goal: implement four features identified 2026-07-20 that fit this project's
existing architecture cleanly - config.toml + theme system, the video
library, the per-output render loop - rather than needing new
infrastructure. **Each phase is one self-contained commit** and leaves the
tree green (`cargo fmt --all --check && cargo clippy --all-targets --
-D warnings && cargo test`), plus a live verification appropriate to that
phase (see each phase's Verify line).

Anchored against master @ `0e4db32` (post-markdown-patch-notes). Re-grep
before editing if the tree has moved.

Do phases 1-3 in any order; each is independent. Phase 4 (per-monitor) should
come last: it's the only one big enough to warrant its own sub-plan once
started, and it touches the render loop's output identification, which the
other three don't.

---

## Context primer (regions this plan touches)

| File | What it is |
|---|---|
| `src/modules/config/types.rs:75-95` | `AppearanceConfig` - `custom_background_path`, `video_background_path` already exist as plain `Option<String>` fields. Phases 1-2 add fields alongside these, not new subsystems. |
| `src/modules/config/types.rs:397` (`resolved_background`) | Already reads `custom_background_path` first, falls back to cosmic-bg's own resolved wallpaper. Phase 2's picker just needs to *set* this field via GUI; the engine-side plumbing is done. |
| `src/modules/gui/library.rs` | `videos_dir()`, `Config::available_videos()`, `scan()`. Phase 1's rotation picks from the same list already shown on the Live Wallpapers page. |
| `src/modules/renderer/core/mod.rs:213-218` | The run loop's per-tick FPS re-read (`self.state.config.fps.max(1)`) - the exact pattern phase 3's battery-throttle follows: read a cheap, cached signal once per loop iteration, react if it changed. |
| `src/modules/renderer/draw.rs:234` (`for (i_idx, gpu_out) in renderer.outputs.iter_mut().enumerate()`) | The per-output render loop. Phase 4 is the only phase that touches this - everything else stays global-config-driven. |
| `src/modules/wayland.rs:38-49` (`WaylandWindowInfo`) | Holds `output: wl_output::WlOutput` but **no name/description** today. Phase 4's monitor identification needs to add this via smithay-client-toolkit's `OutputState::info(&output)` (already a dependency, just not called for this yet) - confirmed by grep: no `.info(` call exists in `wayland.rs` currently. |
| `Cargo.lock` (`dbus v0.9.10`) | Already a transitive dependency via the `mpris` crate. Phase 3 reuses it directly - confirmed via `busctl --system get-property org.freedesktop.UPower /org/freedesktop/UPower org.freedesktop.UPower OnBattery` returning a clean boolean on a live system. **No new dependency for phase 3.** |
| `~/.cargo/git/checkouts/libcosmic-.../Cargo.toml:55` | `rfd = ["dep:rfd"]` - libcosmic's optional native file-dialog feature (same shape as the `markdown` feature enabled for patch notes). Phase 2 enables this the same way. |

Notes that save dead ends:

- **system76-power was considered and rejected for phase 3.** Checked live
  (`busctl list` on the system bus): not running on this (Arch) machine at
  all, only `org.freedesktop.UPower` and `power-profiles-daemon` are.
  cosmic-wpengine already targets non-Pop!_OS COSMIC installs (this dev
  machine is proof, and the whole static-ffmpeg saga was specifically about
  not assuming a Pop!_OS-shaped environment) - UPower is the portable,
  correct choice and ships on essentially every modern Linux desktop.
- **Phase 1 and phase 2 both only ever set existing config fields.** Neither
  needs new `ResolvedBackground` variants or new renderer-side branches -
  `resolved_background()` and `video_background_path`'s consumers
  (`main.rs`'s `spawn_video_watcher`) already do the right thing when the
  field changes out from under them via the config file watcher, since
  that's exactly how a user manually editing `config.toml` already behaves
  today.
- **Phase 3's UPower poll must not become a redraw source.** The render
  loop's `scene_dirty` gating (documented at `core/mod.rs`'s
  `scene_is_animating`) exists specifically to avoid needless redraws;
  battery status should feed into the *FPS cap*, not into `scene_dirty`.
- **Phase 4 is a real, separate feature, not a quick add.** Flagged
  honestly to the user as such (2026-07-20 conversation) - it needs
  monitor identification (new), a per-output config schema (new), and GUI
  for assigning per-monitor settings (new). Scope it as its own plan once
  started; the phase below is deliberately just the identification
  groundwork, not the full feature.

---

## Phase 1: Wallpaper rotation / playlist

Add `appearance.rotation: Option<RotationConfig>` (or a flatter
`rotation_enabled: bool` + `rotation_interval_minutes: u64`, matching
`WeatherConfig::poll_interval_minutes`'s existing shape) to `AppearanceConfig`.
A new lightweight watcher (mirrors `spawn_video_watcher` in `main.rs`) ticks
on the configured interval and, when enabled, advances
`video_background_path` to the next entry in `Config::available_videos()`
(shuffled or sequential - sequential first, shuffle is a one-line
`rand`-free `fastrand`-or-index-rotation follow-up). GUI: a toggle + interval
dropdown on the Live Wallpapers page, next to the existing video list,
using the same `POLL_MINUTES`-style dropdown already built for weather.

**Verify:** live run with 2-3 videos imported and a short (~30s, dev-only)
interval; confirm the background actually cycles and the config file's
`video_background_path` updates each tick.

## Phase 2: Independent custom image wallpaper picker

Enable libcosmic's `rfd` feature (`features = ["markdown", "rfd"]`,
alongside the already-enabled `markdown`). Add a "Browse..." button to the
Wallpaper page's Frosted Glass section that opens a native file dialog
(image filter: png/jpg/webp, matching `image` crate's supported decoders)
and sets `appearance.custom_background_path` directly - independent of
whatever COSMIC's own desktop wallpaper is set to. Needs a small UX
decision: does setting a custom image implicitly disable the
cosmic-bg-tracking behavior permanently, or only until cleared? (Lean
toward: setting it always wins per `resolved_background()`'s existing
precedence - already true today - so no engine change needed, just
surfacing the control.) Add a "Clear" action to fall back to tracking the
system wallpaper again.

**Verify:** live run; pick an image via the dialog, confirm the frosted
background updates to it and *stops* following subsequent system wallpaper
changes until cleared.

## Phase 3: Pause (throttle) on battery

New `modules::power` (engine-side): a background task polling
`org.freedesktop.UPower`'s `OnBattery` property via the existing `dbus`
crate (either a cheap interval poll matching the weather watcher's
structure, or - better - subscribing to the property's `PropertiesChanged`
signal for instant reaction with zero polling cost). Feed the result into
`Renderer` as a new cached bool, read once per run-loop tick alongside the
existing `target_fps` read (`core/mod.rs:213`). When on battery, cap FPS
sharply (e.g. min(target_fps, 10) - concrete number is a GUI-exposed
setting, matching `Frame rate limit`'s existing slider) rather than
stopping rendering entirely, so the wallpaper doesn't visibly freeze.
GUI: a toggle on the General page ("Reduce frame rate on battery") next to
the existing Frame rate limit slider, plus the reduced-rate value itself as
a second small control.

**Verify:** toggle `OnBattery` via `busctl --system set-property
org.freedesktop.UPower /org/freedesktop/UPower org.freedesktop.UPower
OnBattery b true` (UPower allows this for testing without unplugging
anything) and confirm the engine's logged FPS drops accordingly; toggle
back and confirm it recovers.

## Phase 4: Per-monitor wallpaper identification (groundwork only)

Scoped down from the full feature. Extend `WaylandWindowInfo` with a
`name: String` / `description: String` (via smithay-client-toolkit's
`OutputState::info(&output)`, called in `new_output`) so outputs are
durably identifiable across reconnects/reorders - today `renderer.outputs`
is purely positional (index into the same `Vec` as
`wayland_manager.app_data.windows`), which is exactly why per-monitor
config isn't possible yet. This phase only adds the identification and a
read-only "Detected monitors" list on the General page (validating the
names are stable and sensible on real multi-monitor hardware) - it
deliberately does **not** yet add per-monitor config schema, per-monitor
theme selection, or the GUI to assign them. Write a fresh, dedicated plan
for the full feature once this groundwork lands and its output naming has
been validated on real hardware.

**Verify:** live run on a multi-monitor setup (or simulated via a second
virtual output if unavailable); confirm detected names are stable across a
monitor unplug/replug.

---

## Explicitly out of scope

- Workshop/sharing hub - needs a backend service.
- Web-based (HTML/JS) wallpapers - needs an embedded browser engine
  (CEF-class dependency), a different rendering pipeline entirely.
- Mouse-reactive parallax - already tracked in `ROADMAP.md`'s "Unscheduled
  ideas"; genuinely hard on Wayland specifically (layer-shell background
  surfaces don't receive pointer input by design), not a quick add despite
  how simple it can look at a glance.
