//! Shared tracing setup for both binaries: everything still goes to
//! stdout exactly as before (so `journalctl` and a terminal keep working
//! unchanged), plus a daily-rotating file under the config directory.
//!
//! The file sink exists so the GUI can read recent log lines regardless of
//! how the engine was launched. `journalctl --user -u <unit>` only covers
//! the engine's systemd autostart path; a manually-run engine, and the GUI
//! itself (which previously initialised no subscriber at all - its
//! `tracing::warn!` calls went nowhere), have no other durable record.

use std::path::{Path, PathBuf};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// How long a rotated-out log file is kept before `init` prunes it. Bounds
/// the otherwise-unbounded growth of daily files under a wallpaper engine
/// that can run for months between restarts.
const MAX_LOG_AGE: std::time::Duration = std::time::Duration::from_secs(14 * 24 * 60 * 60);

/// Scanning further back than this into a (possibly never-rotated, e.g. if
/// the clock is wrong) log file to find recent lines isn't worth the read:
/// comfortably more than `tail_lines`/`tail_error_lines` could ever need at
/// realistic line lengths.
const MAX_SCAN_BYTES: usize = 512 * 1024;

pub fn log_dir() -> PathBuf {
    crate::modules::config::Config::config_dir().join("logs")
}

/// Initialises the global tracing subscriber for one binary. `component`
/// ("engine" or "gui") keeps the two binaries' logs in separate files so
/// concurrent writers don't interleave mid-line.
///
/// Must be called at most once per process (a second call would panic on
/// the already-installed global subscriber), and as early as possible so
/// no log lines are lost to the pre-init no-op default.
pub fn init(component: &str) {
    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    prune_old_logs(&dir);

    let file_appender = tracing_appender::rolling::daily(&dir, format!("{component}.log"));
    // Non-blocking: the writer hands lines to a background thread instead
    // of blocking the async runtime on file I/O. The guard flushes on drop,
    // so it must outlive every tracing call - both `main`s run for the
    // process's entire life and have no single point to hold a local guard
    // across (main.rs's work happens inside a `run_until` future; the GUI's
    // `main` hands control to iced's own event loop), so a one-time,
    // one-per-process leak is simpler than threading it through either.
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    Box::leak(Box::new(guard));

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer())
        .with(fmt::layer().with_ansi(false).with_writer(non_blocking))
        .init();
}

fn prune_old_logs(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        let is_old = entry
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|modified| {
                now.duration_since(modified)
                    .is_ok_and(|age| age > MAX_LOG_AGE)
            });
        if is_old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// The most recently modified log file belonging to `component`, if any.
/// `rolling::daily` names files `{component}.log.YYYY-MM-DD`; picking by
/// mtime (rather than reconstructing today's date) is correct even across
/// a rotation boundary or a clock that's off.
fn current_log_file(component: &str) -> Option<PathBuf> {
    let prefix = format!("{component}.log");
    std::fs::read_dir(log_dir())
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

/// The last chunk of `component`'s current log file, as complete lines in
/// chronological order. Bounded to `MAX_SCAN_BYTES` from the end so an
/// old, unrotated file can't be read in full just to keep its tail.
fn scan_window(component: &str) -> Vec<String> {
    let Some(path) = current_log_file(component) else {
        return Vec::new();
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    let start = bytes.len().saturating_sub(MAX_SCAN_BYTES);
    // The scan window can start mid-UTF8-sequence; from_utf8_lossy repairs
    // that with replacement characters rather than panicking or losing the
    // rest of the file.
    String::from_utf8_lossy(&bytes[start..])
        .lines()
        .map(str::to_string)
        .collect()
}

/// Last `max_lines` lines of `component`'s log, any level, oldest first.
pub fn tail_lines(component: &str, max_lines: usize) -> Vec<String> {
    let lines = scan_window(component);
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].to_vec()
}

/// Last `max_lines` `ERROR`/`WARN` lines of `component`'s log, oldest
/// first - the excerpt a bug report actually wants, without the routine
/// INFO noise drowning it out.
pub fn tail_error_lines(component: &str, max_lines: usize) -> Vec<String> {
    let matches: Vec<String> = scan_window(component)
        .into_iter()
        .filter(|l| l.contains("ERROR") || l.contains("WARN"))
        .collect();
    let start = matches.len().saturating_sub(max_lines);
    matches[start..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::utils::test_support::ENV_MUTEX;

    fn with_temp_config_dir<R>(f: impl FnOnce() -> R) -> R {
        let _guard = ENV_MUTEX.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let prev = std::env::var("XDG_CONFIG_HOME").ok();
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        let result = f();
        match prev {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        result
    }

    fn write_log(dir: &Path, name: &str, contents: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), contents).unwrap();
    }

    #[test]
    fn tail_lines_reads_the_most_recently_modified_matching_file() {
        with_temp_config_dir(|| {
            let dir = log_dir();
            write_log(&dir, "engine.log.2026-07-01", "stale line\n");
            // Back-date the stale file so mtime, not filename, decides.
            let stale = dir.join("engine.log.2026-07-01");
            let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
            let _ = filetime_touch(&stale, old);

            write_log(
                &dir,
                "engine.log.2026-07-20",
                "line one\nline two\nline three\n",
            );
            write_log(&dir, "gui.log.2026-07-20", "unrelated gui line\n");

            let lines = tail_lines("engine", 2);
            assert_eq!(lines, vec!["line two", "line three"]);
        });
    }

    #[test]
    fn tail_lines_returns_empty_when_no_log_exists() {
        with_temp_config_dir(|| {
            assert!(tail_lines("engine", 10).is_empty());
        });
    }

    #[test]
    fn tail_error_lines_filters_out_info_noise() {
        with_temp_config_dir(|| {
            let dir = log_dir();
            write_log(
                &dir,
                "engine.log.2026-07-20",
                "2026-07-20T00:00:00Z  INFO cosmic_wallpaper: Starting up\n\
                 2026-07-20T00:00:01Z  WARN cosmic_wallpaper::modules::audio: no PipeWire stream\n\
                 2026-07-20T00:00:02Z  INFO cosmic_wallpaper: All subsystems started\n\
                 2026-07-20T00:00:03Z ERROR cosmic_wallpaper::modules::mpris: fetch failed\n",
            );

            let errors = tail_error_lines("engine", 10);
            assert_eq!(errors.len(), 2);
            assert!(errors[0].contains("no PipeWire stream"));
            assert!(errors[1].contains("fetch failed"));
        });
    }

    #[test]
    fn tail_error_lines_caps_to_max_lines_keeping_the_most_recent() {
        with_temp_config_dir(|| {
            let dir = log_dir();
            let mut contents = String::new();
            for i in 0..5 {
                contents.push_str(&format!("2026-07-20T00:00:0{i}Z ERROR test: error {i}\n"));
            }
            write_log(&dir, "engine.log.2026-07-20", &contents);

            let errors = tail_error_lines("engine", 2);
            assert_eq!(errors.len(), 2);
            assert!(errors[0].contains("error 3"));
            assert!(errors[1].contains("error 4"));
        });
    }

    #[test]
    fn prune_old_logs_removes_only_stale_files() {
        with_temp_config_dir(|| {
            let dir = log_dir();
            std::fs::create_dir_all(&dir).unwrap();
            write_log(&dir, "engine.log.2020-01-01", "ancient\n");
            let ancient = dir.join("engine.log.2020-01-01");
            let long_ago =
                std::time::SystemTime::now() - MAX_LOG_AGE - std::time::Duration::from_secs(60);
            filetime_touch(&ancient, long_ago).unwrap();

            write_log(&dir, "engine.log.2026-07-20", "fresh\n");

            prune_old_logs(&dir);

            assert!(
                !ancient.exists(),
                "log older than MAX_LOG_AGE must be pruned"
            );
            assert!(
                dir.join("engine.log.2026-07-20").exists(),
                "fresh log must survive"
            );
        });
    }

    /// std::fs has no portable mtime setter; go through the filetime crate's
    /// minimal surface via libc-free syscalls isn't available here, so shell
    /// out to `touch -d` instead - test-only, and `touch` is universally
    /// present on the Linux CI/dev environments this suite runs in.
    fn filetime_touch(path: &Path, time: std::time::SystemTime) -> std::io::Result<()> {
        let secs = time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let status = std::process::Command::new("touch")
            .arg("-d")
            .arg(format!("@{secs}"))
            .arg(path)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("touch failed"))
        }
    }
}
