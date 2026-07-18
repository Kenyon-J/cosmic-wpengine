use cosmic::iced::widget::{pick_list, slider, Column, Row};
use cosmic::iced::Length;
use cosmic::widget::{button, settings, text, text_input};

use super::{BackgroundMode, Message, Page, SettingsApp, UpdateState};

pub(crate) fn view_app(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let page = app
        .nav
        .active_data::<Page>()
        .copied()
        .unwrap_or(Page::Wallpaper);

    let content = match page {
        Page::Wallpaper => wallpaper(app),
        Page::LiveWallpapers => live_wallpapers(app),
        Page::Themes => themes(app),
        Page::NowPlaying => now_playing(app),
        Page::Visualiser => visualiser(app),
        Page::Weather => weather(app),
        Page::General => general(app),
    };

    cosmic::iced::widget::scrollable(
        cosmic::iced::widget::container(content)
            .padding([24, 32])
            .width(Length::Fill),
    )
    .into()
}

/// Shared page scaffold: title, one-line summary, settings sections, and the
/// status line every action reports into.
fn page<'a>(
    app: &'a SettingsApp,
    title: &'a str,
    summary: &'a str,
    sections: Vec<cosmic::Element<'a, Message>>,
) -> cosmic::Element<'a, Message> {
    let mut children: Vec<cosmic::Element<'a, Message>> =
        vec![text::title3(title).into(), text::caption(summary).into()];
    children.extend(sections);
    children.push(text::caption(&app.status_msg).into());
    settings::view_column(children).into()
}

fn labeled_slider<'a>(
    value_label: String,
    control: cosmic::Element<'a, Message>,
) -> cosmic::Element<'a, Message> {
    Row::new()
        .push(text::body(value_label))
        .push(control)
        .spacing(12)
        .align_y(cosmic::iced::Alignment::Center)
        .into()
}

// ---------------------------------------------------------------- Wallpaper

fn wallpaper(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let mode = app.current_background_mode();

    let mut sections = vec![settings::section()
        .title("Style")
        .add(
            settings::item::builder("Background style")
                .description("What fills the desktop behind the overlays.")
                .control(pick_list(
                    &BackgroundMode::ALL[..],
                    Some(mode),
                    Message::BackgroundModeSelected,
                )),
        )
        .into()];

    if mode == BackgroundMode::FrostedGlass {
        sections.push(
            settings::section()
                .title("Frosted Glass")
                .add(
                    settings::item::builder("Blur amount")
                        .description("Same dual-Kawase strengths as COSMIC's frosted windows.")
                        .control(labeled_slider(
                            format!("{:.2}", app.wp_config.appearance.blur_opacity),
                            slider(
                                0.0..=1.0,
                                app.wp_config.appearance.blur_opacity,
                                Message::BlurOpacityChanged,
                            )
                            .step(0.05_f32)
                            .width(Length::Fixed(220.0))
                            .into(),
                        )),
                )
                .into(),
        );
    }

    if mode == BackgroundMode::Video {
        sections.push(
            settings::section()
                .title("Live Wallpaper")
                .add(settings::item(
                    "Video",
                    text::body("Pick and manage videos on the Live Wallpapers page"),
                ))
                .into(),
        );
    }

    page(
        app,
        "Wallpaper",
        "Options for the selected style appear below it.",
        sections,
    )
}

// --------------------------------------------------------- Live Wallpapers

fn live_wallpapers(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let video_picker = pick_list(
        app.available_videos.clone(),
        app.wp_config.appearance.video_background_path.clone(),
        Message::VideoSelected,
    )
    .placeholder(if app.available_videos.is_empty() {
        "No videos in your library yet"
    } else {
        "Select a video..."
    });

    let sections = vec![settings::section()
        .title("Library")
        .add(
            settings::item::builder("Active video")
                .description("Selecting a video also switches the wallpaper style to it.")
                .control(video_picker),
        )
        .add(
            settings::item::builder("Library folder")
                .description("MP4 and WebM files placed here appear in the list above.")
                .control(button::standard("Open Folder").on_press(Message::OpenVideosFolder)),
        )
        .into()];

    page(
        app,
        "Live Wallpapers",
        "Looping videos that play as your background.",
        sections,
    )
}

// ------------------------------------------------------------------ Themes

fn themes(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let theme_picker = pick_list(
        app.available_themes.clone(),
        app.selected_theme.clone(),
        Message::ThemeSelected,
    )
    .placeholder("Select a theme...");

    let sections = vec![
        settings::section()
            .title("Visualiser Themes")
            .add(
                settings::item::builder("Theme")
                    .description("Where the visualiser sits and how it is shaped.")
                    .control(
                        Row::new()
                            .push(theme_picker)
                            .push(button::suggested("Apply").on_press(Message::ApplyTheme))
                            .spacing(8)
                            .align_y(cosmic::iced::Alignment::Center),
                    ),
            )
            .into(),
        settings::section()
            .title("Manage")
            .add(
                settings::item::builder("Create new theme")
                    .description("Starts from a template layout you can edit.")
                    .control(
                        Row::new()
                            .push(
                                text_input("Theme name", &app.new_theme_name)
                                    .on_input(Message::NewThemeNameChanged)
                                    .on_submit(|_| Message::CreateTheme)
                                    .width(Length::Fixed(180.0)),
                            )
                            .push(button::standard("Create").on_press(Message::CreateTheme))
                            .spacing(8)
                            .align_y(cosmic::iced::Alignment::Center),
                    ),
            )
            .add(
                settings::item::builder("Theme files")
                    .description("Plain TOML in the shaders folder - edits apply live.")
                    .control(button::standard("Open Folder").on_press(Message::OpenConfigFolder)),
            )
            .into(),
    ];

    page(
        app,
        "Layout Themes",
        "How the audio visualiser is laid out on screen.",
        sections,
    )
}

// ------------------------------------------------------------- Now Playing

fn now_playing(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let current_font = app
        .wp_config
        .appearance
        .font_family
        .clone()
        .unwrap_or_else(|| "System Default".to_string());

    let sections = vec![
        settings::section()
            .title("Album Art")
            .add(
                settings::item::builder("Show album art")
                    .description("The current cover, placed by the active layout theme.")
                    .toggler(
                        app.wp_config.appearance.show_album_art,
                        Message::ToggleShowAlbumArt,
                    ),
            )
            .into(),
        settings::section()
            .title("Lyrics & Text")
            .add(
                settings::item::builder("Show lyrics")
                    .description("Synced lyrics for the current track, when available.")
                    .toggler(app.wp_config.audio.show_lyrics, Message::ToggleShowLyrics),
            )
            .add(settings::item::builder("Font family").control(pick_list(
                app.available_fonts.clone(),
                Some(current_font),
                Message::FontFamilySelected,
            )))
            .into(),
    ];

    page(
        app,
        "Now Playing",
        "What appears when music plays: album art, track info, and lyrics.",
        sections,
    )
}

// -------------------------------------------------------------- Visualiser

fn visualiser(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let sections = vec![settings::section()
        .title("Audio Response")
        .add(
            settings::item::builder("Bands")
                .description("How many bars the visualiser draws.")
                .control(labeled_slider(
                    format!("{}", app.wp_config.audio.bands),
                    slider(
                        16.0..=128.0,
                        app.wp_config.audio.bands as f32,
                        Message::BandsChanged,
                    )
                    .step(1.0_f32)
                    .width(Length::Fixed(220.0))
                    .into(),
                )),
        )
        .add(
            settings::item::builder("Smoothing")
                .description("Higher is calmer; lower is snappier.")
                .control(labeled_slider(
                    format!("{:.2}", app.wp_config.audio.smoothing),
                    slider(
                        0.0..=0.95,
                        app.wp_config.audio.smoothing,
                        Message::SmoothingChanged,
                    )
                    .step(0.05_f32)
                    .width(Length::Fixed(220.0))
                    .into(),
                )),
        )
        .into()];

    page(
        app,
        "Visualiser",
        "Audio-reactive bars, driven by whatever plays through PipeWire.",
        sections,
    )
}

// ----------------------------------------------------------------- Weather

const TEMPERATURE_UNITS: [&str; 2] = ["Celsius", "Fahrenheit"];

fn weather(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let current_unit = match app.wp_config.weather.temperature_unit {
        cosmic_wallpaper::modules::config::TemperatureUnit::Celsius => "Celsius",
        cosmic_wallpaper::modules::config::TemperatureUnit::Fahrenheit => "Fahrenheit",
    };

    let sections = vec![settings::section()
        .title("Weather")
        .add(
            settings::item::builder("Show weather")
                .description("Current conditions on the desktop.")
                .toggler(app.wp_config.weather.enabled, Message::ToggleWeatherEnabled),
        )
        .add(
            settings::item::builder("Hide animated effects")
                .description("Skips rain and snow particles to save GPU.")
                .toggler(
                    app.wp_config.weather.hide_effects,
                    Message::ToggleHideWeatherEffects,
                ),
        )
        .add(settings::item::builder("Units").control(pick_list(
            &TEMPERATURE_UNITS[..],
            Some(current_unit),
            |unit| Message::TemperatureUnitSelected(unit.to_string()),
        )))
        .into()];

    page(
        app,
        "Weather",
        "Conditions and effects layered over the wallpaper.",
        sections,
    )
}

// ----------------------------------------------------------------- General

fn general(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let update_control: cosmic::Element<'_, Message> = match &app.update_state {
        UpdateState::UpToDate => text::body("Up to date").into(),
        UpdateState::Available(tag) if super::updater::is_self_updatable() => {
            button::suggested(format!("Update to {tag}"))
                .on_press(Message::StartUpdate)
                .into()
        }
        // Installed via a system package - point at the release page instead.
        UpdateState::Available(tag) => button::standard(format!("{tag} release page"))
            .on_press(Message::OpenUpdateLink)
            .into(),
        UpdateState::Updating(tag) => text::body(format!("Updating to {tag}...")).into(),
        UpdateState::Installed(tag) => text::body(format!("{tag} installed - restart")).into(),
    };

    let mut sections = vec![
        settings::section()
            .title("Engine")
            .add(
                settings::item::builder("Start on login")
                    .description("Launches the wallpaper engine when you log in.")
                    .toggler(app.autostart, Message::ToggleAutostart),
            )
            .add(
                settings::item::builder("Frame rate limit")
                    .description("Lower saves power; the engine idles when nothing animates.")
                    .control(labeled_slider(
                        format!("{} fps", app.wp_config.fps),
                        slider(15.0..=144.0, app.wp_config.fps as f32, Message::FpsChanged)
                            .step(1.0_f32)
                            .width(Length::Fixed(220.0))
                            .into(),
                    )),
            )
            .add(
                settings::item::builder("Config folder")
                    .description("All engine configuration lives here.")
                    .control(button::standard("Open Folder").on_press(Message::OpenConfigFolder)),
            )
            .into(),
        settings::section()
            .title("About")
            .add(
                settings::item::builder("Version")
                    .description(env!("CARGO_PKG_VERSION"))
                    .control(update_control),
            )
            .add(
                settings::item::builder("Patch notes")
                    .description("What changed in the latest release.")
                    .control(if app.patch_notes.is_some() {
                        button::standard("Hide").on_press(Message::ClosePatchNotes)
                    } else {
                        button::standard("Show").on_press(Message::ShowPatchNotes)
                    }),
            )
            .add(
                settings::item::builder("Something broken?")
                    .control(button::standard("Report an Issue").on_press(Message::ReportIssue)),
            )
            .into(),
    ];

    if let Some(notes) = &app.patch_notes {
        sections.push(
            settings::section()
                .title("Patch Notes")
                .add(
                    Column::new()
                        .push(text::body(notes.as_str()))
                        .width(Length::Fill)
                        .padding(8),
                )
                .into(),
        );
    }

    page(
        app,
        "General",
        "Engine behaviour and housekeeping.",
        sections,
    )
}
