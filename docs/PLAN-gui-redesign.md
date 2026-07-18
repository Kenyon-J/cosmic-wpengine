# PLAN: Settings GUI redesign

Graduated from [ROADMAP.md](ROADMAP.md). Reshapes the single-page settings
window into a sidebar-paged app in the COSMIC System Settings idiom, with a
drag-and-drop Live Wallpapers library. An interactive HTML mockup of the
target design exists (Joshua has the link; ask him if you need it — it shows
the seven pages, the style cards, and the video library grid).

**Progress convention: keep this file updated as phases land — it is the
handoff document if a session ends mid-work.**

## Target shape (from the approved mockup)

Sidebar groups → pages:

- **Background**: Wallpaper · Live Wallpapers · Layout Themes
- **Overlays**: Now Playing · Visualiser · Weather
- **Engine**: General

Principles: pages mirror the desktop's layers; options render only when the
style they belong to is selected (progressive disclosure); media is a
thumbnail library, not a dropdown.

## Verified API facts (libcosmic pinned rev 6359a946)

- `Application::nav_model()` / `on_nav_select()` + `nav_bar::Model` give the
  sidebar; model built via `nav_bar::Model::builder().insert(|b| b.text(..).icon(..).data(Page))`
  (`src/widget/segmented_button/model/builder.rs`).
- `cosmic::widget::settings::{view_column, section, item}` produce the
  card-with-rows layout natively (`src/widget/settings/`).
- File drop events exist: `iced::core::window::Event::{FileHovered, FileDropped}`
  (`iced/core/src/window/event.rs:56,66`).

## Current GUI inventory (pre-redesign)

`src/modules/gui/`: `mod.rs` (SettingsApp, Message enum ~25 variants,
update(), debounced-save machinery), `view.rs` (single 460-line page),
`updater.rs` (self-update flow), `tests.rs`.
Controls: background-mode pick_list, video pick_list, blur slider, 5
checkboxes (art/lyrics/autostart/weather/effects), font pick_list, fps
pick_list, TOML editor w/ file dropdown + Save, Apply/Create theme, Open
Folder, Report Issue / Patch Notes / update button, status_msg line.

## Phases

### Phase 1 — infrastructure: nav + pages (CODE COMPLETE, verifying)

Pure restructure plus a few free wins; no drag-and-drop yet.

- [x] `Page` enum + `nav_bar::Model` in `SettingsApp`; `nav_model`/`on_nav_select`
- [x] Split `view.rs` into one fn per page using `settings::section` rows
- [x] Wallpaper page: style pick_list + conditional Frosted Glass section
      (blur slider); Video style points at Live Wallpapers page
- [x] Live Wallpapers page: video list + Open Folder (grid/DnD is Phase 2)
- [x] Layout Themes page: theme pick_list + Apply/Create + Open Folder;
      dropped the inline TOML editor (replaced by Open Folder + engine's
      live reload; removed `EditorAction`/`SaveFile` messages and
      `editor_content`)
- [x] Now Playing page: art toggle, lyrics toggle, font family
- [x] Visualiser page: NEW rows for `bands` / `smoothing`
- [x] Weather page: enable + hide-effects toggles, NEW temperature-unit row
- [x] General page: autostart, fps slider, About section (version/update
      with self-updatable fallback to release page/patch notes/report
      issue), Open Config Folder, videos-folder shortcut
- [x] Toggles use `settings::item::builder(...).toggler(...)` rows
- [x] Builds clean (0 warnings), 66 tests pass, GUI launches; Wallpaper page
      verified on-screen (sidebar + conditional frost card render, config
      values load)
- [ ] Joshua to click through the remaining pages and confirm each control
      writes config + the engine live-reloads (no input automation available
      on Wayland to do this hands-off)

2026-07-18 evening: Phases 1+2 signed off by Joshua and installed as the
system GUI (`~/.local/bin/cosmic-wallpaper-gui`). Fixed en route: thumbnail
overflow (image widget doesn't clip Cover; use fixed-box Contain),
tech-jargon subtitles, and the stuck-frame bug when switching Video →
Frosted (engine now reloads the wallpaper on the video's Some→None
transition). Next: Phase 3.

Feedback round 1 (2026-07-18): sidebar felt laggy → was the unoptimized
debug build, plus per-render `Vec<String>` clones feeding iced pick_lists;
pick_list overlay menus also rendered translucent on the frosted system
theme. Fixed by converting all five pick_lists to `cosmic::widget::dropdown`
(opaque themed menus, borrowed `Cow` selections — no per-frame clones;
selection messages are now index-based: `FontSelected`/`ThemeSelected`/
`VideoSelected(usize)`), and testing against the release build.

### Phase 2 — Live Wallpapers library (CODE COMPLETE, verifying)

- [x] Grid of tiles (3-across chunked rows), Active tile highlighted with
      `theme::Button::Suggested`; click = `VideoSelected(idx)` → sets
      `video_background_path` (mode derivation is video-first, so this also
      switches the style)
- [x] Drag-and-drop import via `cosmic::widget::dnd_destination::
      dnd_destination_for_data::<DroppedFiles>` (`text/uri-list` payload
      type in `gui/library.rs`, parsed with the `url` crate) — NOT iced's
      `FileDropped` window event, which doesn't fire under the Wayland sctk
      backend; copies into the videos dir, then rescans
- [x] Thumbnails: `gui/library.rs::scan()` (spawn_blocking at startup and
      after imports) probes duration and decodes the first frame with
      ffmpeg-next into `videos/.thumbs/<name>.png` at 320px wide
- [x] "Prefer Spotify Canvas" toggle: `appearance.prefer_canvas`
      (serde-default true) gates `spawn_canvas_decoder` at both mpris call
      sites + an instant gate on `Event::CanvasVideoFrame` in the renderer
- [x] Verified: startup scan extracted a correct thumbnail for a generated
      test clip (`videos/test-clip.mp4`, safe to delete); engine + GUI
      release builds installed and running
- [ ] Joshua to verify: drag a video from Files onto the page (hover
      highlight + import), tile click switches the wallpaper, canvas toggle
      with Spotify playing

### Phase 3 — Wallpaper page polish + colour picker (CODE COMPLETE, verifying)

- [x] Style cards replacing the dropdown: real previews (blurred/sharp
      wallpaper snapshots, first video thumbnail, palette gradient, music
      icon) in a wrapping `flex_row`; selection = accent border via
      `theme::Button::Custom` (Suggested fill looked like a giant pill)
- [x] Frosted Glass live preview strip: `Stack` of sharp wallpaper +
      blurred copy at `Image::opacity(blur_opacity)` + glass-tint layer +
      sample lyric in the colour the engine would pick. Wallpaper loaded
      via the engine's own `resolved_background()`; snapshots prepared
      off-thread in `build_wallpaper_preview` (gaussian approximation of
      the Kawase look)
- [x] Text colour: `appearance.text_color: Option<[f32;3]>` (None =
      automatic) short-circuits `update_text_colors`; GUI has
      Automatic/Custom dropdown + native `ColorPickerModel` (save applies,
      Reset returns to automatic)
- [x] Weather: lat/lon text inputs (validated, debounced) + poll-interval
      dropdown
- [ ] Joshua to verify: card clicks, picker flow end-to-end with the
      engine running, weather inputs. NOTE: engine binary with text_color
      support is installed but NOT started (Joshua had quit it while
      gaming); GUI running the phase 3 build.

Known nits: image widget clips nothing at this rev, so every preview uses
fixed-box Contain; preview strip is loaded once at startup (wallpaper
changes mid-session won't refresh it until reopen).

## Status log

- 2026-07-18: Plan written; API facts verified against the pinned checkout.
  Phase 1 implementation starting.
- 2026-07-18 (later): Phase 1 code complete, first compile in progress.
  What changed:
  - `mod.rs`: `Page` enum; `nav: nav_bar::Model` built in `init()` (7 pages,
    symbolic icons); `nav_model()`/`on_nav_select()`; `current_background_mode()`
    moved from view; `load_files()` → `load_themes()` (bare names matching
    `audio.style`); Message enum: removed `FileSelected`/`EditorAction`/`SaveFile`,
    added `ThemeSelected`/`BandsChanged`/`SmoothingChanged`/
    `TemperatureUnitSelected`/`ClosePatchNotes`/`OpenVideosFolder`; editor
    fields (`editor_content`, `selected_file`, `available_files`) replaced by
    `selected_theme`/`available_themes`/`patch_notes`; patch notes render on
    the General page instead of the editor.
  - `view.rs`: full rewrite - `view_app` dispatches on `nav.active_data::<Page>()`;
    one fn per page built from `settings::section()` + `settings::item::builder`
    (togglers via `.toggler(...)`); shared `page()` scaffold with title,
    summary and the `status_msg` caption.
  - `is_safe_path` + its tests untouched (still guards CreateTheme).
  Remaining for Phase 1: fix compile errors from this pass, run the GUI,
  click through pages, verify config writes + engine live-reload.
