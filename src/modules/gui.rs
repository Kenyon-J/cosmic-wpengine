#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use cosmic::app::Core;
use cosmic::iced::widget::{checkbox, pick_list, slider};
use cosmic::iced::Length;
use cosmic::iced::Task;
use cosmic::widget::{column, row, text, text_editor, text_input};
use cosmic::{Application, Element};
use cosmic_text::fontdb;

// Import the shared modules from your newly created library crate!
use cosmic_wallpaper::modules::config;

fn main() -> cosmic::iced::Result {
    // Launch the libcosmic application
    cosmic::app::run::<SettingsApp>(cosmic::app::Settings::default(), ())
}

struct SettingsApp {
    core: Core,
    wp_config: config::Config,
    available_fonts: Vec<String>,
    available_files: Vec<String>,
    available_videos: Vec<String>,
    selected_file: Option<String>,
    editor_content: text_editor::Content,
    new_theme_name: String,
    status_msg: String,
    autostart: bool,
    update_available: Option<String>,
}

impl SettingsApp {
    fn refresh_editor(&mut self) {
        if self.selected_file.as_deref() == Some("config.toml") {
            let path = config::Config::config_dir().join("config.toml");
            let content_str = std::fs::read_to_string(path).unwrap_or_default();
            self.editor_content = text_editor::Content::with_text(&content_str);
        }
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
        let _ = std::process::Command::new("busctl")
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

fn is_safe_path(path_str: &str) -> bool {
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

fn load_files() -> Vec<String> {
    let mut files = vec!["config.toml".to_string()];
    let shaders_dir = config::Config::config_dir().join("shaders");
    if let Ok(entries) = std::fs::read_dir(shaders_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".toml") {
                    files.push(format!("shaders/{}", name));
                }
            }
        }
    }
    files.sort();
    files
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

async fn fetch_patch_notes() -> String {
    let url = "https://api.github.com/repos/Kenyon-J/cosmic-wpengine/releases/latest";
    let client = reqwest::Client::builder()
        .user_agent("cosmic-wallpaper/1.0")
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => return format!("Failed to build HTTP client: {}", e),
    };

    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<GitHubRelease>().await {
            Ok(release) => format!(
                "COSMIC Wallpaper Engine {}\n\n{}",
                release.tag_name, release.body
            ),
            Err(e) => format!("Failed to parse patch notes from GitHub: {}", e),
        },
        Ok(resp) => format!("Failed to fetch patch notes: HTTP {}", resp.status()),
        Err(e) => format!("Failed to fetch patch notes: {}", e),
    }
}

async fn check_for_updates() -> Option<String> {
    let url = "https://api.github.com/repos/Kenyon-J/cosmic-wpengine/releases/latest";
    let client = reqwest::Client::builder()
        .user_agent("cosmic-wallpaper/1.0")
        .build()
        .ok()?;

    let release: GitHubRelease = client.get(url).send().await.ok()?.json().await.ok()?;
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
    FontFamilySelected(String),
    ToggleShowAlbumArt(bool),
    FileSelected(String),
    VideoSelected(String),
    EditorAction(text_editor::Action),
    SaveFile,
    ApplyTheme,
    FpsChanged(f32),
    BlurOpacityChanged(f32),
    ToggleShowLyrics(bool),
    ToggleAutostart(bool),
    ToggleWeatherEnabled(bool),
    ToggleHideWeatherEffects(bool),
    NewThemeNameChanged(String),
    CreateTheme,
    ShowPatchNotes,
    PatchNotesLoaded(String),
    ReportIssue,
    UpdateCheckDone(Option<String>),
    OpenUpdateLink,
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
        let available_files = load_files();
        let selected_file = Some("config.toml".to_string());

        let path = config::Config::config_dir().join("config.toml");
        let content_str = std::fs::read_to_string(path).unwrap_or_default();
        let editor_content = text_editor::Content::with_text(&content_str);

        (
            SettingsApp {
                core,
                wp_config,
                available_fonts,
                available_files,
                available_videos: config::Config::available_videos(),
                selected_file,
                editor_content,
                autostart: autostart_path().exists(),
                new_theme_name: String::new(),
                status_msg: "Ready.".into(),
                update_available: None,
            },
            Task::perform(check_for_updates(), |version| {
                Message::UpdateCheckDone(version).into()
            }),
        )
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
                    }
                    BackgroundMode::Video => {
                        self.wp_config.appearance.disable_blur = false;
                        self.wp_config.appearance.transparent_background = false;
                        self.wp_config.appearance.album_art_background = false;
                        self.wp_config.appearance.album_color_background = false;
                        self.wp_config.appearance.video_background_path = None;
                        if self.wp_config.appearance.video_background_path.is_none() {
                            if let Some(first_video) = config::Config::available_videos().first() {
                                self.wp_config.appearance.video_background_path =
                                    Some(first_video.clone());
                            }
                        }
                    }
                }
                let _ = self.wp_config.save();
                self.refresh_editor();
            }
            Message::FontFamilySelected(family) => {
                if family == "System Default" {
                    self.wp_config.appearance.font_family = None;
                } else {
                    self.wp_config.appearance.font_family = Some(family);
                }
                let _ = self.wp_config.save();
                self.refresh_editor();
            }
            Message::ToggleShowAlbumArt(state) => {
                self.wp_config.appearance.show_album_art = state;
                let _ = self.wp_config.save();
                self.refresh_editor();
            }
            Message::VideoSelected(video) => {
                self.wp_config.appearance.video_background_path = Some(video);
                let _ = self.wp_config.save();
                self.refresh_editor();
            }
            Message::FileSelected(file) => {
                if is_safe_path(&file) {
                    self.selected_file = Some(file.clone());
                    let path = config::Config::config_dir().join(&file);
                    let content_str = std::fs::read_to_string(path).unwrap_or_default();
                    self.editor_content = text_editor::Content::with_text(&content_str);
                    self.status_msg = format!("Loaded {}", file);
                } else {
                    self.status_msg = format!("Blocked unsafe file path: {}", file);
                }
            }
            Message::EditorAction(action) => {
                self.editor_content.perform(action);
            }
            Message::SaveFile => {
                if let Some(file) = &self.selected_file {
                    if !is_safe_path(file) {
                        self.status_msg = format!("Blocked unsafe save path: {}", file);
                        return Task::none();
                    }
                    let path = config::Config::config_dir().join(file);
                    let text = self.editor_content.text();
                    match std::fs::write(&path, text) {
                        Ok(_) => {
                            self.status_msg = format!("Saved {}", file);
                            // If we edited the base config, ensure our GUI state stays in sync
                            if file == "config.toml" {
                                if let Ok(new_cfg) = config::Config::load_or_default() {
                                    self.wp_config = new_cfg;
                                }
                            }
                        }
                        Err(e) => self.status_msg = format!("Error saving: {}", e),
                    }
                }
            }
            Message::ApplyTheme => {
                if let Some(file) = &self.selected_file {
                    if file.starts_with("shaders/") && file.ends_with(".toml") {
                        let theme_name = file
                            .trim_start_matches("shaders/")
                            .trim_end_matches(".toml");
                        self.wp_config.audio.style = theme_name.to_string();
                        if let Err(e) = self.wp_config.save() {
                            self.status_msg = format!("Error applying theme: {}", e);
                        } else {
                            self.status_msg = format!("Applied theme: '{}'", theme_name);
                        }
                    } else {
                        self.status_msg =
                            "Please select a theme (.toml in shaders/) to apply.".into();
                    }
                }
            }
            Message::FpsChanged(fps) => {
                self.wp_config.fps = fps as u32;
                let _ = self.wp_config.save(); // Instantly hot-reloads the engine!
                self.refresh_editor();
            }
            Message::BlurOpacityChanged(opacity) => {
                self.wp_config.appearance.blur_opacity = opacity;
                let _ = self.wp_config.save();
                self.refresh_editor();
            }
            Message::ToggleShowLyrics(state) => {
                self.wp_config.audio.show_lyrics = state;
                let _ = self.wp_config.save();
                self.refresh_editor();
            }
            Message::ToggleAutostart(state) => {
                self.autostart = state;
                set_autostart(state);
            }
            Message::ToggleWeatherEnabled(state) => {
                self.wp_config.weather.enabled = state;
                let _ = self.wp_config.save();
                self.refresh_editor();
            }
            Message::ToggleHideWeatherEffects(state) => {
                self.wp_config.weather.hide_effects = state;
                let _ = self.wp_config.save();
                self.refresh_editor();
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
                            self.available_files = load_files();
                            self.selected_file = Some(file_name.clone());
                            let content_str = std::fs::read_to_string(path).unwrap_or_default();
                            self.editor_content = text_editor::Content::with_text(&content_str);
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
                self.selected_file = None;
                self.editor_content =
                    text_editor::Content::with_text("Fetching latest patch notes from GitHub...");
                self.status_msg = "Fetching patch notes...".into();
                return Task::perform(fetch_patch_notes(), |notes| {
                    Message::PatchNotesLoaded(notes).into()
                });
            }
            Message::PatchNotesLoaded(notes) => {
                self.editor_content = text_editor::Content::with_text(&notes);
                self.status_msg = "Viewing Patch Notes. Select a file to return to editing.".into();
            }
            Message::ReportIssue => {
                let _ = std::process::Command::new("xdg-open")
                    .arg("https://github.com/Kenyon-J/cosmic-wpengine/issues")
                    .spawn();
            }
            Message::UpdateCheckDone(version) => {
                self.update_available = version;
            }
            Message::OpenUpdateLink => {
                let _ = std::process::Command::new("xdg-open")
                    .arg("https://github.com/Kenyon-J/cosmic-wpengine/releases/latest")
                    .spawn();
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let font = cosmic::iced::Font::DEFAULT;

        let current_bg_mode = if self.wp_config.appearance.video_background_path.is_some() {
            BackgroundMode::Video
        } else if self.wp_config.appearance.album_color_background {
            BackgroundMode::AlbumPalette
        } else if self.wp_config.appearance.album_art_background {
            BackgroundMode::AlbumArt
        } else if self.wp_config.appearance.transparent_background {
            BackgroundMode::Transparent
        } else {
            BackgroundMode::FrostedGlass
        };

        let bg_mode_selector = cosmic::iced::widget::tooltip(
            pick_list(
                &BackgroundMode::ALL[..],
                Some(current_bg_mode),
                Message::BackgroundModeSelected,
            ),
            "Changes the desktop background effect.",
            cosmic::iced::widget::tooltip::Position::Top,
        );

        let mut toggles_row = column().push(
            row()
                .push(
                    text("Background Style:")
                        .font(font)
                        .width(Length::Fixed(200.0)),
                )
                .push(bg_mode_selector)
                .spacing(20),
        );

        if current_bg_mode == BackgroundMode::Video {
            let video_selector = cosmic::iced::widget::tooltip(
                pick_list(
                    self.available_videos.clone(),
                    self.wp_config.appearance.video_background_path.clone(),
                    Message::VideoSelected,
                ),
                "Select a video file to play as the background.",
                cosmic::iced::widget::tooltip::Position::Top,
            );
            toggles_row = toggles_row.push(
                row()
                    .push(
                        text("Selected Video:")
                            .font(font)
                            .width(Length::Fixed(200.0)),
                    )
                    .push(video_selector)
                    .spacing(20),
            );
        }

        let toggles_row = toggles_row
            .push(
                row()
                    .push(cosmic::iced::widget::tooltip(
                        checkbox(self.wp_config.appearance.show_album_art)
                            .on_toggle(Message::ToggleShowAlbumArt)
                            .label("Show Album Art Foreground")
                            .font(font),
                        "Displays the current album cover over the background.",
                        cosmic::iced::widget::tooltip::Position::Top,
                    ))
                    .push(cosmic::iced::widget::tooltip(
                        checkbox(self.wp_config.audio.show_lyrics)
                            .on_toggle(Message::ToggleShowLyrics)
                            .label("Show Lyrics")
                            .font(font),
                        "Displays scrolling lyrics for the current track.",
                        cosmic::iced::widget::tooltip::Position::Top,
                    ))
                    .push(cosmic::iced::widget::tooltip(
                        checkbox(self.autostart)
                            .on_toggle(Message::ToggleAutostart)
                            .label("Autostart on Login")
                            .font(font),
                        "Launches the wallpaper engine automatically when you log in.",
                        cosmic::iced::widget::tooltip::Position::Top,
                    ))
                    .spacing(20),
            )
            .push(
                row()
                    .push(cosmic::iced::widget::tooltip(
                        checkbox(self.wp_config.weather.enabled)
                            .on_toggle(Message::ToggleWeatherEnabled)
                            .label("Enable Weather")
                            .font(font),
                        "Displays current weather information on the desktop.",
                        cosmic::iced::widget::tooltip::Position::Top,
                    ))
                    .push(cosmic::iced::widget::tooltip(
                        checkbox(self.wp_config.weather.hide_effects)
                            .on_toggle(Message::ToggleHideWeatherEffects)
                            .label("Hide Weather Effects")
                            .font(font),
                        "Disables rain and snow animations to save performance.",
                        cosmic::iced::widget::tooltip::Position::Top,
                    ))
                    .spacing(20),
            )
            .spacing(15);

        let font_row = row()
            .push(text("Font Family:").font(font).width(Length::Fixed(200.0)))
            .push(cosmic::iced::widget::tooltip(
                pick_list(
                    self.available_fonts.clone(),
                    self.wp_config
                        .appearance
                        .font_family
                        .clone()
                        .or_else(|| Some("System Default".to_string())),
                    Message::FontFamilySelected,
                ),
                "Select the font used for displaying the clock, weather, and lyrics.",
                cosmic::iced::widget::tooltip::Position::Top,
            ))
            .spacing(20);

        let framerate_row = row()
            .push(
                text(format!("Target Framerate: {} FPS", self.wp_config.fps))
                    .font(font)
                    .width(Length::Fixed(200.0)),
            )
            .push(cosmic::iced::widget::tooltip(
                slider(15.0..=144.0, self.wp_config.fps as f32, Message::FpsChanged),
                "Higher framerates are smoother but use more system resources.",
                cosmic::iced::widget::tooltip::Position::Top,
            ))
            .spacing(20);

        let blur_row = row()
            .push(
                text(format!(
                    "Blur Strength: {:.2}",
                    self.wp_config.appearance.blur_opacity
                ))
                .font(font)
                .width(Length::Fixed(200.0)),
            )
            .push(
                cosmic::iced::widget::tooltip(
                    slider(
                        0.0..=1.0,
                        self.wp_config.appearance.blur_opacity,
                        Message::BlurOpacityChanged,
                    )
                    .step(0.05),
                    "Controls the strength of the background blur (only applies to Frosted Glass mode).",
                    cosmic::iced::widget::tooltip::Position::Top,
                )
            )
            .spacing(20);

        let file_selector = cosmic::iced::widget::tooltip(
            pick_list(
                self.available_files.clone(),
                self.selected_file.clone(),
                Message::FileSelected,
            ),
            "Select a configuration or shader theme file to edit.",
            cosmic::iced::widget::tooltip::Position::Top,
        );

        let save_btn: Element<'_, Self::Message> = {
            let btn = cosmic::iced::widget::button(text("Save File").font(font));
            if self.selected_file.is_some() {
                cosmic::iced::widget::tooltip(
                    btn.on_press(Message::SaveFile),
                    "Save changes to the current file.",
                    cosmic::iced::widget::tooltip::Position::Top,
                )
                .into()
            } else {
                cosmic::iced::widget::tooltip(
                    btn,
                    "Select a file to enable saving.",
                    cosmic::iced::widget::tooltip::Position::Top,
                )
                .into()
            }
        };

        let apply_btn: Element<'_, Self::Message> = {
            let selected_theme = self.selected_file.as_ref().and_then(|f| {
                if f.starts_with("shaders/") && f.ends_with(".toml") {
                    Some(
                        f.trim_start_matches("shaders/")
                            .trim_end_matches(".toml")
                            .to_string(),
                    )
                } else {
                    None
                }
            });

            if let Some(theme_name) = selected_theme {
                if theme_name == self.wp_config.audio.style {
                    let btn = cosmic::iced::widget::button(text("Theme Active").font(font));
                    cosmic::iced::widget::tooltip(
                        btn,
                        "This theme is currently active.",
                        cosmic::iced::widget::tooltip::Position::Top,
                    )
                    .into()
                } else {
                    let btn = cosmic::iced::widget::button(text("Apply Theme").font(font));
                    cosmic::iced::widget::tooltip(
                        btn.on_press(Message::ApplyTheme),
                        "Apply this theme to the wallpaper engine.",
                        cosmic::iced::widget::tooltip::Position::Top,
                    )
                    .into()
                }
            } else {
                let btn = cosmic::iced::widget::button(text("Apply Theme").font(font));
                cosmic::iced::widget::tooltip(
                    btn,
                    "Select a theme (.toml in shaders/) to apply it.",
                    cosmic::iced::widget::tooltip::Position::Top,
                )
                .into()
            }
        };

        let new_theme_input = cosmic::iced::widget::tooltip(
            text_input("New Theme Name...", &self.new_theme_name)
                .on_input(Message::NewThemeNameChanged)
                .on_submit(|_| Message::CreateTheme),
            "Enter a name to create a new copy of the current theme.",
            cosmic::iced::widget::tooltip::Position::Top,
        );

        let create_btn: Element<'_, Self::Message> = {
            let btn = cosmic::iced::widget::button(text("Create Theme").font(font));
            if !self.new_theme_name.trim().is_empty() {
                cosmic::iced::widget::tooltip(
                    btn.on_press(Message::CreateTheme),
                    "Create a new theme with this name.",
                    cosmic::iced::widget::tooltip::Position::Top,
                )
                .into()
            } else {
                cosmic::iced::widget::tooltip(
                    btn,
                    "Enter a name for the new theme first.",
                    cosmic::iced::widget::tooltip::Position::Top,
                )
                .into()
            }
        };

        let toolbar = row()
            .push(text("Edit File:").font(font).width(Length::Shrink))
            .push(file_selector)
            .push(save_btn)
            .push(apply_btn)
            .push(text(" | ").font(font))
            .push(new_theme_input)
            .push(create_btn)
            .spacing(10);

        let editor = text_editor(&self.editor_content)
            .font(cosmic::iced::Font::MONOSPACE)
            .on_action(Message::EditorAction)
            .height(Length::Fill);

        let report_btn = cosmic::iced::widget::tooltip(
            cosmic::iced::widget::button(text("Report Issue").font(font).size(14))
                .on_press(Message::ReportIssue),
            "Open GitHub to report a bug or request a feature.",
            cosmic::iced::widget::tooltip::Position::Top,
        );
        let notes_btn = cosmic::iced::widget::tooltip(
            cosmic::iced::widget::button(text("Patch Notes").font(font).size(14))
                .on_press(Message::ShowPatchNotes),
            "View recent changes and updates to the engine.",
            cosmic::iced::widget::tooltip::Position::Top,
        );

        let version_display: Element<'_, Self::Message> =
            if let Some(new_v) = &self.update_available {
                let update_btn = cosmic::iced::widget::button(
                    text(format!("Update Available: {}", new_v))
                        .font(font)
                        .size(14),
                )
                .on_press(Message::OpenUpdateLink);

                cosmic::iced::widget::tooltip(
                    update_btn,
                    "Open the release page to download the update.",
                    cosmic::iced::widget::tooltip::Position::Top,
                )
                .into()
            } else {
                text(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .font(font)
                    .size(14)
                    .into()
            };

        let footer_row = row()
            .push(
                text(&self.status_msg)
                    .font(font)
                    .size(14)
                    .width(Length::Fill),
            )
            .push(version_display)
            .push(notes_btn)
            .push(report_btn)
            .spacing(15);

        column()
            .push(text("COSMIC Wallpaper Settings").font(font).size(32))
            .push(toggles_row)
            .push(font_row)
            .push(framerate_row)
            .push(blur_row)
            .push(toolbar)
            .push(editor)
            .push(footer_row)
            .padding(40)
            .spacing(20)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_safe_path() {
        // Valid paths
        assert!(is_safe_path("config.toml"));
        assert!(is_safe_path("shaders/theme.toml"));
        assert!(is_safe_path("shaders/nested/theme.toml"));

        // Path traversal
        assert!(!is_safe_path("../test.txt"));
        assert!(!is_safe_path("shaders/../../etc/passwd"));
        assert!(!is_safe_path(".."));

        // Absolute paths
        assert!(!is_safe_path("/etc/passwd"));
        #[cfg(windows)]
        assert!(!is_safe_path("C:\\Windows\\System32\\config\\SAM"));
    }
}
