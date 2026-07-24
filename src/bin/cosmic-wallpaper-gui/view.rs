use cosmic::iced::widget::{slider, Column, Row};
use cosmic::iced::Length;
use cosmic::widget::{button, dropdown, settings, text, text_input};
use cosmic_wallpaper::fl;

use super::{BackgroundMode, Message, Page, SettingsApp, UpdateState};

/// Labels for the background-style dropdown, index-aligned with
/// [`BackgroundMode::ALL`].
fn bg_mode_labels() -> Vec<String> {
    vec![
        fl!("wallpaper-mode-frosted-glass"),
        fl!("wallpaper-mode-transparent"),
        fl!("wallpaper-mode-album-art"),
        fl!("wallpaper-mode-album-colour"),
        fl!("wallpaper-mode-live-wallpaper"),
    ]
}

fn text_color_mode_labels() -> Vec<String> {
    vec![
        fl!("text-color-mode-automatic"),
        fl!("text-color-mode-custom"),
    ]
}

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
        Page::Packs => packs(app),
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
    title: impl Into<String>,
    summary: impl Into<String>,
    sections: Vec<cosmic::Element<'a, Message>>,
) -> cosmic::Element<'a, Message> {
    let mut children: Vec<cosmic::Element<'a, Message>> = vec![
        text::title3(title.into()).into(),
        text::caption(summary.into()).into(),
    ];
    children.extend(sections);
    children.push(text::caption(&app.status_msg).into());
    settings::view_column(children).into()
}

/// A slider paired with a spin button: drag for coarse changes, click the
/// steppers for one-step fine tuning. Both drive the same message.
fn stepped_slider<'a>(
    name: impl Into<String>,
    value_label: String,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    on_change: impl Fn(f32) -> Message + Clone + 'static,
) -> cosmic::Element<'a, Message> {
    let (min, max) = (*range.start(), *range.end());
    Row::new()
        .push(cosmic::widget::spin_button(
            value_label,
            name.into(),
            value,
            step,
            min,
            max,
            on_change.clone(),
        ))
        .push(
            slider(range, value, on_change)
                .step(step)
                .width(Length::Fixed(180.0)),
        )
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
            None => card_box(text::caption(fl!("wallpaper-preview-none")).into()),
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
                text::title4(fl!("wallpaper-preview-sample-title"))
                    .class(cosmic::theme::Text::Color(sample_color)),
            )
            .push(
                text::caption(fl!("wallpaper-preview-sample-caption"))
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
    let bg_mode_labels = bg_mode_labels();
    let mut cards: Vec<cosmic::Element<'_, Message>> = Vec::new();
    for (idx, card_mode) in BackgroundMode::ALL.iter().enumerate() {
        let selected = *card_mode == mode;
        cards.push(
            button::custom(
                Column::new()
                    .push(style_card_preview(app, *card_mode))
                    .push(text::caption(bg_mode_labels[idx].clone()))
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

    let mut sections = vec![settings::section()
        .title(fl!("wallpaper-style-title"))
        .add(cards)
        .into()];

    if mode == BackgroundMode::FrostedGlass {
        let mut frosted = settings::section().title(fl!("wallpaper-frosted-glass-title"));
        if let Some(preview) = frosted_preview(app) {
            frosted = frosted.add(preview);
        }
        frosted = frosted.add(
            settings::item::builder(fl!("wallpaper-blur-amount"))
                .description(fl!("wallpaper-blur-amount-desc"))
                .control(stepped_slider(
                    fl!("wallpaper-blur-amount"),
                    format!("{:.2}", app.wp_config.appearance.blur_opacity),
                    app.wp_config.appearance.blur_opacity,
                    0.0..=1.0,
                    0.05,
                    Message::BlurOpacityChanged,
                )),
        );
        sections.push(frosted.into());
    }

    if mode == BackgroundMode::Video {
        sections.push(
            settings::section()
                .title(fl!("wallpaper-live-wallpaper-title"))
                .add(settings::item(
                    fl!("wallpaper-video-item"),
                    text::body(fl!("wallpaper-video-item-desc")),
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
            text_color_mode_labels(),
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
            .title(fl!("wallpaper-text-title"))
            .add(
                settings::item::builder(fl!("wallpaper-text-colour"))
                    .description(fl!("wallpaper-text-colour-desc"))
                    .control(colour_row),
            )
            .into(),
    );

    if custom_active && app.color_picker.get_is_active() {
        sections.push(
            app.color_picker
                .builder(Message::TextColorPicker)
                .build(
                    fl!("common-recent-colours"),
                    fl!("common-copy-to-clipboard"),
                    fl!("common-copied-to-clipboard"),
                )
                .into(),
        );
    }

    page(
        app,
        fl!("wallpaper-page-title"),
        fl!("wallpaper-page-summary"),
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
        fl!("live-wallpapers-drop-release")
    } else {
        fl!("live-wallpapers-drop-prompt")
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
                meta = meta.push(text::caption(fl!("common-active")));
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

    let mut library_section = settings::section()
        .title(fl!("live-wallpapers-library-title"))
        .add(drop_zone);
    if app.library.is_empty() {
        library_section = library_section.add(settings::item(
            fl!("live-wallpapers-no-videos"),
            text::body(fl!("live-wallpapers-no-videos-desc")),
        ));
    } else {
        library_section = library_section.add(grid);
    }

    let sections = vec![
        library_section.into(),
        settings::section()
            .title(fl!("live-wallpapers-playback-title"))
            .add(
                settings::item::builder(fl!("live-wallpapers-prefer-canvas"))
                    .description(fl!("live-wallpapers-prefer-canvas-desc"))
                    .toggler(
                        app.wp_config.appearance.prefer_canvas,
                        Message::ToggleWatchCanvas,
                    ),
            )
            .add(
                settings::item::builder(fl!("live-wallpapers-library-folder"))
                    .description(fl!("live-wallpapers-library-folder-desc"))
                    .control(
                        button::standard(fl!("common-open-folder"))
                            .on_press(Message::OpenVideosFolder),
                    ),
            )
            .into(),
    ];

    page(
        app,
        fl!("live-wallpapers-page-title"),
        fl!("live-wallpapers-page-summary"),
        sections,
    )
}

// ------------------------------------------------------------------ Themes

/// Element tab labels, index-aligned with `app.theme_element`.
pub(crate) fn theme_elements() -> Vec<String> {
    vec![
        fl!("theme-element-album-art"),
        fl!("theme-element-track-info"),
        fl!("theme-element-lyrics"),
        fl!("theme-element-visualiser"),
        fl!("theme-element-weather"),
        fl!("theme-element-effects"),
    ]
}
fn text_align_labels() -> Vec<String> {
    vec![
        fl!("common-align-left"),
        fl!("common-align-center"),
        fl!("common-align-right"),
    ]
}
fn art_shape_labels() -> Vec<String> {
    vec![fl!("common-shape-square"), fl!("common-shape-circular")]
}
fn vis_shape_labels() -> Vec<String> {
    vec![
        fl!("common-shape-linear"),
        fl!("common-shape-circular"),
        fl!("common-shape-square"),
    ]
}

/// One editor slider row: label, live value, TOML key caption. `key` is the
/// literal TOML field name (e.g. `position[0]`), shown verbatim as a
/// reference for anyone cross-checking against docs/THEMES.md - not
/// translated, like the docs themselves.
fn theme_slider<'a>(
    label: impl Into<String>,
    key: &'a str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    msg: fn(f32) -> super::ThemeEditMsg,
) -> cosmic::Element<'a, Message> {
    let label = label.into();
    settings::item::builder(label.clone())
        .description(key)
        .control(stepped_slider(
            label,
            format!("{value:.2}"),
            value,
            range,
            step,
            move |v| Message::ThemeEdit(msg(v)),
        ))
        .into()
}

fn theme_editor_rows<'a>(
    app: &'a SettingsApp,
    layout: &'a cosmic_wallpaper::modules::config::ThemeLayout,
) -> cosmic::Element<'a, Message> {
    use super::ThemeEditMsg as E;
    use cosmic_wallpaper::modules::config::{ArtShape, TextAlign, VisAlign, VisShape};

    let mut section = settings::section().title(theme_elements()[app.theme_element].clone());

    let align_row = |align: usize| {
        settings::item::builder(fl!("theme-align"))
            .description("align")
            .control(dropdown(text_align_labels(), Some(align), |idx| {
                Message::ThemeEdit(E::Align(idx))
            }))
    };
    let text_align_idx = |a: TextAlign| match a {
        TextAlign::Left => 0,
        TextAlign::Center => 1,
        TextAlign::Right => 2,
    };

    match app.theme_element {
        // Album Art
        0 => {
            let art = &layout.album_art;
            if layout.visualiser.shape == VisShape::Circular && layout.visualiser.dock_art {
                section = section.add(settings::item(
                    fl!("theme-docked"),
                    text::body(fl!("theme-docked-desc")),
                ));
            }
            section = section
                .add(theme_slider(
                    fl!("theme-position-x"),
                    "position[0]",
                    art.position[0],
                    0.0..=1.0,
                    0.01,
                    E::PosX,
                ))
                .add(theme_slider(
                    fl!("theme-position-y"),
                    "position[1]",
                    art.position[1],
                    0.0..=1.0,
                    0.01,
                    E::PosY,
                ))
                .add(theme_slider(
                    fl!("theme-size"),
                    "size",
                    art.size,
                    0.05..=1.0,
                    0.01,
                    E::Size,
                ))
                .add(
                    settings::item::builder(fl!("theme-shape"))
                        .description("shape")
                        .control(dropdown(
                            art_shape_labels(),
                            Some(match art.shape {
                                ArtShape::Square => 0,
                                ArtShape::Circular => 1,
                            }),
                            |idx| Message::ThemeEdit(E::Shape(idx)),
                        )),
                );
        }
        // Track Info / Lyrics / Weather
        1 | 2 | 4 => {
            let t = match app.theme_element {
                1 => &layout.track_info,
                2 => &layout.lyrics,
                _ => &layout.weather,
            };
            section = section
                .add(theme_slider(
                    fl!("theme-position-x"),
                    "position[0]",
                    t.position[0],
                    0.0..=1.0,
                    0.01,
                    E::PosX,
                ))
                .add(theme_slider(
                    fl!("theme-position-y"),
                    "position[1]",
                    t.position[1],
                    0.0..=1.0,
                    0.01,
                    E::PosY,
                ))
                .add(theme_slider(
                    fl!("theme-text-size"),
                    "size",
                    t.size,
                    0.5..=2.5,
                    0.05,
                    E::Size,
                ))
                .add(align_row(text_align_idx(t.align)));
        }
        // Visualiser
        3 => {
            let v = &layout.visualiser;
            section = section
                .add(
                    settings::item::builder(fl!("theme-shape"))
                        .description("shape")
                        .control(dropdown(
                            vis_shape_labels(),
                            Some(match v.shape {
                                VisShape::Linear => 0,
                                VisShape::Circular => 1,
                                VisShape::Square => 2,
                            }),
                            |idx| Message::ThemeEdit(E::Shape(idx)),
                        )),
                )
                .add(theme_slider(
                    fl!("theme-position-x"),
                    "position[0]",
                    v.position[0],
                    0.0..=1.0,
                    0.01,
                    E::PosX,
                ))
                .add(theme_slider(
                    fl!("theme-position-y"),
                    "position[1]",
                    v.position[1],
                    0.0..=1.0,
                    0.01,
                    E::PosY,
                ))
                .add(theme_slider(
                    fl!("theme-size"),
                    "size",
                    v.size,
                    0.05..=1.5,
                    0.01,
                    E::Size,
                ))
                .add(theme_slider(
                    fl!("theme-rotation"),
                    "rotation (degrees)",
                    v.rotation,
                    -180.0..=180.0,
                    1.0,
                    E::Rotation,
                ))
                .add(theme_slider(
                    fl!("theme-amplitude"),
                    "amplitude",
                    v.amplitude,
                    0.2..=3.0,
                    0.05,
                    E::Amplitude,
                ))
                .add(
                    settings::item::builder(fl!("theme-band-order"))
                        .description("align")
                        .control(dropdown(
                            text_align_labels(),
                            Some(match v.align {
                                VisAlign::Left => 0,
                                VisAlign::Center => 1,
                                VisAlign::Right => 2,
                            }),
                            |idx| Message::ThemeEdit(E::Align(idx)),
                        )),
                )
                .add(theme_slider(
                    fl!("theme-bar-width"),
                    "bar_width_ratio",
                    v.bar_width_ratio,
                    0.05..=1.0,
                    0.01,
                    E::BarWidthRatio,
                ))
                .add(theme_slider(
                    fl!("theme-cap-roundness"),
                    "cap_radius",
                    v.cap_radius,
                    0.0..=1.0,
                    0.01,
                    E::CapRadius,
                ))
                .add(theme_slider(
                    fl!("theme-glow"),
                    "glow_strength",
                    v.glow_strength,
                    0.0..=1.0,
                    0.01,
                    E::GlowStrength,
                ))
                .add(theme_slider(
                    fl!("theme-reflection"),
                    "reflection",
                    v.reflection,
                    0.0..=1.0,
                    0.01,
                    E::Reflection,
                ))
                .add(theme_slider(
                    fl!("theme-led-segments"),
                    "led_segments (0 = off)",
                    v.led_segments as f32,
                    0.0..=32.0,
                    1.0,
                    E::LedSegments,
                ))
                .add(
                    settings::item::builder(fl!("theme-peak-hold"))
                        .description(fl!("theme-peak-hold-desc"))
                        .toggler(v.peak_hold, |on| Message::ThemeEdit(E::PeakHold(on))),
                );
            if matches!(v.shape, VisShape::Circular) {
                section = section.add(
                    settings::item::builder(fl!("theme-dock-art"))
                        .description(fl!("theme-dock-art-desc"))
                        .toggler(v.dock_art, |on| Message::ThemeEdit(E::DockArt(on))),
                );
            }
        }
        // Effects
        _ => {
            let fx = &layout.effects;
            section = section
                .add(theme_slider(
                    fl!("theme-lyric-bounce"),
                    "lyric_bounce",
                    fx.lyric_bounce,
                    0.0..=3.0,
                    0.05,
                    E::Bounce,
                ))
                .add(theme_slider(
                    fl!("theme-spring-stiffness"),
                    "lyric_spring_stiffness",
                    fx.lyric_spring_stiffness,
                    30.0..=400.0,
                    5.0,
                    E::Stiffness,
                ))
                .add(theme_slider(
                    fl!("theme-spring-damping"),
                    "lyric_spring_damping",
                    fx.lyric_spring_damping,
                    2.0..=40.0,
                    1.0,
                    E::Damping,
                ))
                .add(theme_slider(
                    fl!("theme-beat-pulse"),
                    "beat_pulse",
                    fx.beat_pulse,
                    0.0..=3.0,
                    0.05,
                    E::BeatPulse,
                ));
        }
    }

    section = section.add(
        settings::item::builder(fl!("theme-reset-section"))
            .description(fl!(
                "theme-reset-section-desc",
                element = theme_elements()[app.theme_element].clone()
            ))
            .control(button::destructive(fl!("common-reset")).on_press(Message::ResetThemeElement)),
    );

    section.into()
}

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

    let is_active = app.selected_theme.as_deref() == Some(app.wp_config.audio.style.as_str());

    let mut sections = vec![settings::section()
        .title(fl!("theme-page-theme-title"))
        .add(
            settings::item::builder(fl!("theme-editing"))
                .description(if is_active {
                    fl!("theme-editing-live-desc")
                } else {
                    fl!("theme-editing-inactive-desc")
                })
                .control(
                    Row::new()
                        .push(theme_picker)
                        .push(if is_active {
                            button::standard(fl!("common-active"))
                        } else {
                            button::suggested(fl!("common-apply")).on_press(Message::ApplyTheme)
                        })
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center),
                ),
        )
        .into()];

    if let Some(layout) = &app.edit_theme {
        // Element tabs.
        let mut tabs = Row::new().spacing(6);
        for (idx, label) in theme_elements().into_iter().enumerate() {
            let selected = idx == app.theme_element;
            tabs = tabs.push(
                button::custom(text::body(label))
                    .class(card_class(selected))
                    .padding([4, 10])
                    .on_press(Message::ThemeElementSelected(idx)),
            );
        }
        sections.push(tabs.into());
        sections.push(theme_editor_rows(app, layout));
    }

    sections.push(
        settings::section()
            .title(fl!("theme-manage-title"))
            .add(
                settings::item::builder(fl!("theme-create-new"))
                    .description(fl!("theme-create-new-desc"))
                    .control({
                        let name = app.new_theme_name.trim();
                        let is_empty = name.is_empty();
                        let already_exists = app.available_themes.iter().any(|t| t == name);
                        let is_valid = !is_empty && !already_exists;

                        let mut input =
                            text_input(fl!("theme-name-placeholder"), &app.new_theme_name)
                                .on_input(Message::NewThemeNameChanged)
                                .width(Length::Fixed(180.0));
                        let mut btn = button::standard(fl!("common-create"));

                        if is_valid {
                            input = input.on_submit(|_| Message::CreateTheme);
                            btn = btn.on_press(Message::CreateTheme);
                        }

                        let btn_element: cosmic::Element<'_, Message> = if is_valid {
                            btn.into()
                        } else {
                            let error_msg = if is_empty {
                                fl!("theme-name-empty-error")
                            } else {
                                fl!("theme-name-exists-error")
                            };
                            cosmic::widget::tooltip(
                                btn,
                                text::body(error_msg),
                                cosmic::widget::tooltip::Position::Top,
                            )
                            .into()
                        };

                        Row::new()
                            .push(input)
                            .push(btn_element)
                            .spacing(8)
                            .align_y(cosmic::iced::Alignment::Center)
                    }),
            )
            .add(
                settings::item::builder(fl!("theme-import"))
                    .description(fl!("theme-import-desc"))
                    .control(
                        button::standard(fl!("common-open-folder"))
                            .on_press(Message::OpenConfigFolder),
                    ),
            )
            .into(),
    );

    let content = page(
        app,
        fl!("theme-page-title"),
        fl!("theme-page-summary"),
        sections,
    );

    // The whole page accepts theme-file drops.
    cosmic::widget::dnd_destination::dnd_destination_for_data(
        content,
        |data: Option<super::library::DroppedFiles>, _action| Message::ThemeFilesDropped(data),
    )
    .into()
}

// ------------------------------------------------------------------- Packs

/// The "Your Packs" gallery: every pack imported into this profile, each
/// with a single button that makes it (and its bundled background video,
/// when it has one) live in one click - no separate trip to Layout Themes
/// to Apply and Live Wallpapers to pick the video.
fn pack_gallery_section(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let mut section = settings::section().title(fl!("packs-your-packs-title"));
    if app.installed_packs.is_empty() {
        section = section.add(settings::item(
            fl!("packs-none-yet"),
            text::body(fl!("packs-none-yet-desc")),
        ));
    } else {
        for pack in &app.installed_packs {
            let is_active = app.wp_config.audio.style == pack.name;
            let description = match (pack.background.is_some(), is_active) {
                (true, true) => fl!("packs-includes-video-active"),
                (true, false) => fl!("packs-includes-video"),
                (false, true) => fl!("common-active-now"),
                (false, false) => fl!("packs-layout-only"),
            };
            section = section.add(
                settings::item::builder(pack.name.as_str())
                    .description(description)
                    .control(if is_active {
                        button::standard(fl!("common-active"))
                    } else {
                        button::suggested(fl!("common-apply"))
                            .on_press(Message::ApplyPack(pack.name.clone()))
                    }),
            );
        }
    }
    section.into()
}

fn packs(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let export_selected = app
        .pack_export_theme
        .as_ref()
        .and_then(|t| app.available_themes.iter().position(|name| name == t));

    // The background video isn't tied to a specific theme - it's whatever
    // is currently active - so the export description names it explicitly
    // rather than implying "this theme's video", which could be a
    // completely unrelated clip to the theme you're about to bundle.
    let export_description = match app.wp_config.appearance.video_background_path.as_deref() {
        Some(file) => fl!("packs-export-desc-with-video", file = file),
        None => fl!("packs-export-desc-no-video"),
    };

    let sections = vec![
        pack_gallery_section(app),
        settings::section()
            .title(fl!("packs-export-title"))
            .add(
                settings::item::builder(fl!("packs-theme-to-bundle"))
                    .description(export_description)
                    .control(
                        Row::new()
                            .push(dropdown(
                                &app.available_themes[..],
                                export_selected,
                                Message::PackExportThemeSelected,
                            ))
                            .push(
                                button::suggested(fl!("packs-export-pack"))
                                    .on_press(Message::ExportPack),
                            )
                            .spacing(8)
                            .align_y(cosmic::iced::Alignment::Center),
                    ),
            )
            .add(
                settings::item::builder(fl!("packs-folder"))
                    .description(fl!("packs-folder-desc"))
                    .control(
                        button::standard(fl!("common-open-folder"))
                            .on_press(Message::OpenPacksFolder),
                    ),
            )
            .into(),
        settings::item(fl!("packs-import"), text::body(fl!("packs-import-desc"))).into(),
    ];

    let content = page(
        app,
        fl!("packs-page-title"),
        fl!("packs-page-summary"),
        sections,
    );

    cosmic::widget::dnd_destination::dnd_destination_for_data(
        content,
        |data: Option<super::library::DroppedFiles>, _action| Message::PackFilesDropped(data),
    )
    .into()
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
            .title(fl!("now-playing-album-art-title"))
            .add(
                settings::item::builder(fl!("now-playing-show-album-art"))
                    .description(fl!("now-playing-show-album-art-desc"))
                    .toggler(
                        app.wp_config.appearance.show_album_art,
                        Message::ToggleShowAlbumArt,
                    ),
            )
            .into(),
        settings::section()
            .title(fl!("now-playing-lyrics-text-title"))
            .add(
                settings::item::builder(fl!("now-playing-show-lyrics"))
                    .description(fl!("now-playing-show-lyrics-desc"))
                    .toggler(app.wp_config.audio.show_lyrics, Message::ToggleShowLyrics),
            )
            .add(
                settings::item::builder(fl!("now-playing-font-family")).control(dropdown(
                    &app.available_fonts[..],
                    Some(current_font),
                    Message::FontSelected,
                )),
            )
            .into(),
    ];

    page(
        app,
        fl!("now-playing-page-title"),
        fl!("now-playing-page-summary"),
        sections,
    )
}

// -------------------------------------------------------------- Visualiser

fn visualiser(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let sections = vec![settings::section()
        .title(fl!("visualiser-audio-response-title"))
        .add(
            settings::item::builder(fl!("visualiser-bands"))
                .description(fl!("visualiser-bands-desc"))
                .control(stepped_slider(
                    fl!("visualiser-bands"),
                    format!("{}", app.wp_config.audio.bands),
                    app.wp_config.audio.bands as f32,
                    16.0..=128.0,
                    1.0,
                    Message::BandsChanged,
                )),
        )
        .add(
            settings::item::builder(fl!("visualiser-smoothing"))
                .description(fl!("visualiser-smoothing-desc"))
                .control(stepped_slider(
                    fl!("visualiser-smoothing"),
                    format!("{:.2}", app.wp_config.audio.smoothing),
                    app.wp_config.audio.smoothing,
                    0.0..=0.95,
                    0.05,
                    Message::SmoothingChanged,
                )),
        )
        .into()];

    page(
        app,
        fl!("visualiser-page-title"),
        fl!("visualiser-page-summary"),
        sections,
    )
}

// ----------------------------------------------------------------- Weather

fn temperature_unit_labels() -> Vec<String> {
    vec![fl!("weather-unit-celsius"), fl!("weather-unit-fahrenheit")]
}
fn poll_labels() -> Vec<String> {
    vec![
        fl!("weather-poll-5min"),
        fl!("weather-poll-15min"),
        fl!("weather-poll-30min"),
        fl!("weather-poll-1hour"),
    ]
}
pub(crate) const POLL_MINUTES: [u64; 4] = [5, 15, 30, 60];

fn weather(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    let current_unit = match app.wp_config.weather.temperature_unit {
        cosmic_wallpaper::modules::config::TemperatureUnit::Celsius => 0,
        cosmic_wallpaper::modules::config::TemperatureUnit::Fahrenheit => 1,
    };

    let sections = vec![settings::section()
        .title(fl!("weather-page-title"))
        .add(
            settings::item::builder(fl!("weather-show-weather"))
                .description(fl!("weather-show-weather-desc"))
                .toggler(app.wp_config.weather.enabled, Message::ToggleWeatherEnabled),
        )
        .add(
            settings::item::builder(fl!("weather-hide-effects"))
                .description(fl!("weather-hide-effects-desc"))
                .toggler(
                    app.wp_config.weather.hide_effects,
                    Message::ToggleHideWeatherEffects,
                ),
        )
        .add(
            settings::item::builder(fl!("weather-units")).control(dropdown(
                temperature_unit_labels(),
                Some(current_unit),
                Message::TemperatureUnitSelected,
            )),
        )
        .add(
            settings::item::builder(fl!("weather-location"))
                .description(fl!("weather-location-desc"))
                .control(
                    Row::new()
                        .push(
                            text_input(fl!("weather-latitude-placeholder"), &app.lat_input)
                                .on_input(Message::LatitudeChanged)
                                .width(Length::Fixed(100.0)),
                        )
                        .push(
                            text_input(fl!("weather-longitude-placeholder"), &app.lon_input)
                                .on_input(Message::LongitudeChanged)
                                .width(Length::Fixed(100.0)),
                        )
                        .push(
                            button::standard(fl!("weather-use-my-location"))
                                .on_press(Message::DetectLocation),
                        )
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center),
                ),
        )
        .add(
            settings::item::builder(fl!("weather-update-every")).control(dropdown(
                poll_labels(),
                POLL_MINUTES
                    .iter()
                    .position(|m| *m == app.wp_config.weather.poll_interval_minutes),
                Message::PollIntervalSelected,
            )),
        )
        .into()];

    page(
        app,
        fl!("weather-page-title"),
        fl!("weather-page-summary"),
        sections,
    )
}

// ----------------------------------------------------------------- General

fn general(app: &SettingsApp) -> cosmic::Element<'_, Message> {
    // "Up to date" and a failed check both get a recheck button alongside
    // the status - a silent failure (rate limit, network blip) used to
    // read identically to a genuine "you're current", which is actively
    // misleading given GitHub's unauthenticated API is capped at 60
    // requests/hour per IP and shared with anything else on the network.
    let update_control: cosmic::Element<'_, Message> = match &app.update_state {
        UpdateState::Checking => text::body(fl!("general-checking-for-updates")).into(),
        UpdateState::UpToDate => Row::new()
            .push(text::body(fl!("general-up-to-date")))
            .push(
                button::standard(fl!("general-check-for-updates"))
                    .on_press(Message::CheckForUpdates),
            )
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .into(),
        UpdateState::CheckFailed(reason) => Row::new()
            .push(text::body(fl!(
                "general-check-failed",
                reason = reason.as_str()
            )))
            .push(button::standard(fl!("common-retry")).on_press(Message::CheckForUpdates))
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .into(),
        UpdateState::Available(tag) if super::updater::is_self_updatable() => {
            button::suggested(fl!("general-update-to", tag = tag.as_str()))
                .on_press(Message::StartUpdate)
                .into()
        }
        // Installed via a system package - point at the release page instead.
        UpdateState::Available(tag) => {
            button::standard(fl!("general-release-page", tag = tag.as_str()))
                .on_press(Message::OpenUpdateLink)
                .into()
        }
        UpdateState::Updating(tag) => {
            text::body(fl!("general-updating-to", tag = tag.as_str())).into()
        }
        UpdateState::Installed(tag) => {
            text::body(fl!("general-installed-restart", tag = tag.as_str())).into()
        }
    };

    let language_labels: Vec<String> = std::iter::once(fl!("general-language-system-default"))
        .chain(
            cosmic_wallpaper::modules::i18n::AVAILABLE_LANGUAGES
                .iter()
                .map(|(_, name)| name.clone()),
        )
        .collect();
    let language_selected = app
        .wp_config
        .language
        .as_deref()
        .and_then(|tag| {
            cosmic_wallpaper::modules::i18n::AVAILABLE_LANGUAGES
                .iter()
                .position(|(t, _)| t == tag)
        })
        .map_or(0, |idx| idx + 1);

    let mut sections = vec![
        settings::section()
            .title(fl!("general-language-title"))
            .add(
                settings::item::builder(fl!("general-language-title"))
                    .description(fl!("general-language-desc"))
                    .control(dropdown(
                        language_labels,
                        Some(language_selected),
                        Message::LanguageSelected,
                    )),
            )
            .into(),
        settings::section()
            .title(fl!("general-engine-title"))
            .add(
                settings::item::builder(fl!("general-wallpaper-engine"))
                    .description(match (app.engine_pid, &app.engine_failure) {
                        (Some(pid), _) => fl!("general-engine-running", pid = pid),
                        // A binary that dies before main() (linker failure
                        // after a system update) is otherwise invisible.
                        (None, Some(failure)) => failure.clone(),
                        (None, None) => fl!("general-engine-not-running"),
                    })
                    .control(if app.engine_pid.is_some() {
                        button::standard(fl!("common-stop")).on_press(Message::StopEngine)
                    } else {
                        button::suggested(fl!("common-start")).on_press(Message::StartEngine)
                    }),
            )
            .add(
                settings::item::builder(fl!("general-start-on-login"))
                    .description(fl!("general-start-on-login-desc"))
                    .toggler(app.autostart, Message::ToggleAutostart),
            )
            .add(
                settings::item::builder(fl!("general-frame-rate-limit"))
                    .description(fl!("general-frame-rate-limit-desc"))
                    .control(stepped_slider(
                        fl!("general-frame-rate-limit"),
                        format!("{} fps", app.wp_config.fps),
                        app.wp_config.fps as f32,
                        15.0..=144.0,
                        1.0,
                        Message::FpsChanged,
                    )),
            )
            .add(
                settings::item::builder(fl!("general-config-folder"))
                    .description(fl!("general-config-folder-desc"))
                    .control(
                        button::standard(fl!("common-open-folder"))
                            .on_press(Message::OpenConfigFolder),
                    ),
            )
            .into(),
        settings::section()
            .title(fl!("general-about-title"))
            .add(
                settings::item::builder(fl!("general-version"))
                    .description(env!("CARGO_PKG_VERSION"))
                    .control(update_control),
            )
            .add(
                settings::item::builder(fl!("general-patch-notes"))
                    .description(fl!("general-patch-notes-desc"))
                    .control(if app.patch_notes.is_some() {
                        button::standard(fl!("common-hide")).on_press(Message::ClosePatchNotes)
                    } else {
                        button::standard(fl!("common-show")).on_press(Message::ShowPatchNotes)
                    }),
            )
            .add(
                settings::item::builder(fl!("general-diagnostics"))
                    .description(fl!("general-diagnostics-desc"))
                    .control(
                        button::standard(fl!("common-copy")).on_press(Message::CopyDiagnostics),
                    ),
            )
            .add(
                settings::item::builder(fl!("general-something-broken"))
                    .description(fl!("general-something-broken-desc"))
                    .control(
                        button::standard(fl!("general-report-an-issue"))
                            .on_press(Message::ReportIssue),
                    ),
            )
            .into(),
    ];

    // Self-hiding: only appears when there's actually something to fix, so
    // it doesn't clutter the page once setup is in good shape.
    if let Some(issue) = &app.launcher_issue {
        sections.insert(
            0,
            settings::section()
                .title(fl!("general-setup-title"))
                .add(settings::item(
                    fl!("general-not-in-launcher"),
                    text::body(issue.clone()),
                ))
                .into(),
        );
    }

    if let Some(notes) = &app.patch_notes {
        // markdown::Settings has no From<cosmic::Theme> (only iced's own
        // Theme enum gets that shortcut) - COSMIC's Theme instead bridges
        // via the iced::theme::Base trait's palette(), the same colours
        // (accent/success/warning/destructive/bg/on_bg) every other themed
        // widget in this app already follows.
        use cosmic::iced::theme::Base as _;
        let palette = cosmic::theme::active()
            .palette()
            .unwrap_or(cosmic::iced::theme::Palette::LIGHT);
        let markdown_settings = cosmic::widget::markdown::Settings::with_style(
            cosmic::widget::markdown::Style::from_palette(palette),
        );
        sections.push(
            settings::section()
                .title(fl!("general-patch-notes-section-title"))
                .add(
                    Column::new()
                        .push(
                            cosmic::widget::markdown::view(notes, markdown_settings)
                                .map(Message::PatchNotesLinkClicked),
                        )
                        .width(Length::Fill)
                        .padding(8),
                )
                .into(),
        );
    }

    page(
        app,
        fl!("general-page-title"),
        fl!("general-page-summary"),
        sections,
    )
}
