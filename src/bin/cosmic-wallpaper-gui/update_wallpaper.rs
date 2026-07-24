//! `update()` handlers for the Wallpaper page: background mode, font,
//! album-art toggle, text colour, and the preview strip.
use super::*;

impl SettingsApp {
    pub(super) fn on_background_mode_selected(
        &mut self,
        mode: BackgroundMode,
    ) -> Task<cosmic::Action<Message>> {
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
                        self.wp_config.appearance.video_background_path = Some(first_video.clone());
                    }
                }
            }
        }
        let _ = self.wp_config.save();
        Task::none()
    }

    pub(super) fn on_font_selected(&mut self, idx: usize) -> Task<cosmic::Action<Message>> {
        if let Some(family) = self.available_fonts.get(idx) {
            self.wp_config.appearance.font_family =
                (family != "System Default").then(|| family.clone());
            let _ = self.wp_config.save();
        }
        Task::none()
    }

    pub(super) fn on_toggle_show_album_art(
        &mut self,
        state: bool,
    ) -> Task<cosmic::Action<Message>> {
        self.wp_config.appearance.show_album_art = state;
        let _ = self.wp_config.save();
        Task::none()
    }

    pub(super) fn on_wallpaper_preview_loaded(
        &mut self,
        preview: Option<Box<WallpaperPreview>>,
    ) -> Task<cosmic::Action<Message>> {
        self.wallpaper_preview = preview.map(|boxed| *boxed);
        Task::none()
    }

    pub(super) fn on_text_color_mode(&mut self, idx: usize) -> Task<cosmic::Action<Message>> {
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
        Task::none()
    }

    pub(super) fn on_text_color_picker(
        &mut self,
        update: ColorPickerUpdate,
    ) -> Task<cosmic::Action<Message>> {
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
        self.color_picker.update::<cosmic::Action<Message>>(update)
    }
}
