use ksni::menu::StandardItem;
use tokio::sync::mpsc;

pub struct WallpaperTray {
    gui_process: Option<std::process::Child>,
    shutdown_tx: mpsc::Sender<()>,
}

impl WallpaperTray {
    pub fn new(shutdown_tx: mpsc::Sender<()>) -> Self {
        Self {
            gui_process: None,
            shutdown_tx,
        }
    }

    fn launch_gui(&mut self) {
        if let Some(child) = &mut self.gui_process {
            // If try_wait() is Ok(None), the process is still running.
            if child.try_wait().ok() == Some(None) {
                // This is a workaround to force the window to the foreground on Wayland.
                // A more advanced solution would involve D-Bus activation.
                let _ = child.kill();
                let _ = child.wait();
            }
        }

        // Find the GUI binary in the same folder as this running executable
        let gui_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .map(|p| p.join("cosmic-wallpaper-gui"))
            .filter(|p| p.exists());

        if let Some(path) = gui_path {
            match std::process::Command::new(&path).spawn() {
                Ok(child) => self.gui_process = Some(child),
                Err(e) => tracing::error!("Failed to launch GUI: {}", e),
            }
        } else {
            tracing::error!(
                "Could not find cosmic-wallpaper-gui binary alongside the engine executable."
            );
        }
    }
}

impl ksni::Tray for WallpaperTray {
    fn icon_name(&self) -> String {
        "preferences-desktop-wallpaper".into()
    }

    fn title(&self) -> String {
        "COSMIC Wallpaper".into()
    }

    // This is triggered when the user left-clicks the tray icon
    fn activate(&mut self, _x: i32, _y: i32) {
        self.launch_gui();
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Open Settings...".into(),
                activate: Box::new(|this: &mut Self| {
                    this.launch_gui();
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Quit Engine".into(),
                activate: Box::new(|this: &mut Self| {
                    // Send a shutdown signal instead of exiting directly.
                    if let Err(e) = this.shutdown_tx.blocking_send(()) {
                        tracing::error!("Failed to send shutdown signal: {}", e);
                        // Fallback to a hard exit if the channel is broken.
                        std::process::exit(1);
                    }
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
