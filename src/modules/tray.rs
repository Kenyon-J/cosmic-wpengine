use ksni::menu::StandardItem;

pub struct WallpaperTray {
    gui_process: Option<std::process::Child>,
}

impl WallpaperTray {
    pub fn new() -> Self {
        Self { gui_process: None }
    }

    fn launch_gui(&mut self) {
        if let Some(child) = &mut self.gui_process {
            // Check if it is still running
            if let Ok(None) = child.try_wait() {
                // Kill and respawn to gracefully force Wayland to bring it to the foreground!
                let _ = child.kill();
                let _ = child.wait();
            }
        }

        // Find the GUI binary in the same folder as this running executable
        let mut gui_path = None;
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                let sibling = parent.join("cosmic-wallpaper-gui");
                if sibling.exists() {
                    gui_path = Some(sibling);
                }
            }
        }

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
                activate: Box::new(|_| {
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
