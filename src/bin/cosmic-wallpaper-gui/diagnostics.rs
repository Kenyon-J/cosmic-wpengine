use crate::SettingsApp;
use cosmic_wallpaper::modules::utils::resolve_binary;

/// Path to the engine binary: the GUI's sibling first (both binaries
/// install side by side - ~/.local/bin, /usr/bin, and the .deb all do
/// this; the updater relies on the same layout), falling back to the
/// trusted system paths for split installs.
pub(crate) fn engine_binary_path() -> Option<std::path::PathBuf> {
    if let Ok(gui) = std::env::current_exe() {
        if let Some(dir) = gui.parent() {
            let sibling = dir.join("cosmic-wallpaper");
            if sibling.exists() {
                return Some(sibling);
            }
        }
    }
    resolve_binary("cosmic-wallpaper")
}

/// systemd unit the xdg-autostart generator creates for the engine's
/// autostart entry. Its failure state is the only surviving record of a
/// binary that died before main() at login - e.g. exit 127 from the dynamic
/// linker after a system update changed a library soname - which otherwise
/// fails completely silently (found 2026-07-19: an ffmpeg major bump left
/// the engine dead all session with no visible error anywhere).
const ENGINE_AUTOSTART_UNIT: &str = "app-io.github.kenyon_j.cosmic_wpengine@autostart.service";

/// Asks systemd whether the engine's login autostart failed, translating
/// the exit status into an actionable message. None when the unit is fine,
/// unknown, or systemctl is unavailable.
pub(crate) fn engine_autostart_failure() -> Option<String> {
    let systemctl = resolve_binary("systemctl")?;
    let output = std::process::Command::new(systemctl)
        .args([
            "--user",
            "show",
            ENGINE_AUTOSTART_UNIT,
            "--property=ActiveState,ExecMainStatus",
        ])
        .output()
        .ok()?;
    parse_unit_failure(&String::from_utf8_lossy(&output.stdout))
}

/// Pure parse of `systemctl show --property=ActiveState,ExecMainStatus`
/// output, split out of [`engine_autostart_failure`] for testing.
pub(crate) fn parse_unit_failure(show_output: &str) -> Option<String> {
    let prop = |name: &str| {
        show_output.lines().find_map(|line| {
            line.strip_prefix(name)
                .and_then(|rest| rest.strip_prefix('='))
        })
    };
    if prop("ActiveState") != Some("failed") {
        return None;
    }
    Some(match prop("ExecMainStatus") {
        // 127 is the shell/exec convention for "could not run at all" -
        // for a binary that exists, that means the dynamic linker bailed.
        Some("127") => "The engine failed to start at login: its binary could not load \
             (exit 127, usually missing libraries after a system update). \
             Rebuild or reinstall it, then press Start."
            .to_string(),
        Some(code) if !code.is_empty() => format!(
            "The engine failed to start at login (exit {code}). \
             See: journalctl --user -u {ENGINE_AUTOSTART_UNIT}"
        ),
        _ => format!(
            "The engine failed to start at login. \
             See: journalctl --user -u {ENGINE_AUTOSTART_UNIT}"
        ),
    })
}

/// The most informative line of the engine's captured stderr: the dynamic
/// linker's message when present (the exit-127 case this capture exists
/// for), else the first non-empty line.
pub(crate) fn stderr_headline(stderr: &str) -> String {
    stderr
        .lines()
        .find(|l| l.contains("error while loading shared libraries"))
        .or_else(|| stderr.lines().find(|l| !l.trim().is_empty()))
        .unwrap_or("")
        .trim()
        .to_string()
}

/// `PRETTY_NAME` out of `/etc/os-release` - the one line most useful for
/// telling apart "which Linux" in a bug report, without parsing the whole
/// (semi-standardised, but not worth a crate for one field) file format.
fn distro_pretty_name() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| "Unknown Linux distribution".to_string())
}

/// Full plain-text snapshot for the "Copy Diagnostics" button: version,
/// distro, engine status, and each binary's recent log tail (including GPU
/// adapter selection, logged once at engine startup - see
/// renderer::core::init).
pub(crate) fn build_diagnostics_text(app: &SettingsApp) -> String {
    use cosmic_wallpaper::modules::logging;
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(out, "cosmic-wallpaper {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(out, "OS: {}", distro_pretty_name());
    let _ = writeln!(
        out,
        "Engine: {}",
        match (app.engine_pid, &app.engine_failure) {
            (Some(pid), _) => format!("running (pid {pid})"),
            (None, Some(failure)) => failure.clone(),
            (None, None) => "not running".to_string(),
        }
    );
    let _ = writeln!(out, "Mode: {:?}", app.wp_config.mode);
    let _ = writeln!(out, "Theme: {}", app.wp_config.audio.style);

    for component in ["engine", "gui"] {
        let lines = logging::tail_lines(component, 40);
        let _ = writeln!(
            out,
            "\n--- {component} log (last {} lines) ---",
            lines.len()
        );
        if lines.is_empty() {
            let _ = writeln!(out, "(no log file yet)");
        }
        for line in lines {
            let _ = writeln!(out, "{line}");
        }
    }

    out
}

/// Compact excerpt for the prefilled GitHub issue body: version, distro,
/// and only the ERROR/WARN lines (not the full diagnostics dump) - long
/// enough to be useful, short enough to stay a sane URL length.
pub(crate) fn build_issue_body() -> String {
    use cosmic_wallpaper::modules::logging;
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(out, "**cosmic-wallpaper {}**", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(out, "OS: {}", distro_pretty_name());
    let _ = writeln!(
        out,
        "\n<!-- What were you doing when this happened? -->\n\n"
    );

    let mut errors = logging::tail_error_lines("engine", 12);
    errors.extend(logging::tail_error_lines("gui", 5));
    if !errors.is_empty() {
        let _ = writeln!(out, "<details><summary>Recent errors/warnings</summary>\n");
        let _ = writeln!(out, "```");
        for line in errors {
            let _ = writeln!(out, "{line}");
        }
        let _ = writeln!(out, "```\n</details>");
    }

    out
}

/// PID of a running wallpaper engine, found by cmdline (comm truncates to
/// 15 chars, which cannot distinguish the engine from this GUI).
pub(crate) fn find_engine_pid() -> Option<u32> {
    let entries = std::fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(cmdline) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let arg0 = cmdline.split(|b| *b == 0).next().unwrap_or_default();
        let arg0 = String::from_utf8_lossy(arg0);
        if std::path::Path::new(arg0.as_ref())
            .file_name()
            .is_some_and(|n| n == "cosmic-wallpaper")
        {
            return Some(pid);
        }
    }
    None
}

fn autostart_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(home).join(".config/autostart")
}

/// The canonical autostart entry - the same file name the .deb installs, so
/// the toggle and package-managed autostart agree on one file.
fn autostart_path() -> std::path::PathBuf {
    autostart_dir().join("io.github.kenyon_j.cosmic_wpengine.desktop")
}

/// A stale name this toggle wrote before it matched the packaged entry;
/// removed on disable so old installs can't end up starting two engines.
fn legacy_autostart_path() -> std::path::PathBuf {
    autostart_dir().join("cosmic-wallpaper.desktop")
}

pub(crate) fn autostart_enabled() -> bool {
    autostart_path().exists() || legacy_autostart_path().exists()
}

pub(crate) fn set_autostart(enable: bool) {
    // If we are running inside a Flatpak sandbox, use the XDG Background Portal
    if std::path::Path::new("/.flatpak-info").exists() {
        let enable_str = if enable { "true" } else { "false" };
        // Execute a D-Bus call to the portal using busctl (standard in Freedesktop runtimes)
        if let Some(busctl) = resolve_binary("busctl") {
            let _ = std::process::Command::new(busctl)
                .args([
                    "--user",
                    "call",
                    "org.freedesktop.portal.Desktop",
                    "/org/freedesktop/portal/desktop",
                    "org.freedesktop.portal.Background",
                    "RequestBackground",
                    "sa{sv}",
                    "", // parent_window
                    "1",
                    "autostart",
                    "b",
                    enable_str,
                ])
                .output();
        } else {
            tracing::warn!("Failed to set autostart: busctl not found in trusted PATH");
        }
        return;
    }

    let path = autostart_path();
    // Whatever the new state, the pre-1.2 file name must go: leaving it
    // alongside the canonical entry would start two engines at login.
    let _ = std::fs::remove_file(legacy_autostart_path());
    if enable {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Absolute Exec: the session's autostart environment does not
        // reliably include ~/.local/bin in PATH.
        let exec = engine_binary_path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "cosmic-wallpaper".to_string());
        let _ = std::fs::write(
            &path,
            format!(
                r#"[Desktop Entry]
Type=Application
Name=COSMIC Wallpaper Engine
Comment=Live wallpaper engine for the COSMIC desktop
Exec={exec}
Terminal=false
StartupNotify=false
X-GNOME-Autostart-enabled=true
"#
            ),
        );
    } else {
        let _ = std::fs::remove_file(path);
    }
}
