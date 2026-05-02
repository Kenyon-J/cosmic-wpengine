use cosmic::iced::widget::{checkbox, pick_list, slider};
use cosmic::iced::Length;
use cosmic::widget::{column, row, text, text_editor, text_input};
pub(crate) fn view_app(app: &super::SettingsApp) -> cosmic::Element<'_, super::Message> {
    let font = cosmic::iced::Font::DEFAULT;

    let current_bg_mode = if app.wp_config.appearance.video_background_path.is_some() {
        super::BackgroundMode::Video
    } else if app.wp_config.appearance.album_color_background {
        super::BackgroundMode::AlbumPalette
    } else if app.wp_config.appearance.album_art_background {
        super::BackgroundMode::AlbumArt
    } else if app.wp_config.appearance.transparent_background {
        super::BackgroundMode::Transparent
    } else {
        super::BackgroundMode::FrostedGlass
    };

    let bg_mode_selector = cosmic::iced::widget::tooltip(
        pick_list(
            &super::BackgroundMode::ALL[..],
            Some(current_bg_mode),
            super::Message::BackgroundModeSelected,
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

    if current_bg_mode == super::BackgroundMode::Video {
        let video_selector = cosmic::iced::widget::tooltip(
            pick_list(
                app.available_videos.clone(),
                app.wp_config.appearance.video_background_path.clone(),
                super::Message::VideoSelected,
            )
            .placeholder(if app.available_videos.is_empty() {
                "Place videos in ~/.config/cosmic-wallpaper/videos"
            } else {
                "Select a video..."
            }),
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
                    checkbox(app.wp_config.appearance.show_album_art)
                        .on_toggle(super::Message::ToggleShowAlbumArt)
                        .label("Show Album Art Foreground")
                        .font(font),
                    "Displays the current album cover over the background.",
                    cosmic::iced::widget::tooltip::Position::Top,
                ))
                .push(cosmic::iced::widget::tooltip(
                    checkbox(app.wp_config.audio.show_lyrics)
                        .on_toggle(super::Message::ToggleShowLyrics)
                        .label("Show Lyrics")
                        .font(font),
                    "Displays scrolling lyrics for the current track.",
                    cosmic::iced::widget::tooltip::Position::Top,
                ))
                .push(cosmic::iced::widget::tooltip(
                    checkbox(app.autostart)
                        .on_toggle(super::Message::ToggleAutostart)
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
                    checkbox(app.wp_config.weather.enabled)
                        .on_toggle(super::Message::ToggleWeatherEnabled)
                        .label("Enable Weather")
                        .font(font),
                    "Displays current weather information on the desktop.",
                    cosmic::iced::widget::tooltip::Position::Top,
                ))
                .push({
                    let cb = checkbox(app.wp_config.weather.hide_effects)
                        .label("Hide Weather Effects")
                        .font(font);
                    let element: cosmic::Element<'_, super::Message> =
                        if app.wp_config.weather.enabled {
                            cosmic::iced::widget::tooltip(
                                cb.on_toggle(super::Message::ToggleHideWeatherEffects),
                                "Disables rain and snow animations to save performance.",
                                cosmic::iced::widget::tooltip::Position::Top,
                            )
                            .into()
                        } else {
                            cosmic::iced::widget::tooltip(
                                cb,
                                "Enable Weather first to use this setting.",
                                cosmic::iced::widget::tooltip::Position::Top,
                            )
                            .into()
                        };
                    element
                })
                .spacing(20),
        )
        .spacing(15);

    let font_row = row()
        .push(text("Font Family:").font(font).width(Length::Fixed(200.0)))
        .push(cosmic::iced::widget::tooltip(
            pick_list(
                app.available_fonts.clone(),
                app.wp_config
                    .appearance
                    .font_family
                    .clone()
                    .or_else(|| Some("System Default".to_string())),
                super::Message::FontFamilySelected,
            )
            .placeholder("Select a font..."),
            "Select the font used for displaying the clock, weather, and lyrics.",
            cosmic::iced::widget::tooltip::Position::Top,
        ))
        .spacing(20);

    let framerate_row = row()
        .push(
            text(format!("Target Framerate: {} FPS", app.wp_config.fps))
                .font(font)
                .width(Length::Fixed(200.0)),
        )
        .push(cosmic::iced::widget::tooltip(
            slider(
                15.0..=144.0,
                app.wp_config.fps as f32,
                super::Message::FpsChanged,
            ),
            "Higher framerates are smoother but use more system resources.",
            cosmic::iced::widget::tooltip::Position::Top,
        ))
        .spacing(20);

    let blur_row = row()
        .push(
            text(format!(
                "Blur Strength: {:.2}",
                app.wp_config.appearance.blur_opacity
            ))
            .font(font)
            .width(Length::Fixed(200.0)),
        )
        .push(cosmic::iced::widget::tooltip(
            slider(
                0.0..=1.0,
                app.wp_config.appearance.blur_opacity,
                super::Message::BlurOpacityChanged,
            )
            .step(0.05),
            "Controls the strength of the background blur.",
            cosmic::iced::widget::tooltip::Position::Top,
        ))
        .spacing(20);

    let file_selector = cosmic::iced::widget::tooltip(
        pick_list(
            app.available_files.clone(),
            app.selected_file.clone(),
            super::Message::FileSelected,
        )
        .placeholder("Select a file..."),
        "Select a configuration or shader theme file to edit.",
        cosmic::iced::widget::tooltip::Position::Top,
    );

    let save_btn: cosmic::Element<'_, super::Message> = {
        let btn = cosmic::iced::widget::button(text("Save File").font(font));
        if app.selected_file.is_some() {
            cosmic::iced::widget::tooltip(
                btn.on_press(super::Message::SaveFile),
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

    let apply_btn: cosmic::Element<'_, super::Message> = {
        let selected_theme = app.selected_file.as_ref().and_then(|f| {
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
            if theme_name == app.wp_config.audio.style {
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
                    btn.on_press(super::Message::ApplyTheme),
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
        text_input("New Theme Name...", &app.new_theme_name)
            .on_input(super::Message::NewThemeNameChanged)
            .on_submit(|_| super::Message::CreateTheme),
        "Enter a name to create a new copy of the current theme.",
        cosmic::iced::widget::tooltip::Position::Top,
    );

    let create_btn: cosmic::Element<'_, super::Message> = {
        let btn = cosmic::iced::widget::button(text("Create Theme").font(font));
        if !app.new_theme_name.trim().is_empty() {
            cosmic::iced::widget::tooltip(
                btn.on_press(super::Message::CreateTheme),
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

    let editor = text_editor(&app.editor_content)
        .font(cosmic::iced::Font::MONOSPACE)
        .on_action(super::Message::EditorAction)
        .height(Length::Fill);

    let report_btn = cosmic::iced::widget::tooltip(
        cosmic::iced::widget::button(text("Report Issue").font(font).size(14))
            .on_press(super::Message::ReportIssue),
        "Open GitHub to report a bug or request a feature.",
        cosmic::iced::widget::tooltip::Position::Top,
    );
    let notes_btn: cosmic::Element<'_, super::Message> = {
        if app.status_msg == "Fetching patch notes..." {
            let btn = cosmic::iced::widget::button(text("Fetching...").font(font).size(14));
            cosmic::iced::widget::tooltip(
                btn,
                "Downloading patch notes from GitHub...",
                cosmic::iced::widget::tooltip::Position::Top,
            )
            .into()
        } else {
            let btn = cosmic::iced::widget::button(text("Patch Notes").font(font).size(14));
            cosmic::iced::widget::tooltip(
                btn.on_press(super::Message::ShowPatchNotes),
                "View recent changes and updates to the engine.",
                cosmic::iced::widget::tooltip::Position::Top,
            )
            .into()
        }
    };

    let version_display: cosmic::Element<'_, super::Message> =
        if let Some(new_v) = &app.update_available {
            let update_btn = cosmic::iced::widget::button(
                text(format!("Update Available: {}", new_v))
                    .font(font)
                    .size(14),
            )
            .on_press(super::Message::OpenUpdateLink);

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
            text(&app.status_msg)
                .font(font)
                .size(14)
                .width(Length::Fill),
        )
        .push(version_display)
        .push(notes_btn)
        .push(report_btn)
        .spacing(15);

    let mut main_col = column()
        .push(text("COSMIC Wallpaper Settings").font(font).size(32))
        .push(toggles_row)
        .push(font_row)
        .push(framerate_row);

    if current_bg_mode == super::BackgroundMode::FrostedGlass {
        main_col = main_col.push(blur_row);
    }

    main_col
        .push(toolbar)
        .push(editor)
        .push(footer_row)
        .padding(40)
        .spacing(20)
        .into()
}
