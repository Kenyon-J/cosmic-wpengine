//! `update()` handlers for the General page: engine process control
//! (start/stop/probe/autostart), fps/blur sliders, language, patch notes,
//! diagnostics/bug-report, and the self-updater.
use super::*;

impl SettingsApp {
    pub(super) fn on_start_engine(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(engine) = engine_binary_path() {
            // stderr goes to a scratch file, not a pipe: a pipe's
            // buffer would fill (and its reader vanish with this
            // process) under a long-lived engine, while a file is
            // safe to leave attached forever - and holds the dynamic
            // linker's message when the binary can't load at all.
            let stderr_log = std::env::temp_dir().join("cosmic-wallpaper-start.log");
            let stderr = std::fs::File::create(&stderr_log)
                .map(std::process::Stdio::from)
                .unwrap_or_else(|_| std::process::Stdio::null());
            match std::process::Command::new(engine).stderr(stderr).spawn() {
                Ok(mut child) => {
                    self.status_msg = fl!("status-engine-starting");
                    return Task::perform(
                        async move {
                            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                            tokio::task::spawn_blocking(move || {
                                match child.try_wait() {
                                    // Died within the probe window:
                                    // report how, from the exit code
                                    // and captured stderr.
                                    Ok(Some(status)) => {
                                        let stderr = std::fs::read_to_string(&stderr_log)
                                            .unwrap_or_default();
                                        Some((status.code(), stderr_headline(&stderr)))
                                    }
                                    _ => None,
                                }
                            })
                            .await
                            .unwrap_or(None)
                        },
                        |probe| Message::EngineStartProbed(probe).into(),
                    );
                }
                Err(e) => {
                    self.status_msg = fl!("status-failed-to-start-engine", error = e.to_string())
                }
            }
        } else {
            self.status_msg = fl!("status-could-not-find-engine-binary");
        }
        Task::none()
    }

    pub(super) fn on_engine_start_probed(
        &mut self,
        probe: Option<(Option<i32>, String)>,
    ) -> Task<cosmic::Action<Message>> {
        self.refresh_engine_status();
        match probe {
            Some((code, headline)) => {
                let code = code.map_or_else(
                    || fl!("status-killed-by-signal"),
                    |c| fl!("status-exit-code", code = c),
                );
                let detail = if headline.is_empty() {
                    fl!("status-engine-exited-immediately", code = code.as_str())
                } else {
                    fl!(
                        "status-engine-exited-immediately-detail",
                        code = code.as_str(),
                        headline = headline.as_str()
                    )
                };
                self.status_msg = detail.clone();
                self.engine_failure = Some(detail);
            }
            None => {
                self.status_msg = match self.engine_pid {
                    Some(_) => fl!("status-engine-running"),
                    None => fl!("status-engine-did-not-start"),
                };
            }
        }
        Task::none()
    }

    pub(super) fn on_stop_engine(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(pid) = self.engine_pid {
            // The tray's Quit item is the tested graceful-shutdown
            // path; menu id 3 is Quit Engine.
            if let Some(busctl) = resolve_binary("busctl") {
                let _ = std::process::Command::new(busctl)
                    .args([
                        "--user",
                        "call",
                        &format!("org.kde.StatusNotifierItem-{pid}-1"),
                        "/MenuBar",
                        "com.canonical.dbusmenu",
                        "Event",
                        "isvu",
                        "3",
                        "clicked",
                        "v",
                        "s",
                        "",
                        "0",
                    ])
                    .output();
                self.status_msg = fl!("status-engine-stopping");
                return Task::perform(
                    tokio::time::sleep(std::time::Duration::from_millis(1500)),
                    |()| Message::RefreshEngineStatus.into(),
                );
            }
        }
        Task::none()
    }

    pub(super) fn on_refresh_engine_status(&mut self) -> Task<cosmic::Action<Message>> {
        self.refresh_engine_status();
        // Resolve the transitional status set by Start/Stop.
        if self.status_msg == fl!("status-engine-starting") {
            self.status_msg = match self.engine_pid {
                Some(_) => fl!("status-engine-running"),
                None => fl!("status-engine-did-not-start"),
            };
        } else if self.status_msg == fl!("status-engine-stopping") {
            self.status_msg = match self.engine_pid {
                Some(_) => fl!("status-engine-still-running"),
                None => fl!("status-engine-stopped"),
            };
        }
        Task::none()
    }

    pub(super) fn on_toggle_autostart(&mut self, state: bool) -> Task<cosmic::Action<Message>> {
        self.autostart = state;
        set_autostart(state);
        Task::none()
    }

    pub(super) fn on_fps_changed(&mut self, fps: f32) -> Task<cosmic::Action<Message>> {
        self.wp_config.fps = fps as u32;
        self.schedule_debounced_save()
    }

    pub(super) fn on_blur_opacity_changed(
        &mut self,
        opacity: f32,
    ) -> Task<cosmic::Action<Message>> {
        self.wp_config.appearance.blur_opacity = opacity;
        self.schedule_debounced_save()
    }

    pub(super) fn on_language_selected(&mut self, idx: usize) -> Task<cosmic::Action<Message>> {
        self.wp_config.language = idx
            .checked_sub(1)
            .and_then(|i| cosmic_wallpaper::modules::i18n::AVAILABLE_LANGUAGES.get(i))
            .map(|(tag, _)| tag.clone());
        cosmic_wallpaper::modules::i18n::set_language(self.wp_config.language.as_deref());
        let _ = self.wp_config.save();
        Task::none()
    }

    pub(super) fn on_debounced_save(&mut self, generation: u64) -> Task<cosmic::Action<Message>> {
        // A newer slider change re-armed the timer; let its own
        // DebouncedSave do the (single) write.
        if generation == self.save_generation {
            let _ = self.wp_config.save(); // Hot-reloads the engine via its file watcher
        }
        Task::none()
    }

    pub(super) fn on_show_patch_notes(&mut self) -> Task<cosmic::Action<Message>> {
        self.status_msg = fl!("status-fetching-patch-notes");
        Task::perform(fetch_patch_notes(), |notes| {
            Message::PatchNotesLoaded(notes).into()
        })
    }

    pub(super) fn on_patch_notes_loaded(&mut self, notes: String) -> Task<cosmic::Action<Message>> {
        self.patch_notes = Some(cosmic::widget::markdown::parse(&notes).collect());
        self.status_msg = fl!("status-ready");
        Task::none()
    }

    pub(super) fn on_patch_notes_link_clicked(
        &mut self,
        url: cosmic::widget::markdown::Uri,
    ) -> Task<cosmic::Action<Message>> {
        if let Some(xdg_open) = resolve_binary("xdg-open") {
            let _ = std::process::Command::new(xdg_open).arg(url).spawn();
        } else {
            tracing::warn!("Failed to open link: xdg-open not found in trusted PATH");
            self.status_msg = fl!("status-xdg-open-not-found");
        }
        Task::none()
    }

    pub(super) fn on_close_patch_notes(&mut self) -> Task<cosmic::Action<Message>> {
        self.patch_notes = None;
        Task::none()
    }

    pub(super) fn on_report_issue(&mut self) -> Task<cosmic::Action<Message>> {
        let body = build_issue_body();
        // Parsed via `url` rather than hand-formatted so the log
        // excerpt - which can contain '&', '#', newlines - is
        // correctly percent-encoded into the query string.
        let mut url = url::Url::parse("https://github.com/Kenyon-J/cosmic-wpengine/issues/new")
            .expect("static URL is always valid");
        url.query_pairs_mut().append_pair("body", &body);

        if let Some(xdg_open) = resolve_binary("xdg-open") {
            let _ = std::process::Command::new(xdg_open)
                .arg(url.as_str())
                .spawn();
        } else {
            tracing::warn!("Failed to open link: xdg-open not found in trusted PATH");
            self.status_msg = fl!("status-xdg-open-not-found");
        }
        Task::none()
    }

    pub(super) fn on_copy_diagnostics(&mut self) -> Task<cosmic::Action<Message>> {
        let text = build_diagnostics_text(self);
        self.status_msg = fl!("status-diagnostics-copied");
        cosmic::iced::clipboard::write(text)
    }

    pub(super) fn on_update_check_done(
        &mut self,
        result: Result<Option<String>, String>,
    ) -> Task<cosmic::Action<Message>> {
        self.update_state = match result {
            Ok(Some(v)) => UpdateState::Available(v),
            Ok(None) => UpdateState::UpToDate,
            Err(e) => UpdateState::CheckFailed(e),
        };
        Task::none()
    }

    pub(super) fn on_check_for_updates(&mut self) -> Task<cosmic::Action<Message>> {
        self.update_state = UpdateState::Checking;
        Task::perform(check_for_updates(), |result| {
            Message::UpdateCheckDone(result).into()
        })
    }

    pub(super) fn on_start_update(&mut self) -> Task<cosmic::Action<Message>> {
        if let UpdateState::Available(tag) = self.update_state.clone() {
            match get_http_client() {
                Ok(client) => {
                    let client = client.clone();
                    self.update_state = UpdateState::Updating(tag.clone());
                    self.status_msg = fl!("status-downloading", tag = tag.as_str());
                    return Task::perform(updater::perform_update(client, tag), |res| {
                        Message::UpdateFinished(res).into()
                    });
                }
                Err(e) => {
                    self.status_msg = fl!("status-update-failed", error = e.to_string());
                }
            }
        }
        Task::none()
    }

    pub(super) fn on_update_finished(
        &mut self,
        result: Result<String, String>,
    ) -> Task<cosmic::Action<Message>> {
        match result {
            Ok(tag) => {
                self.update_state = UpdateState::Installed(tag.clone());
                self.status_msg = fl!("status-updated-restart-needed", tag = tag.as_str());
            }
            Err(e) => {
                self.status_msg = fl!("status-update-failed", error = e.to_string());
                // Fall back to Available so the button offers a retry
                // instead of getting stuck showing "Updating...".
                self.update_state = match &self.update_state {
                    UpdateState::Updating(tag) => UpdateState::Available(tag.clone()),
                    other => other.clone(),
                };
            }
        }
        Task::none()
    }

    pub(super) fn on_open_update_link(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(xdg_open) = resolve_binary("xdg-open") {
            let _ = std::process::Command::new(xdg_open)
                .arg("https://github.com/Kenyon-J/cosmic-wpengine/releases/latest")
                .spawn();
        } else {
            tracing::warn!("Failed to open link: xdg-open not found in trusted PATH");
            self.status_msg = fl!("status-xdg-open-not-found");
        }
        Task::none()
    }

    pub(super) fn on_open_config_folder(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(xdg_open) = resolve_binary("xdg-open") {
            let config_dir = config::Config::config_dir();
            let _ = std::process::Command::new(xdg_open).arg(config_dir).spawn();
        } else {
            tracing::warn!("Failed to open folder: xdg-open not found in trusted PATH");
            self.status_msg = fl!("status-xdg-open-folder-not-found");
        }
        Task::none()
    }

    pub(super) fn on_open_videos_folder(&mut self) -> Task<cosmic::Action<Message>> {
        let videos_dir = config::Config::config_dir().join("videos");
        let _ = std::fs::create_dir_all(&videos_dir);
        if let Some(xdg_open) = resolve_binary("xdg-open") {
            let _ = std::process::Command::new(xdg_open).arg(videos_dir).spawn();
        } else {
            tracing::warn!("Failed to open folder: xdg-open not found in trusted PATH");
            self.status_msg = fl!("status-xdg-open-folder-not-found");
        }
        Task::none()
    }
}
