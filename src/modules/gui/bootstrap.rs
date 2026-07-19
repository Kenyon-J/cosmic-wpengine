//! First-run desktop integration for manual installs.
//!
//! Packaged installs ship the launcher entry and icons themselves (the .deb
//! under /usr, the Flatpak via its manifest), but a release-tarball install -
//! and every install the self-updater keeps alive afterwards - is just two
//! binaries in ~/.local/bin. Without this bootstrap the app never appears in
//! the desktop's app library or launcher; the tray and a terminal are the
//! only ways in. On GUI startup we install the missing pieces for exactly
//! those installs, and never touch the packaged ones.

use std::path::{Path, PathBuf};

const APP_ID: &str = "io.github.kenyon_j.cosmic_wpengine";

/// The repo's canonical desktop entry; single source of truth shared with
/// the release tarball and the Flatpak manifest. Its bare `Exec=` is
/// replaced with an absolute path at install time: launcher sessions do not
/// reliably have ~/.local/bin on PATH.
const DESKTOP_TEMPLATE: &str =
    include_str!("../../../io.github.kenyon_j.cosmic_wpengine.desktop");
const TEMPLATE_EXEC_LINE: &str = "Exec=cosmic-wallpaper-gui";

/// The shipped hicolor icon set (~0.6 MB total), embedded so a two-binary
/// install can materialise it without carrying the tarball's icons/ folder.
const ICONS: [(u32, &[u8]); 6] = [
    (
        32,
        include_bytes!(
            "../../../resources/icons/hicolor/32x32/apps/io.github.kenyon_j.cosmic_wpengine.png"
        ),
    ),
    (
        48,
        include_bytes!(
            "../../../resources/icons/hicolor/48x48/apps/io.github.kenyon_j.cosmic_wpengine.png"
        ),
    ),
    (
        64,
        include_bytes!(
            "../../../resources/icons/hicolor/64x64/apps/io.github.kenyon_j.cosmic_wpengine.png"
        ),
    ),
    (
        128,
        include_bytes!(
            "../../../resources/icons/hicolor/128x128/apps/io.github.kenyon_j.cosmic_wpengine.png"
        ),
    ),
    (
        256,
        include_bytes!(
            "../../../resources/icons/hicolor/256x256/apps/io.github.kenyon_j.cosmic_wpengine.png"
        ),
    ),
    (
        512,
        include_bytes!(
            "../../../resources/icons/hicolor/512x512/apps/io.github.kenyon_j.cosmic_wpengine.png"
        ),
    ),
];

/// Installs the launcher entry and icons for manual installs. Safe to call
/// on every startup: it no-ops once everything is in place, and it never
/// runs for installs whose package manager owns these files.
pub(crate) fn ensure_desktop_integration() {
    // The Flatpak's manifest installs the entry and icons inside the
    // sandbox, and writes into the sandboxed home would be invisible to the
    // host desktop anyway.
    if Path::new("/.flatpak-info").exists() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    // Mirrors updater::is_self_updatable: /usr means a package-managed
    // install, whose .desktop and icons belong to the package.
    if exe.starts_with("/usr") {
        return;
    }
    install(&data_dir(), &exe);
}

fn data_dir() -> PathBuf {
    std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
            PathBuf::from(home).join(".local/share")
        })
}

/// The first word of the entry's `Exec=` value, when present.
fn exec_target(desktop_contents: &str) -> Option<&str> {
    desktop_contents
        .lines()
        .find_map(|line| line.strip_prefix("Exec="))
        .and_then(|value| value.split_whitespace().next())
}

/// Core install, parameterised over the target dir and executable so tests
/// can drive it against a tempdir without touching process-global env vars.
fn install(data_dir: &Path, exe: &Path) {
    let desktop_path = data_dir
        .join("applications")
        .join(format!("{APP_ID}.desktop"));
    let desired = DESKTOP_TEMPLATE.replace(
        TEMPLATE_EXEC_LINE,
        &format!("Exec={}", exe.display()),
    );

    // Write when missing, or heal an entry whose absolute Exec no longer
    // exists (the install was moved). An entry the user pointed elsewhere -
    // or left with a bare command name - still works, so leave it alone
    // rather than clobbering their edit on every startup.
    let write_desktop = match std::fs::read_to_string(&desktop_path) {
        Err(_) => true,
        Ok(existing) => exec_target(&existing)
            .is_some_and(|target| target.starts_with('/') && !Path::new(target).exists()),
    };
    if write_desktop {
        if let Some(parent) = desktop_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&desktop_path, desired) {
            Ok(()) => tracing::info!("Installed launcher entry at {:?}", desktop_path),
            Err(e) => tracing::warn!("Could not install launcher entry: {e}"),
        }
    }

    // Icons are ours alone, so a content mismatch means the shipped artwork
    // changed - rewrite so updates propagate.
    for (size, bytes) in ICONS {
        let icon_path = data_dir
            .join(format!("icons/hicolor/{size}x{size}/apps"))
            .join(format!("{APP_ID}.png"));
        if std::fs::read(&icon_path).is_ok_and(|existing| existing == bytes) {
            continue;
        }
        if let Some(parent) = icon_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&icon_path, bytes) {
            tracing::warn!("Could not install {size}x{size} icon: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_exe(dir: &Path) -> PathBuf {
        let exe = dir.join("cosmic-wallpaper-gui");
        std::fs::write(&exe, b"stand-in").unwrap();
        exe
    }

    fn desktop_path(data_dir: &Path) -> PathBuf {
        data_dir
            .join("applications")
            .join(format!("{APP_ID}.desktop"))
    }

    /// Guards template drift: if the repo .desktop's Exec line changes
    /// shape, the replace() in install() silently stops substituting and
    /// every bootstrapped entry ships a PATH-dependent Exec again.
    #[test]
    fn template_contains_the_exec_line_install_replaces() {
        assert!(DESKTOP_TEMPLATE.contains(TEMPLATE_EXEC_LINE));
    }

    #[test]
    fn first_run_installs_entry_with_absolute_exec_and_all_icons() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_exe(tmp.path());
        install(tmp.path(), &exe);

        let entry = std::fs::read_to_string(desktop_path(tmp.path())).unwrap();
        assert_eq!(exec_target(&entry), Some(exe.to_str().unwrap()));
        assert!(entry.contains(&format!("Icon={APP_ID}")));

        for (size, bytes) in ICONS {
            let icon = tmp
                .path()
                .join(format!("icons/hicolor/{size}x{size}/apps/{APP_ID}.png"));
            assert_eq!(std::fs::read(icon).unwrap(), bytes);
        }
    }

    #[test]
    fn user_edited_entry_with_working_exec_is_preserved() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_exe(tmp.path());
        let elsewhere = tmp.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        let other = fake_exe(&elsewhere);
        let custom = format!("[Desktop Entry]\nExec={}\n", other.display());
        std::fs::create_dir_all(tmp.path().join("applications")).unwrap();
        std::fs::write(desktop_path(tmp.path()), &custom).unwrap();

        install(tmp.path(), &exe);

        assert_eq!(
            std::fs::read_to_string(desktop_path(tmp.path())).unwrap(),
            custom
        );
    }

    #[test]
    fn entry_with_dangling_absolute_exec_is_healed() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_exe(tmp.path());
        std::fs::create_dir_all(tmp.path().join("applications")).unwrap();
        std::fs::write(
            desktop_path(tmp.path()),
            "[Desktop Entry]\nExec=/nonexistent/old-location/cosmic-wallpaper-gui\n",
        )
        .unwrap();

        install(tmp.path(), &exe);

        let entry = std::fs::read_to_string(desktop_path(tmp.path())).unwrap();
        assert_eq!(exec_target(&entry), Some(exe.to_str().unwrap()));
    }

    #[test]
    fn outdated_icon_content_is_rewritten() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_exe(tmp.path());
        let icon = tmp
            .path()
            .join(format!("icons/hicolor/512x512/apps/{APP_ID}.png"));
        std::fs::create_dir_all(icon.parent().unwrap()).unwrap();
        std::fs::write(&icon, b"old artwork").unwrap();

        install(tmp.path(), &exe);

        assert_eq!(std::fs::read(&icon).unwrap(), ICONS[5].1);
    }
}
