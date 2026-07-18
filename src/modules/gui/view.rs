use cosmic::iced::widget::{slider, Column, Row};
use cosmic::iced::Length;
use cosmic::widget::{button, dropdown, settings, text, text_input};

use super::{BackgroundMode, Message, Page, SettingsApp, UpdateState};

/// Labels for the background-style dropdown, index-aligned with
/// [`BackgroundMode::ALL`].
const BG_MODE_LABELS: [&str; 5] = [
    "Frosted Glass",
    "Transparent",
    "Album Art",
    "Album Colour",
    "Live Wallpaper",
];

const TEXT_COLOR_MODES: [&str; 2] = ["Automatic", "Custom"];

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

/// Preview box inside each style card.
const CARD_W: f32 = 116.0;
const CARD_H: f32 = 65.0;

/// Card button style: quiet by default, accent border when selected, faint
/// overlay on hover/press.
fn card_class(selected: bool) -> cosmic::theme::Button {
    fn base(selected: bool, overlay_alpha: f32, theme: &cosmic::Theme) -> button::Style {
        let mut style = button::Style::new();
        style.border_radius = theme.cosmic().corner_radii.radius_s.into();
        style.border_width = 2.0;
        style.border_color = if selected {
            cosmic::iced::Color::from(theme.cosmic().accent_color())
        } else {
            cosmic::iced::Color::TRANSPARENT
        };
        if overlay_alpha > 0.0 {
            style.overlay = Some(cosmic::iced::Background::Color(
                cosmic::iced::Color::from_rgba(1.0, 1.0, 1.0, overlay_alpha),
            ));
        }
        style
    }
    cosmic::theme::Button::Custom {
        active: Box::new(move |_, theme| base(selected, 0.0, theme)),
        disabled: Box::new(move |theme| base(selected, 0.0, theme)),
        hovered: Box::new(move |_, theme| base(selected, 0.05, theme)),
        pressed: Box::new(move |_, theme| base(selected, 0.1, theme)),
    }
}

/// A fixed-size card preview box, centring `content`.
fn card_box(content: cosmic::Element<'_, Message>) -> cosmic::Element<'_, Message> {
    cosmic::widget::container(content)
        .width(Length::Fixed(CARD_W))
        .height(Length::Fixed(CARD_H))
        .align_x(cosmic::iced::alignment::Horizontal::Center)
        .align_y(cosmic::iced::alignment::Vertical::Center)
        .into()
}

fn card_image(handle: &cosmic::widget::image::Handle) -> cosmic::widget::Image<'_> {
    cosmic::widget::image(handle.clone())
        .content_fit(cosmic::iced::ContentFit::Contain)
        .width(Length::Fixed(CARD_W))
        .height(Length::Fixed(CARD_H))
}

/// The preview drawn inside one style card.
fn style_card_preview(app: &SettingsApp, mode: BackgroundMode) -> cosmic::Element<'_, Message> {
    let preview = app.wallpaper_preview.as_ref();
    match mode {
        BackgroundMode::FrostedGlass => match preview {
            Some(p) => card_image(&p.card_blurred).into(),
            None => card_box(text::title3("❄").into()),
        },
        BackgroundMode::Transparent => match preview {
            Some(p) => card_image(&p.card_sharp).opacity(0.25_f32).into(),
            None => card_box(text::caption("None").into()),
        },
        BackgroundMode::AlbumArt => card_box(
            cosmic::widget::icon::from_name("emblem-music-symbolic")
                .size(28)
                .into(),
        ),
        BackgroundMode::AlbumPalette => cosmic::widget::container(text::body(""))
            .width(Length::Fixed(CARD_W))
            .height(Length::Fixed(CARD_H))
            .class(cosmic::theme::Container::custom(|_| {
                cosmic::iced::widget::container::Style {
                    background: Some(cosmic::iced::Background::Gradient(
                        cosmic::iced::Gradient::Linear(
                            cosmic::iced::gradient::Linear::new(cosmic::iced::Radians(
                                std::f32::consts::FRAC_PI_2,
                            ))
                            .add_stop(0.0, cosmic::iced::Color::from_rgb(0.48, 0.20, 0.32))
                            .add_stop(0.5, cosmic::iced::Color::from_rgb(0.77, 0.42, 0.29))
                            .add_stop(1.0, cosmic::iced::Color::from_rgb(0.85, 0.63, 0.36)),
                        ),
                    )),
                    ..Default::default()
                }
            }))
            .into(),
        BackgroundMode::Video => match app.library.iter().find_map(|e| e.thumbnail.as_ref()) {
            Some(thumb) => cosmic::widget::image(cosmic::widget::image::Handle::from_path(thumb))
                .content_fit(cosmic::iced::ContentFit::Contain)
                .width(Length::Fixed(CARD_W))
                .height(Length::Fixed(CARD_H))
                .into(),
            None => card_box(text::title3("▶").into()),
        },
    }
}

/// The frosted-glass live preview: the real wallpaper, its blurred copy
/// mixed in at the configured amount, the glass tint, and sample text in
/// the colour the engine would pick.
fn frosted_preview(app: &SettingsApp) -> Option<cosmic::Element<'_, Message>> {
    use cosmic_wallpaper::modules::colour;
    let p = app.wallpaper_preview.as_ref()?;
    let opacity = app.wp_config.appearance.blur_opacity;
    let tint_alpha = opacity * 0.45;

    // Mirror the engine's adaptive choice against the tinted backdrop.
    let sample_color = match app.wp_config.appearance.text_color {
        Some(c) => cosmic::iced::Color::from_rgb(c[0], c[1], c[2]),
        None => {
            let dimmed = colour::lerp_colour(p.mean, [0.106, 0.106, 0.106], tint_alpha);
            if colour::relative_luminance(dimmed) > 0.179 {
                cosmic::iced::Color::from_rgb(0.1, 0.1, 0.1)
            } else {
                cosmic::iced::Color::from_rgb(0.95, 0.95, 0.95)
            }
        }
    };

    let tint = cosmic::widget::container(text::body(""))
        .width(Length::Fixed(480.0))
        .height(Length::Fixed(160.0))
        .class(cosmic::theme::Container::custom(move |_| {
            cosmic::iced::widget::container::Style {
                background: Some(cosmic::iced::Background::Color(
                    cosmic::iced::Color::from_rgba(0.106, 0.106, 0.106, tint_alpha),
                )),
                ..Default::default()
            }
        }));

    let sample = cosmic::widget::container(
        Column::new()
            .push(
                text::title4("On, and on, and on, and on")
                    .class(cosmic::theme::Text::Color(sample_color)),
            )
            .push(
                text::caption("I can feel the rush, I can feel the noise")
                    .class(cosmic::theme::Text::Color(sample_color)),
            )
            .spacing(4)
            .align_x(cosmic::iced::Alignment::Center),
    )
    .width(Length::Fixed(480.0))
    .height(Length::Fixed(160.0))
    .align_x(cosmic::iced::alignment::Horizontal::Center)
    .align_y(cosmic::iced::alignment::Vertical::Center);

    let stack = cosmic::iced::widget::Stack::with_children(vec![
        cosmic::widget::image(p.strip_sharp.clone())
            .content_fit(cosmic::iced::ContentFit::Contain)
            .width(Length::Fixed(480.0))
            .height(Length::Fixed(160.0))
            .into(),
        cosmic::widget::image(p.strip_blurred.clone())
            .opacity(opacity)
            .content_fit(cosmic::iced::ContentFit::Contain)
            .width(Length::Fixed(480.0))
            .height(Length::Fixed(160.0))
            .into(),
        tint.into(),
        sample.into(),
    ]);

    Some(
        cosmic::widget::container(stack)
            .width(Length::Fill)
            .align_x(cosmic::iced::alignment::Horizontal::Center)
            .into(),
    )
}

fn wallpaper(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let mode = app.current_background_mode();

    // Style cards: preview + label, selected card highlighted.
    let mut cards: Vec<cosmic::Element<'_, Message>> = Vec::new();
    for (idx, card_mode) in BackgroundMode::ALL.iter().enumerate() {
        let selected = *card_mode == mode;
        cards.push(
            button::custom(
                Column::new()
                    .push(style_card_preview(app, *card_mode))
                    .push(text::caption(BG_MODE_LABELS[idx]))
                    .spacing(4)
                    .align_x(cosmic::iced::Alignment::Center),
            )
            .class(card_class(selected))
            .padding(4)
            .on_press(Message::BackgroundModeSelected(*card_mode))
            .into(),
        );
    }
    let cards = cosmic::widget::flex_row(cards)
        .row_spacing(8)
        .column_spacing(8);

    let mut sections = vec![settings::section().title("Style").add(cards).into()];

    if mode == BackgroundMode::FrostedGlass {
        let mut frosted = settings::section().title("Frosted Glass");
        if let Some(preview) = frosted_preview(app) {
            frosted = frosted.add(preview);
        }
        frosted = frosted.add(
            settings::item::builder("Blur amount")
                .description("How strongly the wallpaper is blurred.")
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
        );
        sections.push(frosted.into());
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

    // Text colour applies to every style, so it lives outside the
    // conditional sections.
    let custom_active = app.wp_config.appearance.text_color.is_some();
    let mut colour_row = Row::new()
        .spacing(8)
        .align_y(cosmic::iced::Alignment::Center)
        .push(dropdown(
            &TEXT_COLOR_MODES[..],
            Some(usize::from(custom_active)),
            Message::TextColorMode,
        ));
    if custom_active {
        colour_row = colour_row.push(
            app.color_picker
                .picker_button(Message::TextColorPicker, None)
                .width(Length::Fixed(48.0)),
        );
    }
    sections.push(
        settings::section()
            .title("Text")
            .add(
                settings::item::builder("Text colour")
                    .description("Automatic picks a colour that stays readable on your wallpaper.")
                    .control(colour_row),
            )
            .into(),
    );

    if custom_active && app.color_picker.get_is_active() {
        sections.push(
            app.color_picker
                .builder(Message::TextColorPicker)
                .build("Recent colours", "Copy to clipboard", "Copied to clipboard")
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

/// Fixed thumbnail box per tile; 16:9, small enough for three tiles plus
/// spacing at the window's default width.
const TILE_THUMB_WIDTH: f32 = 168.0;
const TILE_THUMB_HEIGHT: f32 = 94.5;

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

            // Contain (not Cover): the image renderer at this rev does not
            // clip overflow, so the scaled image must never exceed its box.
            let thumb_inner: cosmic::Element<'_, Message> = match &entry.thumbnail {
                Some(path) => cosmic::widget::image(cosmic::widget::image::Handle::from_path(path))
                    .content_fit(cosmic::iced::ContentFit::Contain)
                    .width(Length::Fixed(TILE_THUMB_WIDTH))
                    .height(Length::Fixed(TILE_THUMB_HEIGHT))
                    .into(),
                None => text::title3("▶").into(),
            };
            let thumb = cosmic::widget::container(thumb_inner)
                .width(Length::Fill)
                .height(Length::Fixed(TILE_THUMB_HEIGHT))
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .align_y(cosmic::iced::alignment::Vertical::Center);

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
                    .description("Edit theme files by hand - changes apply while the engine runs.")
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
        "Bars that move with whatever is playing.",
        sections,
    )
}

// ----------------------------------------------------------------- Weather

const TEMPERATURE_UNITS: [&str; 2] = ["Celsius", "Fahrenheit"];
const POLL_LABELS: [&str; 4] = ["5 minutes", "15 minutes", "30 minutes", "1 hour"];
pub(crate) const POLL_MINUTES: [u64; 4] = [5, 15, 30, 60];

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
                .description("Turns off rain and snow animations to save power.")
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
        .add(
            settings::item::builder("Location")
                .description("Latitude and longitude for the forecast.")
                .control(
                    Row::new()
                        .push(
                            text_input("Latitude", &app.lat_input)
                                .on_input(Message::LatitudeChanged)
                                .width(Length::Fixed(100.0)),
                        )
                        .push(
                            text_input("Longitude", &app.lon_input)
                                .on_input(Message::LongitudeChanged)
                                .width(Length::Fixed(100.0)),
                        )
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center),
                ),
        )
        .add(
            settings::item::builder("Update every").control(dropdown(
                &POLL_LABELS[..],
                POLL_MINUTES
                    .iter()
                    .position(|m| *m == app.wp_config.weather.poll_interval_minutes),
                Message::PollIntervalSelected,
            )),
        )
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
