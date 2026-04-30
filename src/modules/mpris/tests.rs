#![cfg(test)]

use super::*;
use std::fs;
use std::path::Path;

/// Tests the `is_safe_path` function to ensure it properly blocks path traversal and arbitrary file reads.
/// This prevents untrusted MPRIS metadata (like album art paths) from leaking sensitive local files.
#[test]
fn test_is_safe_path() {
    let old_home = std::env::var("HOME").ok();

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
    let ssh_dir = home_path.join(".ssh");

    fs::create_dir_all(&music_dir).unwrap();
    fs::create_dir_all(&cache_dir).unwrap();
    fs::create_dir_all(&ssh_dir).unwrap();

    // Create test files to allow canonicalize to succeed
    let art1 = music_dir.join("cover.png");
    let art2 = cache_dir.join("art.jpg");
    let rsa_key = ssh_dir.join("id_rsa");
    let doc = home_path.join("document.pdf");

    fs::write(&art1, "").unwrap();
    fs::write(&art2, "").unwrap();
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
    assert!(MprisWatcher::is_safe_path(&tmp_art));
    if let Some(ref p) = run_user_art {
        assert!(MprisWatcher::is_safe_path(p));
    }
    assert!(MprisWatcher::is_safe_path(&art1));
    assert!(MprisWatcher::is_safe_path(&art2));

    // Path traversal attempts
    let fake_passwd = home_path.join("passwd");
    fs::write(&fake_passwd, "").unwrap();
    assert!(!MprisWatcher::is_safe_path(
        &tmp_test_dir.path().join("../etc/passwd")
    ));
    assert!(!MprisWatcher::is_safe_path(Path::new(
        "/run/user/../../var/log"
    )));

    // Blocked home directory access attempts
    assert!(!MprisWatcher::is_safe_path(&rsa_key));
    assert!(!MprisWatcher::is_safe_path(&doc));

    // Relative paths
    assert!(!MprisWatcher::is_safe_path(Path::new("art.png")));
    assert!(!MprisWatcher::is_safe_path(Path::new("./art.png")));

    // Symlink bypass attempt
    let symlink_path = tmp_test_dir.path().join("symlink_to_passwd.png");
    // Create a symlink pointing to an unsafe location (e.g. the fake passwd file or another file outside whitelist)
    let fake_unsafe = home_path.join("unsafe.txt");
    fs::write(&fake_unsafe, "").unwrap();
    std::os::unix::fs::symlink(&fake_unsafe, &symlink_path).unwrap();

    // The symlink is inside /tmp, but it points to an unsafe location.
    // It should be rejected.
    assert!(!MprisWatcher::is_safe_path(&symlink_path));

    if let Some(old) = old_home {
        std::env::set_var("HOME", old);
    } else {
        std::env::remove_var("HOME");
    }
}
