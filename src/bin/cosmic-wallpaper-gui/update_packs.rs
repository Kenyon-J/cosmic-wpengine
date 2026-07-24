//! `update()` handlers for the Packs page: exporting a `.cwtheme`,
//! importing dropped packs (including the custom-shader review gate),
//! and re-applying an already-installed pack.
use super::*;

impl SettingsApp {
    pub(super) fn on_pack_export_theme_selected(
        &mut self,
        idx: usize,
    ) -> Task<cosmic::Action<Message>> {
        self.pack_export_theme = self.available_themes.get(idx).cloned();
        Task::none()
    }

    pub(super) fn on_export_pack(&mut self) -> Task<cosmic::Action<Message>> {
        let Some(name) = self.pack_export_theme.clone() else {
            self.status_msg = fl!("status-select-theme-to-export");
            return Task::none();
        };
        match self.export_pack(&name) {
            Ok(path) => {
                self.status_msg = fl!("status-exported-pack", path = path.display().to_string());
            }
            Err(e) => {
                self.status_msg = fl!("status-error-exporting-pack", error = e.to_string());
            }
        }
        Task::none()
    }

    pub(super) fn on_open_packs_folder(&mut self) -> Task<cosmic::Action<Message>> {
        let dir = library::packs_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Some(xdg_open) = resolve_binary("xdg-open") {
            let _ = std::process::Command::new(xdg_open).arg(dir).spawn();
        } else {
            tracing::warn!("Failed to open folder: xdg-open not found in trusted PATH");
            self.status_msg = fl!("status-xdg-open-folder-not-found");
        }
        Task::none()
    }

    pub(super) fn on_pack_files_dropped(
        &mut self,
        files: Option<library::DroppedFiles>,
    ) -> Task<cosmic::Action<Message>> {
        self.drop_hover = false;
        let paths = files.map(|f| f.0).unwrap_or_default();
        let mut imported = 0;
        let mut imported_a_background = false;
        let mut messages: Vec<String> = Vec::new();
        for path in paths
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e == "cwtheme"))
        {
            // Checked via metadata, before reading a single byte:
            // an oversized (or maliciously crafted) `.cwtheme`
            // dropped onto this page used to be read into memory
            // in full and handed to `parse()` before the
            // shader-review gate - or any validation at all - ever
            // got a chance to reject it.
            match std::fs::metadata(path) {
                Ok(meta) if meta.len() > config::pack::MAX_PACK_BYTES => {
                    messages.push(fl!(
                        "status-pack-too-large",
                        name = path.display().to_string(),
                        limit = ((config::pack::MAX_PACK_BYTES / (1024 * 1024)) as i64)
                    ));
                    continue;
                }
                Err(_) => {
                    messages.push(fl!("status-could-not-read-dropped-file"));
                    continue;
                }
                _ => {}
            }
            let Ok(bytes) = std::fs::read(path) else {
                messages.push(fl!("status-could-not-read-dropped-file"));
                continue;
            };
            match config::pack::parse(&bytes) {
                Ok(parsed) => {
                    let background = parsed
                        .background
                        .filter(|(name, _)| library::is_video_file(std::path::Path::new(name)));
                    if let Some(shader) = parsed.shader {
                        if self.pending_pack_import.is_some() {
                            messages.push(fl!(
                                "status-skip-one-shader-at-a-time",
                                name = parsed.name.as_str()
                            ));
                            continue;
                        }
                        self.pending_pack_import = Some(PendingPackImport {
                            name: parsed.name,
                            theme_toml: parsed.theme_toml,
                            background,
                            shader,
                        });
                    } else {
                        let has_background = background.is_some();
                        match self.finalize_pack_import(
                            &parsed.name,
                            &parsed.theme_toml,
                            background,
                            None,
                        ) {
                            Ok(written_as) => {
                                imported += 1;
                                imported_a_background |= has_background;
                                if written_as != parsed.name {
                                    messages.push(fl!(
                                        "status-pack-renamed-on-import",
                                        original = parsed.name.as_str(),
                                        written = written_as.as_str()
                                    ));
                                }
                            }
                            Err(e) => messages.push(fl!(
                                "status-pack-import-error",
                                name = parsed.name.as_str(),
                                error = e.to_string()
                            )),
                        }
                    }
                }
                Err(e) => messages.push(fl!("status-not-a-valid-pack", error = e.to_string())),
            }
        }
        if imported > 0 {
            self.available_themes = load_themes();
            self.installed_packs = library::scan_installed_packs();
            messages.insert(
                0,
                fl!("status-imported-packs", imported = (imported as i64)),
            );
        }
        if self.pending_pack_import.is_some() {
            messages.push(fl!("status-pack-includes-shader-review"));
        }
        if !messages.is_empty() {
            self.status_msg = messages.join(" ");
        } else if self.status_msg == fl!("status-ready") {
            self.status_msg = fl!("status-nothing-imported-packs");
        }
        if imported_a_background {
            return scan_library_task();
        }
        Task::none()
    }

    pub(super) fn on_confirm_pack_import(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(pending) = self.pending_pack_import.take() {
            let name = pending.name.clone();
            let has_video = pending.background.is_some();
            match self.finalize_pack_import(
                &pending.name,
                &pending.theme_toml,
                pending.background,
                Some(pending.shader),
            ) {
                Ok(written_as) => {
                    self.available_themes = load_themes();
                    self.installed_packs = library::scan_installed_packs();
                    let detail = if has_video {
                        fl!("status-pack-detail-video-shader")
                    } else {
                        fl!("status-pack-detail-shader")
                    };
                    self.status_msg = if written_as == name {
                        fl!(
                            "status-imported-pack-named",
                            name = name.as_str(),
                            detail = detail.as_str()
                        )
                    } else {
                        fl!(
                            "status-pack-renamed-with-detail",
                            name = name.as_str(),
                            written = written_as.as_str(),
                            detail = detail.as_str()
                        )
                    };
                    if has_video {
                        return scan_library_task();
                    }
                }
                Err(e) => {
                    self.status_msg = fl!("status-error-importing-pack", error = e.to_string());
                }
            }
        }
        Task::none()
    }

    pub(super) fn on_cancel_pack_import(&mut self) -> Task<cosmic::Action<Message>> {
        self.pending_pack_import = None;
        self.status_msg = fl!("status-cancelled-nothing-imported");
        Task::none()
    }

    pub(super) fn on_apply_pack(&mut self, name: String) -> Task<cosmic::Action<Message>> {
        let entry = self.installed_packs.iter().find(|p| p.name == name);
        // A record whose background no longer exists in
        // videos_dir() (deleted by hand since importing) is still
        // listed in the gallery - only apply the video setting
        // when the file is actually still there, so Apply can't
        // silently point the config at a missing file.
        let background = entry
            .and_then(|p| p.background.clone())
            .filter(|file| library::videos_dir().join(file).exists());
        let video_missing = entry.is_some_and(|p| p.background.is_some()) && background.is_none();
        let theme_missing = !config::Config::config_dir()
            .join("shaders")
            .join(format!("{name}.toml"))
            .exists();

        self.wp_config.audio.style = name.clone();
        let has_video = background.is_some();
        if let Some(file) = background {
            // Mirrors BackgroundMode::Video's own reset in
            // Message::BackgroundModeSelected: setting the video
            // path alone, without clearing the other background
            // flags, could leave a combination the Wallpaper page's
            // own mode switch never produces (e.g. Album Colour's
            // flag still on underneath the newly-active video).
            self.wp_config.appearance.disable_blur = false;
            self.wp_config.appearance.transparent_background = false;
            self.wp_config.appearance.album_art_background = false;
            self.wp_config.appearance.album_color_background = false;
            self.wp_config.appearance.video_background_path = Some(file);
        }
        self.switch_edit_theme(Some(name.clone()));
        match self.wp_config.save() {
            Ok(()) => {
                self.status_msg = match (has_video, video_missing, theme_missing) {
                    (true, _, _) => fl!("status-applied-pack-with-video", name = name.as_str()),
                    (false, true, _) => {
                        fl!("status-applied-pack-video-missing", name = name.as_str())
                    }
                    (false, false, true) => {
                        fl!("status-applied-pack-theme-missing", name = name.as_str())
                    }
                    (false, false, false) => fl!("status-applied-pack", name = name.as_str()),
                };
            }
            Err(e) => self.status_msg = fl!("status-error-applying-pack", error = e.to_string()),
        }
        Task::none()
    }
}
