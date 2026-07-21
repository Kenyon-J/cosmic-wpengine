mod bootstrap;
mod library;
#[cfg(test)]
mod tests;
mod updater;
mod view;
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use cosmic::app::Core;
use cosmic::iced::Task;
use cosmic::widget::color_picker::{ColorPickerModel, ColorPickerUpdate};
use cosmic::widget::{icon, nav_bar};
use cosmic::Application;
use cosmic_text::fontdb;
use cosmic_wallpaper::modules::config::ResolvedBackground;

// Import the shared modules from your newly created library crate!
use cosmic_wallpaper::modules::config;
use cosmic_wallpaper::modules::utils::resolve_binary;

fn main() -> cosmic::iced::Result {
    // Previously uninitialised: every `tracing::warn!`/`info!` call in this
    // binary (bootstrap.rs, updater.rs, the folder-open handlers below) had
    // nowhere to go and silently no-opped.
    cosmic_wallpaper::modules::logging::init("gui");
    // Launch the libcosmic application
    cosmic::app::run::<SettingsApp>(cosmic::app::Settings::default(), ())
}

struct SettingsApp {
    core: Core,
    nav: nav_bar::Model,
    wp_config: config::Config,
    available_fonts: Vec<String>,
    available_themes: Vec<String>,
    /// Scanned video library with thumbnails/durations; loaded async at
    /// startup and after imports.
    library: Vec<library::VideoEntry>,
    /// True while a drag hovers the Live Wallpapers drop zone.
    drop_hover: bool,
    selected_theme: Option<String>,
    new_theme_name: String,
    status_msg: String,
    autostart: bool,
    update_state: UpdateState,
    /// Fetched release notes, shown on the General page when present.
    /// Pre-parsed once on arrival (`markdown::Item` owns its data - no
    /// lifetime tie to the source text) rather than reparsed on every view.
    patch_notes: Option<Vec<cosmic::widget::markdown::Item>>,
    /// Wallpaper snapshots for the Wallpaper page previews; loaded async at
    /// startup from the same background resolution the engine uses.
    wallpaper_preview: Option<WallpaperPreview>,
    /// Picker state for the custom text colour.
    color_picker: ColorPickerModel,
    /// Text buffers for the weather location inputs (kept as typed even
    /// while invalid; only valid coordinates are saved).
    lat_input: String,
    lon_input: String,
    /// Parsed layout of `selected_theme`, edited live by the theme editor.
    edit_theme: Option<config::ThemeLayout>,
    /// Index into `view::THEME_ELEMENTS` - which element's controls show.
    theme_element: usize,
    /// Debounce generation for theme-file writes (mirrors `save_generation`).
    theme_save_generation: u64,
    /// Engine process, when running (refreshed on General, and after
    /// Start/Stop).
    engine_pid: Option<u32>,
    /// Why the engine is not running, when systemd (or a failed Start)
    /// knows: shown on the General page's engine row so a binary that dies
    /// before main() stops failing invisibly.
    engine_failure: Option<String>,
    /// `None` when the app is properly registered with the desktop's
    /// launcher (or a packaged install, whose job that isn't ours);
    /// `Some(reason)` surfaces a "Setup" section on General - the app
    /// silently missing from the launcher was itself a real, only
    /// discovered-by-chance gap (see bootstrap.rs).
    launcher_issue: Option<String>,
    /// Monotonic counter pairing each slider change with its debounce timer;
    /// a DebouncedSave only writes to disk if its generation is still the
    /// newest (i.e. the slider settled for the full window).
    save_generation: u64,
}

/// Pre-rendered wallpaper snapshots for the Wallpaper page: a wide 3:1
/// strip (sharp + gaussian-blurred) for the frosted-glass preview, and a
/// 16:9 card pair for the style cards.
#[derive(Debug, Clone)]
struct WallpaperPreview {
    strip_sharp: cosmic::widget::image::Handle,
    strip_blurred: cosmic::widget::image::Handle,
    card_sharp: cosmic::widget::image::Handle,
    card_blurred: cosmic::widget::image::Handle,
    /// Mean sRGB of the wallpaper, for previewing the adaptive text colour.
    mean: [f32; 3],
}

/// A single edit from the theme editor, applied to the selected element of
/// `edit_theme` in `update`.
#[derive(Debug, Clone, Copy)]
enum ThemeEditMsg {
    PosX(f32),
    PosY(f32),
    Size(f32),
    Rotation(f32),
    Amplitude(f32),
    /// Index into the element's shape options (art: square/circular;
    /// visualiser: linear/circular/square).
    Shape(usize),
    /// Index into left/center/right.
    Align(usize),
    Bounce(f32),
    Stiffness(f32),
    Damping(f32),
    BeatPulse(f32),
    /// Circular visualiser: capture the album art into the ring.
    DockArt(bool),
}

/// One page of the settings window, keyed off the sidebar selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Wallpaper,
    LiveWallpapers,
    Themes,
    NowPlaying,
    Visualiser,
    Weather,
    General,
}

#[derive(Debug, Clone, PartialEq)]
enum UpdateState {
    UpToDate,
    Available(String),
    /// Holds the tag being installed so a failed attempt can fall back to
    /// `Available(tag)` (offering a retry) instead of losing track of it.
    Updating(String),
    Installed(String),
}

impl SettingsApp {
    /// Derives the mode shown by the Wallpaper page from the config flags,
    /// video-first (matching the engine's own precedence).
    fn current_background_mode(&self) -> BackgroundMode {
        if self.wp_config.appearance.video_background_path.is_some() {
            BackgroundMode::Video
        } else if self.wp_config.appearance.album_color_background {
            BackgroundMode::AlbumPalette
        } else if self.wp_config.appearance.album_art_background {
            BackgroundMode::AlbumArt
        } else if self.wp_config.appearance.transparent_background {
            BackgroundMode::Transparent
        } else {
            BackgroundMode::FrostedGlass
        }
    }

    /// Debounces disk writes for slider-driven settings. libcosmic sliders
    /// emit a value on every drag step; saving each one writes config.toml
    /// dozens of times per drag and makes the engine's file watcher reload
    /// per step. The in-memory config (which the slider renders from) is
    /// updated immediately by the caller; the save fires only once no newer
    /// change has arrived for the settle window.
    fn schedule_debounced_save(&mut self) -> Task<cosmic::Action<Message>> {
        self.save_generation = self.save_generation.wrapping_add(1);
        let generation = self.save_generation;
        Task::perform(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                generation
            },
            |generation| Message::DebouncedSave(generation).into(),
        )
    }

    /// Theme-file counterpart of [`Self::schedule_debounced_save`].
    fn schedule_theme_save(&mut self) -> Task<cosmic::Action<Message>> {
        self.theme_save_generation = self.theme_save_generation.wrapping_add(1);
        let generation = self.theme_save_generation;
        Task::perform(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                generation
            },
            |generation| Message::DebouncedThemeSave(generation).into(),
        )
    }

    /// Serialises the edited layout to its theme file. The engine watches
    /// the shaders directory, so a successful write is a live reload.
    fn write_theme_file(&mut self) {
        let (Some(name), Some(layout)) = (&self.selected_theme, &self.edit_theme) else {
            return;
        };
        let rel = format!("shaders/{name}.toml");
        if !is_safe_path(&rel) {
            self.status_msg = format!("Blocked unsafe theme name: {name}");
            return;
        }
        match toml::to_string_pretty(layout) {
            Ok(text) => {
                let path = config::Config::config_dir().join(&rel);
                match std::fs::write(&path, text) {
                    Ok(()) => {
                        self.status_msg = if self.wp_config.audio.style == *name {
                            format!("Saved {name} - the desktop is showing your change.")
                        } else {
                            format!("Saved {name}. Apply it to see it on the desktop.")
                        };
                    }
                    Err(e) => self.status_msg = format!("Error saving theme: {e}"),
                }
            }
            Err(e) => self.status_msg = format!("Error serialising theme: {e}"),
        }
    }

    /// (Re)loads the selected theme's layout into the editor.
    fn load_edit_theme(&mut self) {
        self.edit_theme = self
            .selected_theme
            .as_ref()
            .map(|name| config::ThemeLayout::load(name));
    }

    /// Re-reads the engine's liveness, and - only when it is down - asks
    /// systemd whether the login autostart failed, so the engine row can say
    /// why instead of just "Not running".
    fn refresh_engine_status(&mut self) {
        self.engine_pid = find_engine_pid();
        if self.engine_pid.is_some() {
            self.engine_failure = None;
        } else if self.engine_failure.is_none() {
            self.engine_failure = engine_autostart_failure();
        }
    }
}

/// The file `Create Theme` writes: the complete default layout with every
/// key present and explained, so a new theme's file teaches the format.
/// Keep in sync with `ThemeLayout` in config/types.rs.
const THEME_TEMPLATE: &str = r#"# cosmic-wallpaper layout theme.
# Positions are [x, y] fractions of the screen: [0.0, 0.0] is the top-left
# corner, [1.0, 1.0] the bottom-right. Every key is optional - missing keys
# use the defaults shown here. The engine reloads this file as you save it.

[album_art]
position = [0.5, 0.5]
size = 0.25          # fraction of screen height
shape = "square"     # "square" or "circular"

[track_info]
position = [0.5, 0.1]
align = "center"     # "left", "center" or "right"
size = 1.0           # font scale multiplier

[lyrics]
position = [0.5, 0.85]
align = "center"
size = 1.0

[weather]
position = [0.98, 0.05]
align = "right"
size = 1.0

[visualiser]
shape = "linear"     # "linear", "circular" or "square"
position = [0.5, 0.5]
size = 0.25          # bar span (linear) or ring radius (circular)
rotation = 0.0       # degrees
amplitude = 1.0      # bar height multiplier
align = "center"     # band ordering: "left", "center" or "right"
dock_art = true      # circular only: album art follows the ring
# color_top = [1.0, 0.2, 0.5]     # override the album-palette colours
# color_bottom = [0.2, 0.5, 1.0]

[effects]
lyric_bounce = 1.0             # how far the active lyric hops on the beat
lyric_spring_stiffness = 150.0 # snappiness of the lyric scroll
lyric_spring_damping = 12.0    # wobble control: lower = bouncier
beat_pulse = 1.0               # visualiser pulse on detected beats

# font_family = "Fira Sans"    # this theme's font (user setting wins)
"#;

/// Applies one editor change to the element at `element` (index into
/// `view::THEME_ELEMENTS`: art, track, lyrics, visualiser, weather,
/// effects). Messages that don't apply to the element are ignored.
fn apply_theme_edit(layout: &mut config::ThemeLayout, element: usize, edit: ThemeEditMsg) {
    use config::{ArtShape, TextAlign, VisAlign, VisShape};

    const TEXT_ALIGNS: [TextAlign; 3] = [TextAlign::Left, TextAlign::Center, TextAlign::Right];
    const VIS_ALIGNS: [VisAlign; 3] = [VisAlign::Left, VisAlign::Center, VisAlign::Right];
    const ART_SHAPES: [ArtShape; 2] = [ArtShape::Square, ArtShape::Circular];
    const VIS_SHAPES: [VisShape; 3] = [VisShape::Linear, VisShape::Circular, VisShape::Square];

    fn text_layout(
        layout: &mut config::ThemeLayout,
        element: usize,
    ) -> Option<&mut config::TextLayout> {
        match element {
            1 => Some(&mut layout.track_info),
            2 => Some(&mut layout.lyrics),
            4 => Some(&mut layout.weather),
            _ => None,
        }
    }

    match edit {
        ThemeEditMsg::PosX(v) => match element {
            0 => layout.album_art.position[0] = v,
            3 => layout.visualiser.position[0] = v,
            _ => {
                if let Some(t) = text_layout(layout, element) {
                    t.position[0] = v;
                }
            }
        },
        ThemeEditMsg::PosY(v) => match element {
            0 => layout.album_art.position[1] = v,
            3 => layout.visualiser.position[1] = v,
            _ => {
                if let Some(t) = text_layout(layout, element) {
                    t.position[1] = v;
                }
            }
        },
        ThemeEditMsg::Size(v) => match element {
            0 => layout.album_art.size = v,
            3 => layout.visualiser.size = v,
            _ => {
                if let Some(t) = text_layout(layout, element) {
                    t.size = v;
                }
            }
        },
        ThemeEditMsg::Rotation(v) => layout.visualiser.rotation = v,
        ThemeEditMsg::Amplitude(v) => layout.visualiser.amplitude = v,
        ThemeEditMsg::Shape(idx) => match element {
            0 => {
                if let Some(shape) = ART_SHAPES.get(idx) {
                    layout.album_art.shape = *shape;
                }
            }
            3 => {
                if let Some(shape) = VIS_SHAPES.get(idx) {
                    layout.visualiser.shape = *shape;
                }
            }
            _ => {}
        },
        ThemeEditMsg::Align(idx) => match element {
            3 => {
                if let Some(align) = VIS_ALIGNS.get(idx) {
                    layout.visualiser.align = *align;
                }
            }
            _ => {
                if let Some(t) = text_layout(layout, element) {
                    if let Some(align) = TEXT_ALIGNS.get(idx) {
                        t.align = *align;
                    }
                }
            }
        },
        ThemeEditMsg::DockArt(v) => layout.visualiser.dock_art = v,
        ThemeEditMsg::Bounce(v) => layout.effects.lyric_bounce = v,
        ThemeEditMsg::Stiffness(v) => layout.effects.lyric_spring_stiffness = v,
        ThemeEditMsg::Damping(v) => layout.effects.lyric_spring_damping = v,
        ThemeEditMsg::BeatPulse(v) => layout.effects.beat_pulse = v,
    }
}

/// Restores the element at `element` (same indexing as `apply_theme_edit`
/// and `view::THEME_ELEMENTS`) to what `style` actually ships with -
/// `monstercat`/`symmetric`/`waveform`'s own hand-tuned layout, or the
/// plain [0.5, 0.5]/circular/etc. baseline for `bars`/any custom name.
/// Uses `ThemeLayout::builtin_default`, not `load()`: by the time a style
/// has been edited in this editor at all, its autosave has already written
/// those edits to `shaders/<style>.toml`, so `load()` would just read them
/// straight back instead of restoring the shipped look.
fn reset_theme_element(layout: &mut config::ThemeLayout, style: &str, element: usize) {
    let defaults = config::ThemeLayout::builtin_default(style);
    match element {
        0 => layout.album_art = defaults.album_art,
        1 => layout.track_info = defaults.track_info,
        2 => layout.lyrics = defaults.lyrics,
        3 => layout.visualiser = defaults.visualiser,
        4 => layout.weather = defaults.weather,
        5 => layout.effects = defaults.effects,
        _ => {}
    }
}

/// Path to the engine binary: the GUI's sibling first (both binaries
/// install side by side - ~/.local/bin, /usr/bin, and the .deb all do
/// this; the updater relies on the same layout), falling back to the
/// trusted system paths for split installs.
fn engine_binary_path() -> Option<std::path::PathBuf> {
    if let Ok(gui) = std::env::current_exe() {
        if let Some(dir) = gui.parent() {
            let sibling = dir.join("cosmic-wallpaper");
            if sibling.exists() {
                return Some(sibling);
            }
        }
    }
    resolve_binary("cosmic-wallpaper")
}

/// systemd unit the xdg-autostart generator creates for the engine's
/// autostart entry. Its failure state is the only surviving record of a
/// binary that died before main() at login - e.g. exit 127 from the dynamic
/// linker after a system update changed a library soname - which otherwise
/// fails completely silently (found 2026-07-19: an ffmpeg major bump left
/// the engine dead all session with no visible error anywhere).
const ENGINE_AUTOSTART_UNIT: &str = "app-io.github.kenyon_j.cosmic_wpengine@autostart.service";

/// Asks systemd whether the engine's login autostart failed, translating
/// the exit status into an actionable message. None when the unit is fine,
/// unknown, or systemctl is unavailable.
fn engine_autostart_failure() -> Option<String> {
    let systemctl = resolve_binary("systemctl")?;
    let output = std::process::Command::new(systemctl)
        .args([
            "--user",
            "show",
            ENGINE_AUTOSTART_UNIT,
            "--property=ActiveState,ExecMainStatus",
        ])
        .output()
        .ok()?;
    parse_unit_failure(&String::from_utf8_lossy(&output.stdout))
}

/// Pure parse of `systemctl show --property=ActiveState,ExecMainStatus`
/// output, split out of [`engine_autostart_failure`] for testing.
fn parse_unit_failure(show_output: &str) -> Option<String> {
    let prop = |name: &str| {
        show_output.lines().find_map(|line| {
            line.strip_prefix(name)
                .and_then(|rest| rest.strip_prefix('='))
        })
    };
    if prop("ActiveState") != Some("failed") {
        return None;
    }
    Some(match prop("ExecMainStatus") {
        // 127 is the shell/exec convention for "could not run at all" -
        // for a binary that exists, that means the dynamic linker bailed.
        Some("127") => "The engine failed to start at login: its binary could not load \
             (exit 127, usually missing libraries after a system update). \
             Rebuild or reinstall it, then press Start."
            .to_string(),
        Some(code) if !code.is_empty() => format!(
            "The engine failed to start at login (exit {code}). \
             See: journalctl --user -u {ENGINE_AUTOSTART_UNIT}"
        ),
        _ => format!(
            "The engine failed to start at login. \
             See: journalctl --user -u {ENGINE_AUTOSTART_UNIT}"
        ),
    })
}

/// The most informative line of the engine's captured stderr: the dynamic
/// linker's message when present (the exit-127 case this capture exists
/// for), else the first non-empty line.
fn stderr_headline(stderr: &str) -> String {
    stderr
        .lines()
        .find(|l| l.contains("error while loading shared libraries"))
        .or_else(|| stderr.lines().find(|l| !l.trim().is_empty()))
        .unwrap_or("")
        .trim()
        .to_string()
}

/// `PRETTY_NAME` out of `/etc/os-release` - the one line most useful for
/// telling apart "which Linux" in a bug report, without parsing the whole
/// (semi-standardised, but not worth a crate for one field) file format.
fn distro_pretty_name() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| "Unknown Linux distribution".to_string())
}

/// Full plain-text snapshot for the "Copy Diagnostics" button: version,
/// distro, engine status, and each binary's recent log tail (including GPU
/// adapter selection, logged once at engine startup - see
/// renderer::core::init).
fn build_diagnostics_text(app: &SettingsApp) -> String {
    use cosmic_wallpaper::modules::logging;
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(out, "cosmic-wallpaper {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(out, "OS: {}", distro_pretty_name());
    let _ = writeln!(
        out,
        "Engine: {}",
        match (app.engine_pid, &app.engine_failure) {
            (Some(pid), _) => format!("running (pid {pid})"),
            (None, Some(failure)) => failure.clone(),
            (None, None) => "not running".to_string(),
        }
    );
    let _ = writeln!(out, "Mode: {:?}", app.wp_config.mode);
    let _ = writeln!(out, "Theme: {}", app.wp_config.audio.style);

    for component in ["engine", "gui"] {
        let lines = logging::tail_lines(component, 40);
        let _ = writeln!(
            out,
            "\n--- {component} log (last {} lines) ---",
            lines.len()
        );
        if lines.is_empty() {
            let _ = writeln!(out, "(no log file yet)");
        }
        for line in lines {
            let _ = writeln!(out, "{line}");
        }
    }

    out
}

/// Compact excerpt for the prefilled GitHub issue body: version, distro,
/// and only the ERROR/WARN lines (not the full diagnostics dump) - long
/// enough to be useful, short enough to stay a sane URL length.
fn build_issue_body() -> String {
    use cosmic_wallpaper::modules::logging;
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(out, "**cosmic-wallpaper {}**", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(out, "OS: {}", distro_pretty_name());
    let _ = writeln!(
        out,
        "\n<!-- What were you doing when this happened? -->\n\n"
    );

    let mut errors = logging::tail_error_lines("engine", 12);
    errors.extend(logging::tail_error_lines("gui", 5));
    if !errors.is_empty() {
        let _ = writeln!(out, "<details><summary>Recent errors/warnings</summary>\n");
        let _ = writeln!(out, "```");
        for line in errors {
            let _ = writeln!(out, "{line}");
        }
        let _ = writeln!(out, "```\n</details>");
    }

    out
}

/// PID of a running wallpaper engine, found by cmdline (comm truncates to
/// 15 chars, which cannot distinguish the engine from this GUI).
fn find_engine_pid() -> Option<u32> {
    let entries = std::fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(cmdline) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let arg0 = cmdline.split(|b| *b == 0).next().unwrap_or_default();
        let arg0 = String::from_utf8_lossy(arg0);
        if std::path::Path::new(arg0.as_ref())
            .file_name()
            .is_some_and(|n| n == "cosmic-wallpaper")
        {
            return Some(pid);
        }
    }
    None
}

/// Center-crops `img` to `ratio` (w/h) and resizes to `w`x`h`.
fn crop_resize(img: &image::RgbaImage, w: u32, h: u32) -> image::RgbaImage {
    let (sw, sh) = img.dimensions();
    let ratio = w as f32 / h as f32;
    let (cw, ch) = if sw as f32 / sh as f32 > ratio {
        (((sh as f32 * ratio) as u32).clamp(1, sw), sh)
    } else {
        (sw, ((sw as f32 / ratio) as u32).clamp(1, sh))
    };
    let cropped = image::imageops::crop_imm(img, (sw - cw) / 2, (sh - ch) / 2, cw, ch).to_image();
    image::imageops::resize(&cropped, w, h, image::imageops::FilterType::Triangle)
}

/// Loads the same background the engine resolves (wallpaper file, colour or
/// gradient) and prepares the preview snapshots.
fn load_wallpaper_preview_task(
    appearance: config::AppearanceConfig,
) -> Task<cosmic::Action<Message>> {
    Task::perform(
        async move {
            let resolved = appearance.resolved_background().await;
            tokio::task::spawn_blocking(move || build_wallpaper_preview(resolved).map(Box::new))
                .await
                .ok()
                .flatten()
        },
        |preview| Message::WallpaperPreviewLoaded(preview).into(),
    )
}

fn build_wallpaper_preview(resolved: Option<ResolvedBackground>) -> Option<WallpaperPreview> {
    use cosmic_wallpaper::modules::renderer::utils as render_utils;
    let img: image::RgbaImage = match resolved? {
        ResolvedBackground::Image(path) => image::open(path).ok()?.to_rgba8(),
        ResolvedBackground::Colour(colour) => render_utils::solid_colour_image(colour),
        ResolvedBackground::Gradient { colors, angle_deg } => {
            render_utils::gradient_image(&colors, angle_deg, 480, 160)
        }
    };
    let mean = cosmic_wallpaper::modules::colour::average_colour(&img);

    let handle = |img: &image::RgbaImage| {
        cosmic::widget::image::Handle::from_rgba(img.width(), img.height(), img.clone().into_raw())
    };
    let strip = crop_resize(&img, 480, 160);
    let card = crop_resize(&img, 160, 90);
    Some(WallpaperPreview {
        strip_sharp: handle(&strip),
        strip_blurred: handle(&image::imageops::blur(&strip, 8.0)),
        card_sharp: handle(&card),
        card_blurred: handle(&image::imageops::blur(&card, 5.0)),
        mean,
    })
}

/// Rescans the video library (thumbnail extraction included) off the UI
/// thread.
fn scan_library_task() -> Task<cosmic::Action<Message>> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(library::scan)
                .await
                .unwrap_or_default()
        },
        |entries| Message::LibraryLoaded(entries).into(),
    )
}

fn autostart_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(home).join(".config/autostart")
}

/// The canonical autostart entry - the same file name the .deb installs, so
/// the toggle and package-managed autostart agree on one file.
fn autostart_path() -> std::path::PathBuf {
    autostart_dir().join("io.github.kenyon_j.cosmic_wpengine.desktop")
}

/// A stale name this toggle wrote before it matched the packaged entry;
/// removed on disable so old installs can't end up starting two engines.
fn legacy_autostart_path() -> std::path::PathBuf {
    autostart_dir().join("cosmic-wallpaper.desktop")
}

fn autostart_enabled() -> bool {
    autostart_path().exists() || legacy_autostart_path().exists()
}

fn set_autostart(enable: bool) {
    // If we are running inside a Flatpak sandbox, use the XDG Background Portal
    if std::path::Path::new("/.flatpak-info").exists() {
        let enable_str = if enable { "true" } else { "false" };
        // Execute a D-Bus call to the portal using busctl (standard in Freedesktop runtimes)
        if let Some(busctl) = resolve_binary("busctl") {
            let _ = std::process::Command::new(busctl)
                .args([
                    "--user",
                    "call",
                    "org.freedesktop.portal.Desktop",
                    "/org/freedesktop/portal/desktop",
                    "org.freedesktop.portal.Background",
                    "RequestBackground",
                    "sa{sv}",
                    "", // parent_window
                    "1",
                    "autostart",
                    "b",
                    enable_str,
                ])
                .output();
        } else {
            tracing::warn!("Failed to set autostart: busctl not found in trusted PATH");
        }
        return;
    }

    let path = autostart_path();
    // Whatever the new state, the pre-1.2 file name must go: leaving it
    // alongside the canonical entry would start two engines at login.
    let _ = std::fs::remove_file(legacy_autostart_path());
    if enable {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Absolute Exec: the session's autostart environment does not
        // reliably include ~/.local/bin in PATH.
        let exec = engine_binary_path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "cosmic-wallpaper".to_string());
        let _ = std::fs::write(
            &path,
            format!(
                r#"[Desktop Entry]
Type=Application
Name=COSMIC Wallpaper Engine
Comment=Live wallpaper engine for the COSMIC desktop
Exec={exec}
Terminal=false
StartupNotify=false
X-GNOME-Autostart-enabled=true
"#
            ),
        );
    } else {
        let _ = std::fs::remove_file(path);
    }
}

pub(crate) fn is_safe_path(path_str: &str) -> bool {
    let path = std::path::Path::new(path_str);
    if path.is_absolute() {
        return false;
    }
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return false;
        }
    }
    true
}

/// Names of the visualiser layout themes (shaders/*.toml, without path or
/// extension) - the same names `audio.style` takes.
fn load_themes() -> Vec<String> {
    let mut themes = Vec::new();
    let shaders_dir = config::Config::config_dir().join("shaders");
    if let Ok(entries) = std::fs::read_dir(shaders_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if let Some(stem) = name.strip_suffix(".toml") {
                    themes.push(stem.to_string());
                }
            }
        }
    }
    themes.sort();
    themes
}

fn load_fonts() -> Vec<String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut font_names: Vec<String> = db
        .faces()
        .flat_map(|face| face.families.iter().map(|(name, _lang)| name.clone()))
        .collect();
    font_names.sort_unstable();
    font_names.dedup();
    // Add a "System Default" option
    font_names.insert(0, "System Default".to_string());
    font_names
}

#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: String,
}

static HTTP_CLIENT: std::sync::OnceLock<Result<reqwest::Client, String>> =
    std::sync::OnceLock::new();

fn get_http_client() -> Result<&'static reqwest::Client, &'static String> {
    HTTP_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent("cosmic-wallpaper/1.0")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
}

#[derive(serde::Deserialize)]
struct IpLocationResponse {
    latitude: f64,
    longitude: f64,
}

/// Estimates the user's location from their public IP via ipapi.co - opt-in
/// only, fired by the "Use my location" button, never automatically. No
/// SSRF concern here (unlike the mpris/video fetch paths): the URL is a
/// fixed constant, not derived from any untrusted input.
async fn fetch_ip_location() -> Result<(f64, f64), String> {
    let client = get_http_client().map_err(Clone::clone)?;
    let resp = client
        .get("https://ipapi.co/json/")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("location service returned HTTP {}", resp.status()));
    }

    const MAX_JSON_SIZE: usize = 1024 * 1024;
    let bytes = cosmic_wallpaper::modules::utils::read_capped(resp, MAX_JSON_SIZE)
        .await
        .map_err(|e| e.to_string())?;
    let data: IpLocationResponse = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;

    if !(-90.0..=90.0).contains(&data.latitude) || !(-180.0..=180.0).contains(&data.longitude) {
        return Err("location service returned out-of-range coordinates".to_string());
    }
    Ok((data.latitude, data.longitude))
}

async fn fetch_patch_notes() -> String {
    let url = "https://api.github.com/repos/Kenyon-J/cosmic-wpengine/releases/latest";

    let client = match get_http_client() {
        Ok(c) => c,
        Err(e) => return format!("Failed to build HTTP client: {}", e),
    };

    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => {
            const MAX_JSON_SIZE: usize = 10 * 1024 * 1024; // 10 MB limit
            let bytes =
                match cosmic_wallpaper::modules::utils::read_capped(resp, MAX_JSON_SIZE).await {
                    Ok(b) => b,
                    Err(e) => return format!("Failed to fetch patch notes: {}", e),
                };
            match serde_json::from_slice::<GitHubRelease>(&bytes) {
                Ok(release) => format!(
                    "COSMIC Wallpaper Engine {}\n\n{}",
                    release.tag_name, release.body
                ),
                Err(e) => format!("Failed to parse patch notes from GitHub: {}", e),
            }
        }
        Ok(resp) => format!("Failed to fetch patch notes: HTTP {}", resp.status()),
        Err(e) => format!("Failed to fetch patch notes: {}", e),
    }
}

async fn check_for_updates() -> Option<String> {
    let url = "https://api.github.com/repos/Kenyon-J/cosmic-wpengine/releases/latest";

    let client = get_http_client().ok()?;

    let resp = client.get(url).send().await.ok()?;
    const MAX_JSON_SIZE: usize = 10 * 1024 * 1024; // 10 MB limit
    let bytes = cosmic_wallpaper::modules::utils::read_capped(resp, MAX_JSON_SIZE)
        .await
        .ok()?;
    let release: GitHubRelease = serde_json::from_slice(&bytes).ok()?;
    let latest_version = release.tag_name.trim_start_matches('v');
    let current_version = env!("CARGO_PKG_VERSION");

    let is_newer = match (
        latest_version.split('.').collect::<Vec<_>>(),
        current_version.split('.').collect::<Vec<_>>(),
    ) {
        (l, c) if l.len() == 3 && c.len() == 3 => {
            let l_major: u32 = l[0].parse().unwrap_or(0);
            let l_minor: u32 = l[1].parse().unwrap_or(0);
            let l_patch: u32 = l[2].parse().unwrap_or(0);
            let c_major: u32 = c[0].parse().unwrap_or(0);
            let c_minor: u32 = c[1].parse().unwrap_or(0);
            let c_patch: u32 = c[2].parse().unwrap_or(0);

            l_major > c_major
                || (l_major == c_major && l_minor > c_minor)
                || (l_major == c_major && l_minor == c_minor && l_patch > c_patch)
        }
        _ => latest_version != current_version,
    };

    if is_newer {
        Some(release.tag_name)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackgroundMode {
    FrostedGlass,
    Transparent,
    AlbumArt,
    AlbumPalette,
    Video,
}

impl BackgroundMode {
    const ALL: [BackgroundMode; 5] = [
        BackgroundMode::FrostedGlass,
        BackgroundMode::Transparent,
        BackgroundMode::AlbumArt,
        BackgroundMode::AlbumPalette,
        BackgroundMode::Video,
    ];
}

impl std::fmt::Display for BackgroundMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackgroundMode::FrostedGlass => write!(f, "Frosted Glass (Blur)"),
            BackgroundMode::Transparent => write!(f, "Fully Transparent"),
            BackgroundMode::AlbumArt => write!(f, "Album Art Background"),
            BackgroundMode::AlbumPalette => write!(f, "Album Colour"),
            BackgroundMode::Video => write!(f, "Video Background"),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    BackgroundModeSelected(BackgroundMode),
    /// Index into `available_fonts` (cosmic dropdowns report selections by
    /// index; the string is resolved in `update`).
    FontSelected(usize),
    ToggleShowAlbumArt(bool),
    /// Index into `available_themes`.
    ThemeSelected(usize),
    /// Index into `library`.
    VideoSelected(usize),
    ToggleWatchCanvas(bool),
    LibraryLoaded(Vec<library::VideoEntry>),
    /// Files dropped on the Live Wallpapers page (None when the payload
    /// could not be decoded).
    FilesDropped(Option<library::DroppedFiles>),
    DndEntered,
    DndLeft,
    ImportDone {
        imported: usize,
        skipped: usize,
    },
    WallpaperPreviewLoaded(Option<Box<WallpaperPreview>>),
    /// 0 = automatic text colour, 1 = custom.
    TextColorMode(usize),
    TextColorPicker(ColorPickerUpdate),
    LatitudeChanged(String),
    LongitudeChanged(String),
    DetectLocation,
    LocationDetected(Result<(f64, f64), String>),
    /// Index into `view::POLL_MINUTES`.
    PollIntervalSelected(usize),
    /// Index into `view::THEME_ELEMENTS`.
    ThemeElementSelected(usize),
    ThemeEdit(ThemeEditMsg),
    /// Restores the currently-viewed element (`theme_element`) to
    /// `ThemeLayout::default()`'s values for it.
    ResetThemeElement,
    DebouncedThemeSave(u64),
    /// Theme .toml files dropped onto the Layout Themes page.
    ThemeFilesDropped(Option<library::DroppedFiles>),
    StartEngine,
    /// Result of the post-Start probe: `Some((exit code, stderr headline))`
    /// when the engine died within the probe window, `None` when it survived.
    EngineStartProbed(Option<(Option<i32>, String)>),
    StopEngine,
    RefreshEngineStatus,
    ApplyTheme,
    FpsChanged(f32),
    BlurOpacityChanged(f32),
    BandsChanged(f32),
    SmoothingChanged(f32),
    TemperatureUnitSelected(String),
    /// Fired by the settle timer a slider change starts; the payload is the
    /// generation at scheduling time, so only the most recent change's timer
    /// actually writes config.toml.
    DebouncedSave(u64),
    ToggleShowLyrics(bool),
    ToggleAutostart(bool),
    ToggleWeatherEnabled(bool),
    ToggleHideWeatherEffects(bool),
    NewThemeNameChanged(String),
    CreateTheme,
    ShowPatchNotes,
    PatchNotesLoaded(String),
    ClosePatchNotes,
    /// A link inside the rendered patch notes was clicked.
    PatchNotesLinkClicked(cosmic::widget::markdown::Uri),
    ReportIssue,
    CopyDiagnostics,
    UpdateCheckDone(Option<String>),
    StartUpdate,
    UpdateFinished(Result<String, String>),
    OpenUpdateLink,
    OpenConfigFolder,
    OpenVideosFolder,
}

impl Application for SettingsApp {
    type Executor = cosmic::iced::executor::Default;
    type Flags = ();
    type Message = Message;
    // Must match the shipped/installed .desktop file's name
    // (io.github.kenyon_j.cosmic_wpengine.desktop) and its Icon= key: this
    // constant becomes the window's live Wayland app_id, which the panel's
    // taskbar uses to look up a matching .desktop entry for the running
    // window's icon. A mismatch here (this was
    // "com.system76.CosmicWallpaperSettings" - a leftover template ID,
    // referenced nowhere else in the repo) left the app launcher showing
    // the right icon (it reads installed .desktop files directly) while
    // the taskbar fell back to a generic one (it couldn't find a
    // .desktop file for this app_id).
    const APP_ID: &'static str = "io.github.kenyon_j.cosmic_wpengine";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        // Off the UI thread: pure filesystem work with no result the UI
        // needs, and the first run writes ~0.6 MB of icons.
        std::thread::spawn(bootstrap::ensure_desktop_integration);

        // Load your existing engine configuration
        let wp_config = config::Config::load_or_default().unwrap_or_default();
        let available_fonts = load_fonts();
        let available_themes = load_themes();
        let selected_theme = available_themes
            .iter()
            .find(|t| **t == wp_config.audio.style)
            .cloned();

        let nav = nav_bar::Model::builder()
            .insert(|b| {
                b.text("Wallpaper")
                    .icon(icon::from_name("preferences-desktop-wallpaper-symbolic"))
                    .data(Page::Wallpaper)
                    .activate()
            })
            .insert(|b| {
                b.text("Live Wallpapers")
                    .icon(icon::from_name("video-display-symbolic"))
                    .data(Page::LiveWallpapers)
            })
            .insert(|b| {
                b.text("Layout Themes")
                    .icon(icon::from_name("applications-graphics-symbolic"))
                    .data(Page::Themes)
            })
            .insert(|b| {
                b.text("Now Playing")
                    .icon(icon::from_name("emblem-music-symbolic"))
                    .data(Page::NowPlaying)
            })
            .insert(|b| {
                b.text("Visualiser")
                    .icon(icon::from_name("audio-speakers-symbolic"))
                    .data(Page::Visualiser)
            })
            .insert(|b| {
                b.text("Weather")
                    .icon(icon::from_name("weather-clear-symbolic"))
                    .data(Page::Weather)
            })
            .insert(|b| {
                b.text("General")
                    .icon(icon::from_name("emblem-system-symbolic"))
                    .data(Page::General)
            })
            .build();

        let appearance_snapshot = wp_config.appearance.clone();
        let edit_theme = selected_theme
            .as_ref()
            .map(|name| config::ThemeLayout::load(name));

        let engine_pid = find_engine_pid();
        let engine_failure = if engine_pid.is_some() {
            None
        } else {
            engine_autostart_failure()
        };

        (
            SettingsApp {
                core,
                nav,
                available_fonts,
                available_themes,
                library: Vec::new(),
                drop_hover: false,
                selected_theme,
                autostart: autostart_enabled(),
                new_theme_name: String::new(),
                status_msg: "Ready.".into(),
                update_state: UpdateState::UpToDate,
                patch_notes: None,
                wallpaper_preview: None,
                color_picker: ColorPickerModel::new(
                    "Hex",
                    "RGB",
                    None,
                    wp_config
                        .appearance
                        .text_color
                        .map(|c| cosmic::iced::Color::from_rgb(c[0], c[1], c[2])),
                ),
                lat_input: format!("{}", wp_config.weather.latitude),
                lon_input: format!("{}", wp_config.weather.longitude),
                edit_theme,
                theme_element: 0,
                theme_save_generation: 0,
                engine_pid,
                engine_failure,
                // Not computed here: bootstrap::ensure_desktop_integration()
                // was just spawned on a background thread by `main` and
                // hasn't necessarily finished yet. Refreshed for real the
                // first time the user visits General, by which point it has.
                launcher_issue: None,
                wp_config,
                save_generation: 0,
            },
            Task::batch([
                Task::perform(check_for_updates(), |version| {
                    Message::UpdateCheckDone(version).into()
                }),
                scan_library_task(),
                load_wallpaper_preview_task(appearance_snapshot),
            ]),
        )
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        self.nav.activate(id);
        // Keep the engine row (and the Setup section) honest whenever
        // General comes into view.
        if self.nav.active_data::<Page>() == Some(&Page::General) {
            self.refresh_engine_status();
            self.launcher_issue = bootstrap::launcher_issue();
        }
        Task::none()
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::BackgroundModeSelected(mode) => {
                match mode {
                    BackgroundMode::FrostedGlass => {
                        self.wp_config.appearance.disable_blur = false;
                        self.wp_config.appearance.transparent_background = false;
                        self.wp_config.appearance.album_art_background = false;
                        self.wp_config.appearance.album_color_background = false;
                        self.wp_config.appearance.video_background_path = None;
                    }
                    BackgroundMode::Transparent => {
                        self.wp_config.appearance.disable_blur = true;
                        self.wp_config.appearance.transparent_background = true;
                        self.wp_config.appearance.album_art_background = false;
                        self.wp_config.appearance.album_color_background = false;
                        self.wp_config.appearance.video_background_path = None;
                    }
                    BackgroundMode::AlbumArt => {
                        // Album Art typically looks best with some blur fallback or as its own layer
                        self.wp_config.appearance.disable_blur = false;
                        self.wp_config.appearance.transparent_background = false;
                        self.wp_config.appearance.album_art_background = true;
                        self.wp_config.appearance.album_color_background = false;
                        self.wp_config.appearance.video_background_path = None;
                    }
                    BackgroundMode::AlbumPalette => {
                        self.wp_config.appearance.disable_blur = true;
                        self.wp_config.appearance.transparent_background = false;
                        self.wp_config.appearance.album_art_background = false;
                        self.wp_config.appearance.album_color_background = true;
                        // Clear the video path like every other non-Video arm:
                        // the view derives the current mode video-first, so a
                        // leftover path kept the UI (and engine) stuck on Video.
                        self.wp_config.appearance.video_background_path = None;
                    }
                    BackgroundMode::Video => {
                        self.wp_config.appearance.disable_blur = false;
                        self.wp_config.appearance.transparent_background = false;
                        self.wp_config.appearance.album_art_background = false;
                        self.wp_config.appearance.album_color_background = false;
                        // Keep an already-selected video; only default to the
                        // first one when none is set (re-picking "Video" in the
                        // dropdown must not reset the user's choice).
                        if self.wp_config.appearance.video_background_path.is_none() {
                            if let Some(first_video) = config::Config::available_videos().first() {
                                self.wp_config.appearance.video_background_path =
                                    Some(first_video.clone());
                            }
                        }
                    }
                }
                let _ = self.wp_config.save();
            }
            Message::FontSelected(idx) => {
                if let Some(family) = self.available_fonts.get(idx) {
                    self.wp_config.appearance.font_family =
                        (family != "System Default").then(|| family.clone());
                    let _ = self.wp_config.save();
                }
            }
            Message::ToggleShowAlbumArt(state) => {
                self.wp_config.appearance.show_album_art = state;
                let _ = self.wp_config.save();
            }
            Message::VideoSelected(idx) => {
                if let Some(entry) = self.library.get(idx) {
                    self.wp_config.appearance.video_background_path = Some(entry.file_name.clone());
                    let _ = self.wp_config.save();
                }
            }
            Message::ToggleWatchCanvas(state) => {
                self.wp_config.appearance.prefer_canvas = state;
                let _ = self.wp_config.save();
            }
            Message::LibraryLoaded(entries) => {
                self.library = entries;
            }
            Message::DndEntered => {
                self.drop_hover = true;
            }
            Message::DndLeft => {
                self.drop_hover = false;
            }
            Message::FilesDropped(files) => {
                self.drop_hover = false;
                let paths = files.map(|f| f.0).unwrap_or_default();
                if paths.is_empty() {
                    self.status_msg = "Nothing usable was dropped - MP4 or WebM files.".into();
                } else {
                    self.status_msg = format!(
                        "Importing {} file{}...",
                        paths.len(),
                        if paths.len() == 1 { "" } else { "s" }
                    );
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || library::import(paths))
                                .await
                                .unwrap_or((0, 0))
                        },
                        |(imported, skipped)| Message::ImportDone { imported, skipped }.into(),
                    );
                }
            }
            Message::WallpaperPreviewLoaded(preview) => {
                self.wallpaper_preview = preview.map(|boxed| *boxed);
            }
            Message::TextColorMode(idx) => {
                self.wp_config.appearance.text_color = if idx == 0 {
                    None
                } else {
                    let colour = self
                        .color_picker
                        .get_applied_color()
                        .unwrap_or(cosmic::iced::Color::WHITE);
                    Some([colour.r, colour.g, colour.b])
                };
                let _ = self.wp_config.save();
            }
            Message::TextColorPicker(update) => {
                if matches!(update, ColorPickerUpdate::AppliedColor) {
                    // The applied colour lands in the model below; save after.
                    let task = self.color_picker.update::<cosmic::Action<Message>>(update);
                    if let Some(colour) = self.color_picker.get_applied_color() {
                        self.wp_config.appearance.text_color = Some([colour.r, colour.g, colour.b]);
                        let _ = self.wp_config.save();
                    }
                    return task;
                }
                if matches!(update, ColorPickerUpdate::Reset) {
                    self.wp_config.appearance.text_color = None;
                    let _ = self.wp_config.save();
                }
                return self.color_picker.update::<cosmic::Action<Message>>(update);
            }
            Message::LatitudeChanged(input) => {
                self.lat_input = input;
                if let Ok(lat) = self.lat_input.trim().parse::<f64>() {
                    if (-90.0..=90.0).contains(&lat) {
                        self.wp_config.weather.latitude = lat;
                        return self.schedule_debounced_save();
                    }
                }
            }
            Message::LongitudeChanged(input) => {
                self.lon_input = input;
                if let Ok(lon) = self.lon_input.trim().parse::<f64>() {
                    if (-180.0..=180.0).contains(&lon) {
                        self.wp_config.weather.longitude = lon;
                        return self.schedule_debounced_save();
                    }
                }
            }
            Message::DetectLocation => {
                self.status_msg = "Detecting location...".into();
                return Task::perform(fetch_ip_location(), |result| {
                    Message::LocationDetected(result).into()
                });
            }
            Message::LocationDetected(Ok((lat, lon))) => {
                self.lat_input = format!("{lat}");
                self.lon_input = format!("{lon}");
                self.wp_config.weather.latitude = lat;
                self.wp_config.weather.longitude = lon;
                let _ = self.wp_config.save();
                self.status_msg = "Location detected.".into();
            }
            Message::LocationDetected(Err(e)) => {
                self.status_msg = format!("Could not detect location: {e}");
            }
            Message::PollIntervalSelected(idx) => {
                if let Some(&minutes) = view::POLL_MINUTES.get(idx) {
                    self.wp_config.weather.poll_interval_minutes = minutes;
                    let _ = self.wp_config.save();
                }
            }
            Message::ImportDone { imported, skipped } => {
                self.status_msg = match (imported, skipped) {
                    (0, _) => "No videos imported - only MP4, WebM, MKV, MOV and AVI files.".into(),
                    (n, 0) => format!("Imported {n} video{}.", if n == 1 { "" } else { "s" }),
                    (n, s) => format!("Imported {n}, skipped {s} (not video files)."),
                };
                return scan_library_task();
            }
            Message::ThemeSelected(idx) => {
                self.selected_theme = self.available_themes.get(idx).cloned();
                self.load_edit_theme();
            }
            Message::ThemeElementSelected(idx) => {
                self.theme_element = idx;
            }
            Message::ThemeEdit(edit) => {
                let element = self.theme_element;
                if let Some(layout) = &mut self.edit_theme {
                    apply_theme_edit(layout, element, edit);
                    return self.schedule_theme_save();
                }
            }
            Message::ResetThemeElement => {
                let element = self.theme_element;
                if let (Some(style), Some(layout)) = (&self.selected_theme, &mut self.edit_theme) {
                    reset_theme_element(layout, style, element);
                    return self.schedule_theme_save();
                }
            }
            Message::DebouncedThemeSave(generation) => {
                if generation == self.theme_save_generation {
                    self.write_theme_file();
                }
            }
            Message::ThemeFilesDropped(files) => {
                let paths = files.map(|f| f.0).unwrap_or_default();
                let mut imported = 0;
                for path in paths
                    .iter()
                    .filter(|p| p.extension().is_some_and(|e| e == "toml"))
                {
                    let Ok(text) = std::fs::read_to_string(path) else {
                        continue;
                    };
                    if let Err(e) = toml::from_str::<config::ThemeLayout>(&text) {
                        self.status_msg = format!("Not a valid theme: {e}");
                        continue;
                    }
                    let Some(name) = path.file_name() else {
                        continue;
                    };
                    let dir = config::Config::config_dir().join("shaders");
                    let _ = std::fs::create_dir_all(&dir);
                    if std::fs::write(dir.join(name), text).is_ok() {
                        imported += 1;
                    }
                }
                if imported > 0 {
                    self.available_themes = load_themes();
                    self.status_msg = format!(
                        "Imported {imported} theme{}.",
                        if imported == 1 { "" } else { "s" }
                    );
                } else if self.status_msg.starts_with("Ready") {
                    self.status_msg = "Nothing imported - drop .toml theme files.".into();
                }
            }
            Message::StartEngine => {
                if let Some(engine) = engine_binary_path() {
                    // stderr goes to a scratch file, not a pipe: a pipe's
                    // buffer would fill (and its reader vanish with this
                    // process) under a long-lived engine, while a file is
                    // safe to leave attached forever - and holds the dynamic
                    // linker's message when the binary can't load at all.
                    let stderr_log = std::env::temp_dir().join("cosmic-wallpaper-start.log");
                    let stderr = std::fs::File::create(&stderr_log)
                        .map(std::process::Stdio::from)
                        .unwrap_or_else(|_| std::process::Stdio::null());
                    match std::process::Command::new(engine).stderr(stderr).spawn() {
                        Ok(mut child) => {
                            self.status_msg = "Engine starting...".into();
                            return Task::perform(
                                async move {
                                    tokio::time::sleep(std::time::Duration::from_millis(1500))
                                        .await;
                                    tokio::task::spawn_blocking(move || {
                                        match child.try_wait() {
                                            // Died within the probe window:
                                            // report how, from the exit code
                                            // and captured stderr.
                                            Ok(Some(status)) => {
                                                let stderr = std::fs::read_to_string(&stderr_log)
                                                    .unwrap_or_default();
                                                Some((status.code(), stderr_headline(&stderr)))
                                            }
                                            _ => None,
                                        }
                                    })
                                    .await
                                    .unwrap_or(None)
                                },
                                |probe| Message::EngineStartProbed(probe).into(),
                            );
                        }
                        Err(e) => self.status_msg = format!("Failed to start the engine: {e}"),
                    }
                } else {
                    self.status_msg =
                        "Could not find the cosmic-wallpaper binary next to Settings.".into();
                }
            }
            Message::EngineStartProbed(probe) => {
                self.refresh_engine_status();
                match probe {
                    Some((code, headline)) => {
                        let code =
                            code.map_or_else(|| "killed by signal".into(), |c| format!("exit {c}"));
                        let detail = if headline.is_empty() {
                            format!("The engine exited immediately ({code}).")
                        } else {
                            format!("The engine exited immediately ({code}): {headline}")
                        };
                        self.status_msg = detail.clone();
                        self.engine_failure = Some(detail);
                    }
                    None => {
                        self.status_msg = match self.engine_pid {
                            Some(_) => "Engine running.".into(),
                            None => "The engine did not start - check the logs.".into(),
                        };
                    }
                }
            }
            Message::StopEngine => {
                if let Some(pid) = self.engine_pid {
                    // The tray's Quit item is the tested graceful-shutdown
                    // path; menu id 3 is Quit Engine.
                    if let Some(busctl) = resolve_binary("busctl") {
                        let _ = std::process::Command::new(busctl)
                            .args([
                                "--user",
                                "call",
                                &format!("org.kde.StatusNotifierItem-{pid}-1"),
                                "/MenuBar",
                                "com.canonical.dbusmenu",
                                "Event",
                                "isvu",
                                "3",
                                "clicked",
                                "v",
                                "s",
                                "",
                                "0",
                            ])
                            .output();
                        self.status_msg = "Engine stopping...".into();
                        return Task::perform(
                            tokio::time::sleep(std::time::Duration::from_millis(1500)),
                            |()| Message::RefreshEngineStatus.into(),
                        );
                    }
                }
            }
            Message::RefreshEngineStatus => {
                self.refresh_engine_status();
                // Resolve the transitional status set by Start/Stop.
                if self.status_msg.starts_with("Engine start") {
                    self.status_msg = match self.engine_pid {
                        Some(_) => "Engine running.".into(),
                        None => "The engine did not start - check the logs.".into(),
                    };
                } else if self.status_msg.starts_with("Engine stop") {
                    self.status_msg = match self.engine_pid {
                        Some(_) => "The engine is still running.".into(),
                        None => "Engine stopped.".into(),
                    };
                }
            }
            Message::ApplyTheme => {
                if let Some(theme) = &self.selected_theme {
                    self.wp_config.audio.style = theme.clone();
                    if let Err(e) = self.wp_config.save() {
                        self.status_msg = format!("Error applying theme: {}", e);
                    } else {
                        self.status_msg = format!("Applied theme: '{}'", theme);
                    }
                } else {
                    self.status_msg = "Select a theme to apply.".into();
                }
            }
            Message::FpsChanged(fps) => {
                self.wp_config.fps = fps as u32;
                return self.schedule_debounced_save();
            }
            Message::BlurOpacityChanged(opacity) => {
                self.wp_config.appearance.blur_opacity = opacity;
                return self.schedule_debounced_save();
            }
            Message::BandsChanged(bands) => {
                self.wp_config.audio.bands = bands as usize;
                return self.schedule_debounced_save();
            }
            Message::SmoothingChanged(smoothing) => {
                self.wp_config.audio.smoothing = smoothing;
                return self.schedule_debounced_save();
            }
            Message::TemperatureUnitSelected(unit) => {
                self.wp_config.weather.temperature_unit = if unit == "Fahrenheit" {
                    config::TemperatureUnit::Fahrenheit
                } else {
                    config::TemperatureUnit::Celsius
                };
                let _ = self.wp_config.save();
            }
            Message::DebouncedSave(generation) => {
                // A newer slider change re-armed the timer; let its own
                // DebouncedSave do the (single) write.
                if generation == self.save_generation {
                    let _ = self.wp_config.save(); // Hot-reloads the engine via its file watcher
                }
            }
            Message::ToggleShowLyrics(state) => {
                self.wp_config.audio.show_lyrics = state;
                let _ = self.wp_config.save();
            }
            Message::ToggleAutostart(state) => {
                self.autostart = state;
                set_autostart(state);
            }
            Message::ToggleWeatherEnabled(state) => {
                self.wp_config.weather.enabled = state;
                let _ = self.wp_config.save();
            }
            Message::ToggleHideWeatherEffects(state) => {
                self.wp_config.weather.hide_effects = state;
                let _ = self.wp_config.save();
            }
            Message::NewThemeNameChanged(name) => {
                self.new_theme_name = name;
            }
            Message::CreateTheme => {
                if !self.new_theme_name.is_empty() {
                    let name = self.new_theme_name.trim().trim_end_matches(".toml");
                    let file_name = format!("shaders/{}.toml", name);

                    if !is_safe_path(&file_name) {
                        self.status_msg = format!("Blocked unsafe theme name: {}", name);
                        return Task::none();
                    }

                    let path = config::Config::config_dir().join(&file_name);

                    let mut options = std::fs::OpenOptions::new();
                    options.write(true).create_new(true);

                    match options.open(&path) {
                        Ok(mut file) => {
                            use std::io::Write;
                            let _ = file.write_all(THEME_TEMPLATE.as_bytes());
                            self.available_themes = load_themes();
                            self.selected_theme = Some(name.to_string());
                            self.load_edit_theme();
                            self.status_msg = format!("Created {}", file_name);
                            self.new_theme_name.clear();
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                            self.status_msg = format!("Theme '{}' already exists!", name);
                        }
                        Err(e) => {
                            self.status_msg = format!("Error creating theme: {}", e);
                        }
                    }
                }
            }
            Message::ShowPatchNotes => {
                self.status_msg = "Fetching patch notes...".into();
                return Task::perform(fetch_patch_notes(), |notes| {
                    Message::PatchNotesLoaded(notes).into()
                });
            }
            Message::PatchNotesLoaded(notes) => {
                self.patch_notes = Some(cosmic::widget::markdown::parse(&notes).collect());
                self.status_msg = "Ready.".into();
            }
            Message::PatchNotesLinkClicked(url) => {
                if let Some(xdg_open) = resolve_binary("xdg-open") {
                    let _ = std::process::Command::new(xdg_open).arg(url).spawn();
                } else {
                    tracing::warn!("Failed to open link: xdg-open not found in trusted PATH");
                    self.status_msg = "Failed to open link: xdg-open not found".into();
                }
            }
            Message::ClosePatchNotes => {
                self.patch_notes = None;
            }
            Message::ReportIssue => {
                let body = build_issue_body();
                // Parsed via `url` rather than hand-formatted so the log
                // excerpt - which can contain '&', '#', newlines - is
                // correctly percent-encoded into the query string.
                let mut url =
                    url::Url::parse("https://github.com/Kenyon-J/cosmic-wpengine/issues/new")
                        .expect("static URL is always valid");
                url.query_pairs_mut().append_pair("body", &body);

                if let Some(xdg_open) = resolve_binary("xdg-open") {
                    let _ = std::process::Command::new(xdg_open)
                        .arg(url.as_str())
                        .spawn();
                } else {
                    tracing::warn!("Failed to open link: xdg-open not found in trusted PATH");
                    self.status_msg = "Failed to open link: xdg-open not found".into();
                }
            }
            Message::CopyDiagnostics => {
                let text = build_diagnostics_text(self);
                self.status_msg = "Diagnostics copied to clipboard.".into();
                return cosmic::iced::clipboard::write(text);
            }
            Message::UpdateCheckDone(version) => {
                self.update_state = match version {
                    Some(v) => UpdateState::Available(v),
                    None => UpdateState::UpToDate,
                };
            }
            Message::StartUpdate => {
                if let UpdateState::Available(tag) = self.update_state.clone() {
                    match get_http_client() {
                        Ok(client) => {
                            let client = client.clone();
                            self.update_state = UpdateState::Updating(tag.clone());
                            self.status_msg = format!("Downloading {tag}...");
                            return Task::perform(updater::perform_update(client, tag), |res| {
                                Message::UpdateFinished(res).into()
                            });
                        }
                        Err(e) => {
                            self.status_msg = format!("Update failed: {e}");
                        }
                    }
                }
            }
            Message::UpdateFinished(result) => match result {
                Ok(tag) => {
                    self.update_state = UpdateState::Installed(tag.clone());
                    self.status_msg = format!(
                        "Updated to {tag}! The wallpaper engine restarted automatically; restart Settings to use the new version too."
                    );
                }
                Err(e) => {
                    self.status_msg = format!("Update failed: {e}");
                    // Fall back to Available so the button offers a retry
                    // instead of getting stuck showing "Updating...".
                    self.update_state = match &self.update_state {
                        UpdateState::Updating(tag) => UpdateState::Available(tag.clone()),
                        other => other.clone(),
                    };
                }
            },
            Message::OpenUpdateLink => {
                if let Some(xdg_open) = resolve_binary("xdg-open") {
                    let _ = std::process::Command::new(xdg_open)
                        .arg("https://github.com/Kenyon-J/cosmic-wpengine/releases/latest")
                        .spawn();
                } else {
                    tracing::warn!("Failed to open link: xdg-open not found in trusted PATH");
                    self.status_msg = "Failed to open link: xdg-open not found".into();
                }
            }
            Message::OpenConfigFolder => {
                if let Some(xdg_open) = resolve_binary("xdg-open") {
                    let config_dir = config::Config::config_dir();
                    let _ = std::process::Command::new(xdg_open).arg(config_dir).spawn();
                } else {
                    tracing::warn!("Failed to open folder: xdg-open not found in trusted PATH");
                    self.status_msg = "Failed to open folder: xdg-open not found".into();
                }
            }
            Message::OpenVideosFolder => {
                let videos_dir = config::Config::config_dir().join("videos");
                let _ = std::fs::create_dir_all(&videos_dir);
                if let Some(xdg_open) = resolve_binary("xdg-open") {
                    let _ = std::process::Command::new(xdg_open).arg(videos_dir).spawn();
                } else {
                    tracing::warn!("Failed to open folder: xdg-open not found in trusted PATH");
                    self.status_msg = "Failed to open folder: xdg-open not found".into();
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> cosmic::Element<'_, Self::Message> {
        view::view_app(self)
    }
}
