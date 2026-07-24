//! `update()` handlers for the Live Wallpapers page: the video library,
//! its drag-and-drop import, and the canvas-preference toggle.
use super::*;

impl SettingsApp {
    pub(super) fn on_video_selected(&mut self, idx: usize) -> Task<cosmic::Action<Message>> {
        if let Some(entry) = self.library.get(idx) {
            self.wp_config.appearance.video_background_path = Some(entry.file_name.clone());
            let _ = self.wp_config.save();
        }
        Task::none()
    }

    pub(super) fn on_toggle_watch_canvas(&mut self, state: bool) -> Task<cosmic::Action<Message>> {
        self.wp_config.appearance.prefer_canvas = state;
        let _ = self.wp_config.save();
        Task::none()
    }

    pub(super) fn on_library_loaded(
        &mut self,
        entries: Vec<library::VideoEntry>,
    ) -> Task<cosmic::Action<Message>> {
        self.library = entries;
        Task::none()
    }

    pub(super) fn on_dnd_entered(&mut self) -> Task<cosmic::Action<Message>> {
        self.drop_hover = true;
        Task::none()
    }

    pub(super) fn on_dnd_left(&mut self) -> Task<cosmic::Action<Message>> {
        self.drop_hover = false;
        Task::none()
    }

    pub(super) fn on_files_dropped(
        &mut self,
        files: Option<library::DroppedFiles>,
    ) -> Task<cosmic::Action<Message>> {
        self.drop_hover = false;
        let paths = files.map(|f| f.0).unwrap_or_default();
        if paths.is_empty() {
            self.status_msg = fl!("status-nothing-usable-dropped");
            Task::none()
        } else {
            self.status_msg = fl!("status-importing-files", count = (paths.len() as i64));
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || library::import(paths))
                        .await
                        .unwrap_or((0, 0))
                },
                |(imported, skipped)| Message::ImportDone { imported, skipped }.into(),
            )
        }
    }

    pub(super) fn on_import_done(
        &mut self,
        imported: usize,
        skipped: usize,
    ) -> Task<cosmic::Action<Message>> {
        self.status_msg = match (imported, skipped) {
            (0, _) => fl!("status-no-videos-imported"),
            (n, 0) => fl!("status-imported-videos", n = (n as i64)),
            (n, s) => fl!("status-imported-skipped", n = (n as i64), s = (s as i64)),
        };
        scan_library_task()
    }
}
