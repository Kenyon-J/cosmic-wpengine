#[cfg(test)]
mod tests;
mod updater;
mod view;
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use cosmic::app::Core;
use cosmic::iced::Task;
use cosmic::widget::{icon, nav_bar};
use cosmic::Application;
use cosmic_text::fontdb;

// Import the shared modules from your newly created library crate!
use cosmic_wallpaper::modules::config;
use cosmic_wallpaper::modules::utils::resolve_binary;

fn main() -> cosmic::iced::Result {
    // Launch the libcosmic application
    cosmic::app::run::<SettingsApp>(cosmic::app::Settings::default(), ())
}

struct SettingsApp {
    core: Core,
    nav: nav_bar::Model,
    wp_config: config::Config,
    available_fonts: Vec<String>,
    available_themes: Vec<String>,
    available_videos: Vec<String>,
    selected_theme: Option<String>,
    new_theme_name: String,
    status_msg: String,
    autostart: bool,
    update_state: UpdateState,
    /// Fetched release notes, shown on the General page when present.
    patch_notes: Option<String>,
    /// Monotonic counter pairing each slider change with its debounce timer;
    /// a DebouncedSave only writes to disk if its generation is still the
    /// newest (i.e. the slider settled for the full window).
    save_generation: u64,
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
}

fn autostart_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(home).join(".config/autostart/cosmic-wallpaper.desktop")
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
    if enable {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(
            &path,
            r#"[Desktop Entry]
Type=Application
Exec=cosmic-wallpaper
Hidden=false
X-GNOME-Autostart-enabled=true
Name=COSMIC Wallpaper"#,
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
    /// Index into `available_videos`.
    VideoSelected(usize),
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
    ReportIssue,
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
    const APP_ID: &'static str = "com.system76.CosmicWallpaperSettings";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
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

        (
            SettingsApp {
                core,
                nav,
                wp_config,
                available_fonts,
                available_themes,
                available_videos: config::Config::available_videos(),
                selected_theme,
                autostart: autostart_path().exists(),
                new_theme_name: String::new(),
                status_msg: "Ready.".into(),
                update_state: UpdateState::UpToDate,
                patch_notes: None,
                save_generation: 0,
            },
            Task::perform(check_for_updates(), |version| {
                Message::UpdateCheckDone(version).into()
            }),
        )
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        self.nav.activate(id);
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
                if let Some(video) = self.available_videos.get(idx) {
                    self.wp_config.appearance.video_background_path = Some(video.clone());
                    let _ = self.wp_config.save();
                }
            }
            Message::ThemeSelected(idx) => {
                self.selected_theme = self.available_themes.get(idx).cloned();
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
                            let default_content = r#"[visualiser]
shape = "linear"
position = [0.5, 0.5]
size = 0.85
rotation = 0.0
amplitude = 1.5"#;
                            use std::io::Write;
                            let _ = file.write_all(default_content.as_bytes());
                            self.available_themes = load_themes();
                            self.selected_theme = Some(name.to_string());
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
                self.patch_notes = Some(notes);
                self.status_msg = "Ready.".into();
            }
            Message::ClosePatchNotes => {
                self.patch_notes = None;
            }
            Message::ReportIssue => {
                if let Some(xdg_open) = resolve_binary("xdg-open") {
                    let _ = std::process::Command::new(xdg_open)
                        .arg("https://github.com/Kenyon-J/cosmic-wpengine/issues")
                        .spawn();
                } else {
                    tracing::warn!("Failed to open link: xdg-open not found in trusted PATH");
                    self.status_msg = "Failed to open link: xdg-open not found".into();
                }
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
