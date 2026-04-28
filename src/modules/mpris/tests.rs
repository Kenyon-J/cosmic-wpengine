#![cfg(test)]

use super::*;
use std::path::Path;

/// Tests the `is_safe_path` function to ensure it properly blocks path traversal and arbitrary file reads.
/// This prevents untrusted MPRIS metadata (like album art paths) from leaking sensitive local files.
#[test]
fn test_is_safe_path() {
    use std::fs;

    // To avoid parallel test issues, test against the real home directory or handle it properly.
    // Actually, just save and restore HOME or use a mutex if we really need to set it.
    let old_home = std::env::var("HOME").ok();

    // Create actual temporary directories for test files, as canonicalize requires them
    let temp_dir = std::env::temp_dir();
    let test_home = Path::new("/var/tmp/testuser");
    let test_music = test_home.join("Music");
    let test_cache = test_home.join(".cache");

    let _ = fs::create_dir_all(&test_music);
    let _ = fs::create_dir_all(&test_cache);
    let _ = fs::create_dir_all(temp_dir.join("art_dir"));

    // Create dummy files so they can be canonicalized
    let tmp_art_buf = temp_dir.join("art.png");
    let tmp_art = tmp_art_buf.as_path();
    let _ = fs::write(tmp_art, "test");

    let music_art = test_music.join("cover.png");
    let _ = fs::write(&music_art, "test");

    let cache_art = test_cache.join("art.jpg");
    let _ = fs::write(&cache_art, "test");

    // Non-safe file in home dir
    let document = test_home.join("document.pdf");
    let _ = fs::write(&document, "test");

    std::env::set_var("HOME", test_home.to_str().unwrap());

    // Valid absolute paths in safe locations
    assert!(MprisWatcher::is_safe_path(tmp_art));
    assert!(MprisWatcher::is_safe_path(&music_art));
    assert!(MprisWatcher::is_safe_path(&cache_art));

    // Path traversal attempts
    // These will fail both because they use .. and because the resolved paths likely don't exist
    assert!(!MprisWatcher::is_safe_path(Path::new("/etc/passwd")));
    let trav_path = temp_dir.join("art_dir").join("../../var/log");
    assert!(!MprisWatcher::is_safe_path(&trav_path));

    // Blocked home directory access attempts
    assert!(!MprisWatcher::is_safe_path(&document));

    // Relative paths
    assert!(!MprisWatcher::is_safe_path(Path::new("art.png")));
    assert!(!MprisWatcher::is_safe_path(Path::new("./art.png")));

    // Test symlink bypass
    let sym_buf = temp_dir.join("symlink_to_doc");
    let symlink_path = sym_buf.as_path();
    let _ = std::os::unix::fs::symlink(&document, symlink_path);
    // Because it resolves to document.pdf (not in Music or .cache), it should be blocked
    assert!(!MprisWatcher::is_safe_path(symlink_path));

    // Cleanup
    let _ = fs::remove_file(tmp_art);
    let _ = fs::remove_file(symlink_path);
    let _ = fs::remove_dir_all(test_home);

    if let Some(old) = old_home {
        std::env::set_var("HOME", old);
    } else {
        std::env::remove_var("HOME");
    }
}
