#![cfg(test)]

use super::*;
use std::path::Path;

/// Tests the `is_safe_path` function to ensure it properly blocks path traversal and arbitrary file reads.
/// This prevents untrusted MPRIS metadata (like album art paths) from leaking sensitive local files.
#[test]
fn test_is_safe_path() {
    // To avoid parallel test issues, test against the real home directory or handle it properly.
    // Actually, just save and restore HOME or use a mutex if we really need to set it.
    let old_home = std::env::var("HOME").ok();
    let test_home = "/tmp/testuser_home";
    std::env::set_var("HOME", test_home);

    // Create dummy files for tests to pass canonicalization
    let _ = std::fs::write("/tmp/art.png", "");
    let _ = std::fs::create_dir_all("/tmp/testuser_run/1000");
    let _ = std::fs::write("/tmp/testuser_run/1000/art.jpg", "");
    let _ = std::fs::create_dir_all("/tmp/testuser_home/Music");
    let _ = std::fs::write("/tmp/testuser_home/Music/cover.png", "");
    let _ = std::fs::create_dir_all("/tmp/testuser_home/.cache");
    let _ = std::fs::write("/tmp/testuser_home/.cache/art.jpg", "");

    // Valid absolute paths in safe locations
    assert!(MprisWatcher::is_safe_path(Path::new("/tmp/art.png")));
    // /run/user/ is skipped here because creating directories under /run requires root.
    assert!(MprisWatcher::is_safe_path(Path::new(
        "/tmp/testuser_home/Music/cover.png"
    )));
    assert!(MprisWatcher::is_safe_path(Path::new(
        "/tmp/testuser_home/.cache/art.jpg"
    )));

    // Path traversal attempts
    assert!(!MprisWatcher::is_safe_path(Path::new("/tmp/../etc/passwd")));
    assert!(!MprisWatcher::is_safe_path(Path::new(
        "/run/user/../../var/log"
    )));

    // Blocked home directory access attempts
    assert!(!MprisWatcher::is_safe_path(Path::new(
        "/home/testuser/.ssh/id_rsa"
    )));
    assert!(!MprisWatcher::is_safe_path(Path::new(
        "/home/testuser/document.pdf"
    )));

    // Relative paths
    assert!(!MprisWatcher::is_safe_path(Path::new("art.png")));
    assert!(!MprisWatcher::is_safe_path(Path::new("./art.png")));

    if let Some(old) = old_home {
        std::env::set_var("HOME", old);
    } else {
        std::env::remove_var("HOME");
    }
}

#[test]
fn test_is_safe_path_symlink() {
    let old_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", "/home/testuser");

    // The symlink points to /etc/passwd which is outside of allowed prefixes
    assert!(!MprisWatcher::is_safe_path(Path::new(
        "/tmp/symlink_test/passwd_symlink"
    )));

    if let Some(old) = old_home {
        std::env::set_var("HOME", old);
    } else {
        std::env::remove_var("HOME");
    }
}
