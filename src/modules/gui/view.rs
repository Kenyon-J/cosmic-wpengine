use cosmic::iced::widget::{slider, Column, Row};
use cosmic::iced::Length;
use cosmic::widget::{button, dropdown, settings, text, text_input};

use super::{BackgroundMode, Message, Page, SettingsApp, UpdateState};

/// Labels for the background-style dropdown, index-aligned with
/// [`BackgroundMode::ALL`].
const BG_MODE_LABELS: [&str; 5] = [
    "Frosted Glass (Blur)",
    "Fully Transparent",
    "Album Art Background",
    "Album Colour",
    "Video Background",
];

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
                .control(dropdown(
                    &BG_MODE_LABELS[..],
                    BackgroundMode::ALL.iter().position(|m| *m == mode),
                    |idx| Message::BackgroundModeSelected(BackgroundMode::ALL[idx]),
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
    // Drop zone: accepts text/uri-list drags from the file manager.
    let drop_label = if app.drop_hover {
        "Release to add to your library"
    } else {
        "Drop video files here to add them (MP4, WebM)"
    };
    let drop_zone: cosmic::Element<'_, Message> =
        cosmic::widget::dnd_destination::dnd_destination_for_data(
            cosmic::widget::container(text::body(drop_label))
                .width(Length::Fill)
                .padding(28)
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .class(if app.drop_hover {
                    cosmic::theme::Container::Primary
                } else {
                    cosmic::theme::Container::Secondary
                }),
            |data: Option<super::library::DroppedFiles>, _action| Message::FilesDropped(data),
        )
        .on_enter(|_, _, _| Message::DndEntered)
        .on_leave(|| Message::DndLeft)
        .into();

    // Tile grid, three across.
    let active_video = app.wp_config.appearance.video_background_path.as_deref();
    let mut grid = Column::new().spacing(12).width(Length::Fill);
    for (row_idx, chunk) in app.library.chunks(3).enumerate() {
        let mut row = Row::new().spacing(12).width(Length::Fill);
        for (col_idx, entry) in chunk.iter().enumerate() {
            let idx = row_idx * 3 + col_idx;
            let is_active = active_video == Some(entry.file_name.as_str());

            let thumb: cosmic::Element<'_, Message> = match &entry.thumbnail {
                Some(path) => cosmic::widget::image(cosmic::widget::image::Handle::from_path(path))
                    .content_fit(cosmic::iced::ContentFit::Cover)
                    .width(Length::Fill)
                    .height(Length::Fixed(96.0))
                    .into(),
                None => cosmic::widget::container(text::title3("▶"))
                    .width(Length::Fill)
                    .height(Length::Fixed(96.0))
                    .align_x(cosmic::iced::alignment::Horizontal::Center)
                    .align_y(cosmic::iced::alignment::Vertical::Center)
                    .into(),
            };

            let mut meta = Row::new()
                .spacing(6)
                .push(text::caption(entry.file_name.as_str()).width(Length::Fill));
            if is_active {
                meta = meta.push(text::caption("Active"));
            }
            if let Some(duration) = &entry.duration {
                meta = meta.push(text::caption(duration.as_str()));
            }

            let tile = button::custom(
                Column::new()
                    .push(thumb)
                    .push(meta)
                    .spacing(6)
                    .width(Length::Fill),
            )
            .class(if is_active {
                cosmic::theme::Button::Suggested
            } else {
                cosmic::theme::Button::Image
            })
            .padding(6)
            .width(Length::Fill)
            .on_press(Message::VideoSelected(idx));

            row = row.push(tile);
        }
        // Pad the last row so tiles keep equal widths.
        for _ in chunk.len()..3 {
            row = row.push(
                cosmic::widget::container(text::body(""))
                    .width(Length::Fill)
                    .height(Length::Fixed(0.0)),
            );
        }
        grid = grid.push(row);
    }

    let mut library_section = settings::section().title("Library").add(drop_zone);
    if app.library.is_empty() {
        library_section = library_section.add(settings::item(
            "No videos yet",
            text::body("Drop files above, or use Open Folder to add them by hand"),
        ));
    } else {
        library_section = library_section.add(grid);
    }

    let sections = vec![
        library_section.into(),
        settings::section()
            .title("Playback")
            .add(
                settings::item::builder("Prefer Spotify Canvas")
                    .description(
                        "When the playing track has a Canvas loop, show it instead of your wallpaper.",
                    )
                    .toggler(
                        app.wp_config.appearance.prefer_canvas,
                        Message::ToggleWatchCanvas,
                    ),
            )
            .add(
                settings::item::builder("Library folder")
                    .description("Videos live in ~/.config/cosmic-wallpaper/videos.")
                    .control(button::standard("Open Folder").on_press(Message::OpenVideosFolder)),
            )
            .into(),
    ];

    page(
        app,
        "Live Wallpapers",
        "Looping videos that play as your background. Click a tile to set it.",
        sections,
    )
}

// ------------------------------------------------------------------ Themes

fn themes(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let selected_theme = app
        .selected_theme
        .as_ref()
        .and_then(|t| app.available_themes.iter().position(|name| name == t));
    let theme_picker = dropdown(
        &app.available_themes[..],
        selected_theme,
        Message::ThemeSelected,
    );

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
    // "System Default" sits at index 0, so a missing font_family maps there.
    let current_font = app
        .wp_config
        .appearance
        .font_family
        .as_ref()
        .and_then(|f| app.available_fonts.iter().position(|name| name == f))
        .unwrap_or(0);

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
            .add(settings::item::builder("Font family").control(dropdown(
                &app.available_fonts[..],
                Some(current_font),
                Message::FontSelected,
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
        cosmic_wallpaper::modules::config::TemperatureUnit::Celsius => 0,
        cosmic_wallpaper::modules::config::TemperatureUnit::Fahrenheit => 1,
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
        .add(settings::item::builder("Units").control(dropdown(
            &TEMPERATURE_UNITS[..],
            Some(current_unit),
            |idx| Message::TemperatureUnitSelected(TEMPERATURE_UNITS[idx].to_string()),
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
