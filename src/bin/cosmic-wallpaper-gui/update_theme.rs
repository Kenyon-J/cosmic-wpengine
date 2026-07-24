//! `update()` handlers for the Layout Themes page: theme selection, the
//! live element editor, importing dropped `.toml` files, and creating a
//! new theme from the template.
use super::*;

impl SettingsApp {
    pub(super) fn on_theme_selected(&mut self, idx: usize) -> Task<cosmic::Action<Message>> {
        self.switch_edit_theme(self.available_themes.get(idx).cloned());
        Task::none()
    }

    pub(super) fn on_theme_element_selected(
        &mut self,
        idx: usize,
    ) -> Task<cosmic::Action<Message>> {
        self.theme_element = idx;
        Task::none()
    }

    pub(super) fn on_theme_edit(&mut self, edit: ThemeEditMsg) -> Task<cosmic::Action<Message>> {
        let element = self.theme_element;
        if let Some(layout) = &mut self.edit_theme {
            apply_theme_edit(layout, element, edit);
            return self.schedule_theme_save();
        }
        Task::none()
    }

    pub(super) fn on_reset_theme_element(&mut self) -> Task<cosmic::Action<Message>> {
        let element = self.theme_element;
        if let (Some(style), Some(layout)) = (&self.selected_theme, &mut self.edit_theme) {
            reset_theme_element(layout, style, element);
            return self.schedule_theme_save();
        }
        Task::none()
    }

    pub(super) fn on_debounced_theme_save(
        &mut self,
        generation: u64,
    ) -> Task<cosmic::Action<Message>> {
        if generation == self.theme_save_generation {
            self.write_theme_file();
        }
        Task::none()
    }

    pub(super) fn on_theme_files_dropped(
        &mut self,
        files: Option<library::DroppedFiles>,
    ) -> Task<cosmic::Action<Message>> {
        let paths = files.map(|f| f.0).unwrap_or_default();
        let mut imported = 0;
        for path in paths
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        {
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            if let Err(e) = toml::from_str::<config::ThemeLayout>(&text) {
                self.status_msg = fl!("status-invalid-theme", error = e.to_string());
                continue;
            }
            let Some(name) = path.file_name() else {
                continue;
            };
            let dir = config::Config::config_dir().join("shaders");
            let _ = std::fs::create_dir_all(&dir);
            if std::fs::write(dir.join(name), text).is_ok() {
                imported += 1;
            }
        }
        if imported > 0 {
            self.available_themes = load_themes();
            self.status_msg = fl!("status-imported-themes", imported = (imported as i64));
        } else if self.status_msg == fl!("status-ready") {
            self.status_msg = fl!("status-nothing-imported-themes");
        }
        Task::none()
    }

    pub(super) fn on_apply_theme(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(theme) = &self.selected_theme {
            self.wp_config.audio.style = theme.clone();
            if let Err(e) = self.wp_config.save() {
                self.status_msg = fl!("status-error-applying-theme", error = e.to_string());
            } else {
                self.status_msg = fl!("status-applied-theme", theme = theme.as_str());
            }
        } else {
            self.status_msg = fl!("status-select-theme-to-apply");
        }
        Task::none()
    }

    pub(super) fn on_new_theme_name_changed(
        &mut self,
        name: String,
    ) -> Task<cosmic::Action<Message>> {
        self.new_theme_name = name;
        Task::none()
    }

    pub(super) fn on_create_theme(&mut self) -> Task<cosmic::Action<Message>> {
        let name = self.new_theme_name.trim().trim_end_matches(".toml");
        // Checked on the fully-trimmed name, not the raw string:
        // the view layer already blocks submitting a whitespace-
        // only name, but this handler shouldn't trust that on its
        // own. Two distinct inputs can slip past a raw
        // `new_theme_name.is_empty()` check yet still trim down to
        // an empty stem here - whitespace-only, and a name that's
        // exactly `.toml` (stripped entirely by
        // `trim_end_matches(".toml")`) - either would otherwise
        // create `shaders/.toml`.
        if !name.is_empty() {
            let file_name = format!("shaders/{}.toml", name);

            if !is_safe_path(&file_name) {
                self.status_msg = fl!("status-blocked-unsafe-theme-name", name = name);
                return Task::none();
            }

            let path = config::Config::config_dir().join(&file_name);

            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);

            match options.open(&path) {
                Ok(mut file) => {
                    use std::io::Write;
                    let _ = file.write_all(THEME_TEMPLATE.as_bytes());
                    self.available_themes = load_themes();
                    self.switch_edit_theme(Some(name.to_string()));
                    self.status_msg = fl!("status-created-theme", file_name = file_name);
                    self.new_theme_name.clear();
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    self.status_msg = fl!("status-theme-already-exists", name = name);
                }
                Err(e) => {
                    self.status_msg = fl!("status-error-creating-theme", error = e.to_string());
                }
            }
        }
        Task::none()
    }
}
