#![cfg(test)]

use super::*;
use crate::modules::utils::test_support::ENV_MUTEX;
use std::fs;
use std::path::Path;

/// Tests the `resolve_safe_path` function to ensure it properly blocks path traversal and arbitrary file reads.
/// This prevents untrusted MPRIS metadata (like album art paths) from leaking sensitive local files.
///
/// Mutates the process-global HOME env var, so it must hold the shared
/// ENV_MUTEX (see `config::tests`) to avoid racing other tests that read or
/// mutate HOME/XDG_CONFIG_HOME concurrently.
#[test]
fn test_resolve_safe_path() {
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let old_home = std::env::var("HOME").ok();

    let result = std::panic::catch_unwind(run_test_resolve_safe_path);

    if let Some(old) = old_home {
        std::env::set_var("HOME", old);
    } else {
        std::env::remove_var("HOME");
    }

    if let Err(err) = result {
        std::panic::resume_unwind(err);
    }
}

/// Restores HOME even if an assertion panics, so a failure here can't leave
/// HOME pointing at a since-deleted tempdir for whichever test runs next.
fn run_test_resolve_safe_path() {
    // Create a mock home directory structure outside of /tmp and /run/user
    // to prevent false-positives since /tmp is allowed by default.
    // In many build environments, tempdir() creates inside /tmp.
    // Instead we use a path relative to the test runner or a custom directory.
    let base_dir = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let home_dir = tempfile::Builder::new()
        .prefix("mpris_mock_home")
        .tempdir_in(base_dir)
        .unwrap();
    let home_path = home_dir.path();
    std::env::set_var("HOME", home_path);

    let music_dir = home_path.join("Music");
    let cache_dir = home_path.join(".cache");
    let share_dir = home_path.join(".local/share");
    let ssh_dir = home_path.join(".ssh");

    fs::create_dir_all(&music_dir).unwrap();
    fs::create_dir_all(&cache_dir).unwrap();
    fs::create_dir_all(&share_dir).unwrap();
    fs::create_dir_all(&ssh_dir).unwrap();

    // Create test files to allow canonicalize to succeed
    let art1 = music_dir.join("cover.png");
    let art2 = cache_dir.join("art.jpg");
    let art3 = share_dir.join("player/art.png");
    let rsa_key = ssh_dir.join("id_rsa");
    let doc = home_path.join("document.pdf");

    fs::create_dir_all(art3.parent().unwrap()).unwrap();
    fs::write(&art1, "").unwrap();
    fs::write(&art2, "").unwrap();
    fs::write(&art3, "").unwrap();
    fs::write(&rsa_key, "").unwrap();
    fs::write(&doc, "").unwrap();

    // For /tmp and /run/user, we create actual files there for testing since they usually exist.
    // However, to be robust, we'll create unique temp directories inside /tmp.
    let tmp_test_dir = tempfile::Builder::new()
        .prefix("mpris_test_tmp")
        .tempdir_in("/tmp")
        .unwrap();
    let tmp_art = tmp_test_dir.path().join("art.png");
    fs::write(&tmp_art, "").unwrap();

    let run_user_test_dir = match std::fs::create_dir_all("/run/user/1000/mpris_test") {
        Ok(_) => Some(Path::new("/run/user/1000/mpris_test").to_path_buf()),
        Err(_) => None, // /run/user/1000 might not exist on the test machine
    };

    let mut run_user_art = None;
    if let Some(ref dir) = run_user_test_dir {
        let path = dir.join("art.jpg");
        if fs::write(&path, "").is_ok() {
            run_user_art = Some(path);
        }
    }

    // Valid absolute paths in safe locations
    assert!(MprisWatcher::resolve_safe_path(&tmp_art).is_some());
    if let Some(ref p) = run_user_art {
        assert!(MprisWatcher::resolve_safe_path(p).is_some());
    }
    assert!(MprisWatcher::resolve_safe_path(&art1).is_some());
    assert!(MprisWatcher::resolve_safe_path(&art2).is_some());
    assert!(MprisWatcher::resolve_safe_path(&art3).is_some());

    // Path traversal attempts
    let fake_passwd = home_path.join("passwd");
    fs::write(&fake_passwd, "").unwrap();
    assert!(MprisWatcher::resolve_safe_path(&tmp_test_dir.path().join("../etc/passwd")).is_none());
    assert!(MprisWatcher::resolve_safe_path(Path::new("/run/user/../../var/log")).is_none());

    // Blocked home directory access attempts
    assert!(MprisWatcher::resolve_safe_path(&rsa_key).is_none());
    assert!(MprisWatcher::resolve_safe_path(&doc).is_none());

    // Firefox MPRIS artwork dirs are allowed (stock and XDG layouts), but the
    // rest of the profile - where cookies and logins live - stays blocked.
    let ff_art = home_path.join(".mozilla/firefox/firefox-mpris/2368_49.png");
    let ff_xdg_art = home_path.join(".config/mozilla/firefox/firefox-mpris/1_1.png");
    let ff_cookies = home_path.join(".mozilla/firefox/profile.default/cookies.sqlite");
    fs::create_dir_all(ff_art.parent().unwrap()).unwrap();
    fs::create_dir_all(ff_xdg_art.parent().unwrap()).unwrap();
    fs::create_dir_all(ff_cookies.parent().unwrap()).unwrap();
    fs::write(&ff_art, "").unwrap();
    fs::write(&ff_xdg_art, "").unwrap();
    fs::write(&ff_cookies, "").unwrap();
    assert!(MprisWatcher::resolve_safe_path(&ff_art).is_some());
    assert!(MprisWatcher::resolve_safe_path(&ff_xdg_art).is_some());
    assert!(MprisWatcher::resolve_safe_path(&ff_cookies).is_none());

    // Relative paths
    assert!(MprisWatcher::resolve_safe_path(Path::new("art.png")).is_none());
    assert!(MprisWatcher::resolve_safe_path(Path::new("./art.png")).is_none());

    // Symlink bypass attempt
    let symlink_path = tmp_test_dir.path().join("symlink_to_passwd.png");
    // Create a symlink pointing to an unsafe location (e.g. the fake passwd file or another file outside whitelist)
    let fake_unsafe = home_path.join("unsafe.txt");
    fs::write(&fake_unsafe, "").unwrap();
    std::os::unix::fs::symlink(&fake_unsafe, &symlink_path).unwrap();

    // The symlink is inside /tmp, but it points to an unsafe location.
    // It should be rejected.
    assert!(MprisWatcher::resolve_safe_path(&symlink_path).is_none());
}

// `is_safe_ip` and its tests live in `modules::utils` now that both the
// album-art fetcher and the canvas video decoder share it.

/// Canvas fetching is opt-in: with no configured proxy URL the fetch must
/// short-circuit to `None` before any network I/O. (The pre-hardening code
/// defaulted to `http://localhost:3000`, which any local process could bind.)
#[test]
fn test_canvas_fetch_skipped_without_configured_proxy() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let client = reqwest::Client::new();
    let result = rt.block_on(MprisWatcher::fetch_spotify_canvas(
        "4uLU6hMCjMI75M1A2tKUQC",
        None,
        &client,
    ));
    assert_eq!(result, None);
}
