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
    let test_home = "/var/tmp/testuser";
    std::env::set_var("HOME", test_home);

    // Create necessary dummy directories and files for canonicalize to work
    // We cannot create /run/user/1000 as an unprivileged user in the test container,
    // so we will test with /tmp and the user's home directory.
    let dirs_to_create = vec![
        "/tmp".to_string(),
        format!("{}/Music", test_home),
        format!("{}/.cache", test_home),
        format!("{}/.ssh", test_home),
    ];
    for dir in &dirs_to_create {
        let _ = std::fs::create_dir_all(dir);
    }

    let files_to_create = vec![
        "/tmp/art.png".to_string(),
        format!("{}/Music/cover.png", test_home),
        format!("{}/.cache/art.jpg", test_home),
        format!("{}/.ssh/id_rsa", test_home),
        format!("{}/document.pdf", test_home),
        "art.png".to_string(),
    ];
    for file in &files_to_create {
        let _ = std::fs::write(file, "test data");
    }

    // Valid absolute paths in safe locations
    assert!(MprisWatcher::is_safe_path(Path::new("/tmp/art.png")));
    assert!(MprisWatcher::is_safe_path(Path::new(&format!(
        "{}/Music/cover.png",
        test_home
    ))));
    assert!(MprisWatcher::is_safe_path(Path::new(&format!(
        "{}/.cache/art.jpg",
        test_home
    ))));

    // Path traversal attempts
    // Note: Canonicalize will fail if the target doesn't exist, which naturally
    // blocks many traversal attempts, but we should test some that might resolve.
    assert!(!MprisWatcher::is_safe_path(Path::new("/tmp/../etc/passwd")));

    // Blocked home directory access attempts
    assert!(!MprisWatcher::is_safe_path(Path::new(&format!(
        "{}/.ssh/id_rsa",
        test_home
    ))));
    assert!(!MprisWatcher::is_safe_path(Path::new(&format!(
        "{}/document.pdf",
        test_home
    ))));

    // Relative paths
    assert!(!MprisWatcher::is_safe_path(Path::new("art.png")));
    assert!(!MprisWatcher::is_safe_path(Path::new("./art.png")));

    if let Some(old) = old_home {
        std::env::set_var("HOME", old);
    } else {
        std::env::remove_var("HOME");
    }

    // Cleanup
    for file in &files_to_create {
        let _ = std::fs::remove_file(file);
    }
    for dir in dirs_to_create.iter().rev() {
        let _ = std::fs::remove_dir(dir);
    }
}
