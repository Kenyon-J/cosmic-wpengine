mod bootstrap;
mod diagnostics;
mod library;
mod pack_export;
#[cfg(test)]
mod tests;
mod update_engine;
mod update_library;
mod update_overlays;
mod update_packs;
mod update_theme;
mod update_wallpaper;
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
use cosmic_wallpaper::fl;
use cosmic_wallpaper::modules::config::ResolvedBackground;
use diagnostics::{
    autostart_enabled, build_diagnostics_text, build_issue_body, engine_autostart_failure,
    engine_binary_path, find_engine_pid, set_autostart, stderr_headline,
};
use pack_export::{build_pack_bytes, write_pack_to_disk};
use updater::{check_for_updates, fetch_ip_location, fetch_patch_notes, get_http_client};

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
    /// Index into `view::theme_elements()` - which element's controls show.
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
    /// Which theme the Packs page's Export button bundles up.
    pack_export_theme: Option<String>,
    /// A dropped pack that bundles a custom shader, waiting on the user to
    /// review its source and either confirm or cancel via `fn dialog()`.
    /// Nothing from it is written to disk until confirmed.
    pending_pack_import: Option<PendingPackImport>,
    /// Every pack imported into this profile, for the Packs page's "Your
    /// Packs" gallery - refreshed after every import.
    installed_packs: Vec<library::InstalledPack>,
}

/// A parsed `.cwtheme` pack whose shader hasn't been reviewed yet - see
/// `Message::ConfirmPackImport`/`CancelPackImport`.
struct PendingPackImport {
    name: String,
    theme_toml: String,
    /// (file name, bytes)
    background: Option<(String, Vec<u8>)>,
    /// (file name, bytes)
    shader: (String, Vec<u8>),
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
    BarWidthRatio(f32),
    CapRadius(f32),
    Reflection(f32),
    GlowStrength(f32),
    /// Rounded to the nearest integer band-segment count.
    LedSegments(f32),
    PeakHold(bool),
}

/// One page of the settings window, keyed off the sidebar selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Wallpaper,
    LiveWallpapers,
    Themes,
    Packs,
    NowPlaying,
    Visualiser,
    Weather,
    General,
}

#[derive(Debug, Clone, PartialEq)]
enum UpdateState {
    /// A check is in flight - covers both the initial startup check and a
    /// manual recheck, so the button can disable/relabel itself either way.
    Checking,
    UpToDate,
    /// The check itself failed (network, rate limit, bad response) - not
    /// the same as `UpToDate`. Holds a short reason to show the user.
    CheckFailed(String),
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
            self.status_msg = fl!("status-blocked-unsafe-theme-name", name = name.as_str());
            return;
        }
        match toml::to_string_pretty(layout) {
            Ok(text) => {
                let path = config::Config::config_dir().join(&rel);
                match std::fs::write(&path, text) {
                    Ok(()) => {
                        self.status_msg = if self.wp_config.audio.style == *name {
                            fl!("status-saved-theme-live", name = name.as_str())
                        } else {
                            fl!("status-saved-theme-inactive", name = name.as_str())
                        };
                    }
                    Err(e) => {
                        self.status_msg = fl!("status-error-saving-theme", error = e.to_string())
                    }
                }
            }
            Err(e) => {
                self.status_msg = fl!("status-error-serialising-theme", error = e.to_string())
            }
        }
    }

    /// (Re)loads the selected theme's layout into the editor.
    fn load_edit_theme(&mut self) {
        self.edit_theme = self
            .selected_theme
            .as_ref()
            .map(|name| config::ThemeLayout::load(name));
    }

    /// Switches the theme editor to `name` (or clears it, for `None`),
    /// flushing any edit pending for the theme being left first.
    ///
    /// `schedule_theme_save`'s debounce fires 300ms after the last edit,
    /// against whatever `selected_theme`/`edit_theme` happen to be *at fire
    /// time* - not what they were when the edit was made. Switching themes
    /// (via the dropdown, a pack import, theme creation, or applying a
    /// saved pack) before that timer lands used to mean the timer fired
    /// against the newly-selected theme instead, writing *its* own
    /// unedited layout over its own file while silently discarding
    /// whatever was actually pending for the theme just left - no error,
    /// nothing in the status line, the edit just never reached disk.
    /// Flushing synchronously here closes that window, and bumping
    /// `theme_save_generation` invalidates any timer already in flight so
    /// it becomes a harmless no-op instead of firing again later.
    fn switch_edit_theme(&mut self, name: Option<String>) {
        if self.edit_theme.is_some() {
            self.write_theme_file();
        }
        self.theme_save_generation = self.theme_save_generation.wrapping_add(1);
        self.selected_theme = name;
        self.load_edit_theme();
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

    /// Bundles `name`'s layout - plus the currently-configured background
    /// video and/or custom visualiser shader, when set - into a `.cwtheme`
    /// file under `library::packs_dir()`, de-duplicating with a numeric
    /// suffix. Returns the written path.
    fn export_pack(&self, name: &str) -> anyhow::Result<std::path::PathBuf> {
        let bytes = build_pack_bytes(
            name,
            self.wp_config.appearance.video_background_path.as_deref(),
        )?;

        let dir = library::packs_dir();
        std::fs::create_dir_all(&dir)?;
        let mut path = dir.join(format!("{name}.cwtheme"));
        let mut n = 1;
        while path.exists() {
            path = dir.join(format!("{name}-{n}.cwtheme"));
            n += 1;
        }
        std::fs::write(&path, &bytes)?;
        Ok(path)
    }

    /// Writes an imported pack's theme (and, when present, its background
    /// video and/or shader) to disk, then selects it - the shared tail end
    /// of both the no-shader-review import path and `ConfirmPackImport`.
    /// Returns the name actually written under, which differs from `name`
    /// when a theme by that name already existed - see `write_pack_to_disk`.
    fn finalize_pack_import(
        &mut self,
        name: &str,
        theme_toml: &str,
        background: Option<(String, Vec<u8>)>,
        shader: Option<(String, Vec<u8>)>,
    ) -> Result<String, String> {
        let written_as = write_pack_to_disk(name, theme_toml, background, shader)?;
        self.switch_edit_theme(Some(written_as.clone()));
        Ok(written_as)
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
/// `view::theme_elements()`: art, track, lyrics, visualiser, weather,
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
        ThemeEditMsg::BarWidthRatio(v) => layout.visualiser.bar_width_ratio = v,
        ThemeEditMsg::CapRadius(v) => layout.visualiser.cap_radius = v,
        ThemeEditMsg::Reflection(v) => layout.visualiser.reflection = v,
        ThemeEditMsg::GlowStrength(v) => layout.visualiser.glow_strength = v,
        ThemeEditMsg::LedSegments(v) => layout.visualiser.led_segments = v.round() as u32,
        ThemeEditMsg::PeakHold(v) => layout.visualiser.peak_hold = v,
        ThemeEditMsg::Bounce(v) => layout.effects.lyric_bounce = v,
        ThemeEditMsg::Stiffness(v) => layout.effects.lyric_spring_stiffness = v,
        ThemeEditMsg::Damping(v) => layout.effects.lyric_spring_damping = v,
        ThemeEditMsg::BeatPulse(v) => layout.effects.beat_pulse = v,
    }
}

/// Restores the element at `element` (same indexing as `apply_theme_edit`
/// and `view::theme_elements()`) to what `style` actually ships with -
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
    /// Index into `view::theme_elements()`.
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
    /// Index into the temperature-unit dropdown - not the display string,
    /// which is localized and so can't double as a data value.
    TemperatureUnitSelected(usize),
    /// Index into the language dropdown: 0 is "System default" (`None`),
    /// the rest map to `modules::i18n::AVAILABLE_LANGUAGES`.
    LanguageSelected(usize),
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
    UpdateCheckDone(Result<Option<String>, String>),
    /// Manual recheck - the Version row's "Check for Updates" button. Reuses
    /// the same `check_for_updates()`/`UpdateCheckDone` path the startup
    /// check already uses.
    CheckForUpdates,
    StartUpdate,
    UpdateFinished(Result<String, String>),
    OpenUpdateLink,
    OpenConfigFolder,
    OpenVideosFolder,
    /// Index into `available_themes`, for the Packs page's export picker.
    PackExportThemeSelected(usize),
    ExportPack,
    OpenPacksFolder,
    /// `.cwtheme` files dropped onto the Packs page.
    PackFilesDropped(Option<library::DroppedFiles>),
    /// "Enable anyway" on the custom-shader review dialog.
    ConfirmPackImport,
    /// "Cancel" on the custom-shader review dialog - nothing was written.
    CancelPackImport,
    /// One click from the Packs page's gallery: makes an already-imported
    /// pack's theme (and its background video, when it bundled one) live.
    ApplyPack(String),
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
        // Before anything below calls `fl!` for the first time (the nav
        // bar, next) - see `set_language()`'s doc comment on why order
        // matters here.
        cosmic_wallpaper::modules::i18n::set_language(wp_config.language.as_deref());
        let available_fonts = load_fonts();
        let available_themes = load_themes();
        let selected_theme = available_themes
            .iter()
            .find(|t| **t == wp_config.audio.style)
            .cloned();

        let nav = nav_bar::Model::builder()
            .insert(|b| {
                b.text(fl!("wallpaper-page-title"))
                    .icon(icon::from_name("preferences-desktop-wallpaper-symbolic"))
                    .data(Page::Wallpaper)
                    .activate()
            })
            .insert(|b| {
                b.text(fl!("live-wallpapers-page-title"))
                    .icon(icon::from_name("video-display-symbolic"))
                    .data(Page::LiveWallpapers)
            })
            .insert(|b| {
                b.text(fl!("theme-page-title"))
                    .icon(icon::from_name("applications-graphics-symbolic"))
                    .data(Page::Themes)
            })
            .insert(|b| {
                b.text(fl!("packs-page-title"))
                    .icon(icon::from_name("package-x-generic-symbolic"))
                    .data(Page::Packs)
            })
            .insert(|b| {
                b.text(fl!("now-playing-page-title"))
                    .icon(icon::from_name("emblem-music-symbolic"))
                    .data(Page::NowPlaying)
            })
            .insert(|b| {
                b.text(fl!("visualiser-page-title"))
                    .icon(icon::from_name("audio-speakers-symbolic"))
                    .data(Page::Visualiser)
            })
            .insert(|b| {
                b.text(fl!("weather-page-title"))
                    .icon(icon::from_name("weather-clear-symbolic"))
                    .data(Page::Weather)
            })
            .insert(|b| {
                b.text(fl!("general-page-title"))
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
        let pack_export_theme = selected_theme.clone();

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
                status_msg: fl!("status-ready"),
                update_state: UpdateState::Checking,
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
                pack_export_theme,
                pending_pack_import: None,
                installed_packs: library::scan_installed_packs(),
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
            Message::BackgroundModeSelected(mode) => self.on_background_mode_selected(mode),
            Message::FontSelected(idx) => self.on_font_selected(idx),
            Message::ToggleShowAlbumArt(state) => self.on_toggle_show_album_art(state),
            Message::ThemeSelected(idx) => self.on_theme_selected(idx),
            Message::VideoSelected(idx) => self.on_video_selected(idx),
            Message::ToggleWatchCanvas(state) => self.on_toggle_watch_canvas(state),
            Message::LibraryLoaded(entries) => self.on_library_loaded(entries),
            Message::FilesDropped(files) => self.on_files_dropped(files),
            Message::DndEntered => self.on_dnd_entered(),
            Message::DndLeft => self.on_dnd_left(),
            Message::ImportDone { imported, skipped } => self.on_import_done(imported, skipped),
            Message::WallpaperPreviewLoaded(preview) => self.on_wallpaper_preview_loaded(preview),
            Message::TextColorMode(idx) => self.on_text_color_mode(idx),
            Message::TextColorPicker(update) => self.on_text_color_picker(update),
            Message::LatitudeChanged(input) => self.on_latitude_changed(input),
            Message::LongitudeChanged(input) => self.on_longitude_changed(input),
            Message::DetectLocation => self.on_detect_location(),
            Message::LocationDetected(result) => self.on_location_detected(result),
            Message::PollIntervalSelected(idx) => self.on_poll_interval_selected(idx),
            Message::ThemeElementSelected(idx) => self.on_theme_element_selected(idx),
            Message::ThemeEdit(edit) => self.on_theme_edit(edit),
            Message::ResetThemeElement => self.on_reset_theme_element(),
            Message::DebouncedThemeSave(generation) => self.on_debounced_theme_save(generation),
            Message::ThemeFilesDropped(files) => self.on_theme_files_dropped(files),
            Message::StartEngine => self.on_start_engine(),
            Message::EngineStartProbed(probe) => self.on_engine_start_probed(probe),
            Message::StopEngine => self.on_stop_engine(),
            Message::RefreshEngineStatus => self.on_refresh_engine_status(),
            Message::ApplyTheme => self.on_apply_theme(),
            Message::FpsChanged(fps) => self.on_fps_changed(fps),
            Message::BlurOpacityChanged(opacity) => self.on_blur_opacity_changed(opacity),
            Message::BandsChanged(bands) => self.on_bands_changed(bands),
            Message::SmoothingChanged(smoothing) => self.on_smoothing_changed(smoothing),
            Message::TemperatureUnitSelected(idx) => self.on_temperature_unit_selected(idx),
            Message::LanguageSelected(idx) => self.on_language_selected(idx),
            Message::DebouncedSave(generation) => self.on_debounced_save(generation),
            Message::ToggleShowLyrics(state) => self.on_toggle_show_lyrics(state),
            Message::ToggleAutostart(state) => self.on_toggle_autostart(state),
            Message::ToggleWeatherEnabled(state) => self.on_toggle_weather_enabled(state),
            Message::ToggleHideWeatherEffects(state) => self.on_toggle_hide_weather_effects(state),
            Message::NewThemeNameChanged(name) => self.on_new_theme_name_changed(name),
            Message::CreateTheme => self.on_create_theme(),
            Message::ShowPatchNotes => self.on_show_patch_notes(),
            Message::PatchNotesLoaded(notes) => self.on_patch_notes_loaded(notes),
            Message::PatchNotesLinkClicked(url) => self.on_patch_notes_link_clicked(url),
            Message::ClosePatchNotes => self.on_close_patch_notes(),
            Message::ReportIssue => self.on_report_issue(),
            Message::CopyDiagnostics => self.on_copy_diagnostics(),
            Message::UpdateCheckDone(result) => self.on_update_check_done(result),
            Message::CheckForUpdates => self.on_check_for_updates(),
            Message::StartUpdate => self.on_start_update(),
            Message::UpdateFinished(result) => self.on_update_finished(result),
            Message::OpenUpdateLink => self.on_open_update_link(),
            Message::OpenConfigFolder => self.on_open_config_folder(),
            Message::OpenVideosFolder => self.on_open_videos_folder(),
            Message::PackExportThemeSelected(idx) => self.on_pack_export_theme_selected(idx),
            Message::ExportPack => self.on_export_pack(),
            Message::OpenPacksFolder => self.on_open_packs_folder(),
            Message::PackFilesDropped(files) => self.on_pack_files_dropped(files),
            Message::ConfirmPackImport => self.on_confirm_pack_import(),
            Message::CancelPackImport => self.on_cancel_pack_import(),
            Message::ApplyPack(name) => self.on_apply_pack(name),
        }
    }

    fn view(&self) -> cosmic::Element<'_, Self::Message> {
        view::view_app(self)
    }

    /// A pending shader-bearing pack import blocks on this native modal: the
    /// bundled `.wgsl` is arbitrary GPU code from a stranger, so nothing
    /// from the pack is written to disk until the user has actually looked
    /// at the source and clicked through.
    fn dialog(&self) -> Option<cosmic::Element<'_, Self::Message>> {
        let pending = self.pending_pack_import.as_ref()?;
        let (shader_file, shader_bytes) = &pending.shader;
        let shader_source = String::from_utf8_lossy(shader_bytes).into_owned();
        let body = format!(
            "The pack '{}' includes a custom visualiser shader ({}). Review the source below - \
             it runs as GPU code with no sandboxing beyond this check.",
            pending.name, shader_file
        );
        Some(
            cosmic::widget::dialog()
                .title("This pack includes a custom shader")
                .body(body)
                .control(
                    cosmic::iced::widget::scrollable(
                        cosmic::widget::text::monotext(shader_source)
                            .size(11)
                            .width(cosmic::iced::Length::Fill),
                    )
                    .height(cosmic::iced::Length::Fixed(240.0)),
                )
                .secondary_action(
                    cosmic::widget::button::standard("Cancel").on_press(Message::CancelPackImport),
                )
                .primary_action(
                    cosmic::widget::button::destructive("Enable anyway")
                        .on_press(Message::ConfirmPackImport),
                )
                .into(),
        )
    }
}
