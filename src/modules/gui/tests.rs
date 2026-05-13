#![cfg(test)]

#[test]
fn test_get_safe_path() {
    use std::fs;
    let temp_dir = tempfile::tempdir().unwrap();
    let _temp_path = fs::canonicalize(temp_dir.path()).unwrap();

    // Override the base config dir for this test to the temp dir.
    // We cannot easily override config::Config::config_dir() here,
    // so we'll test the logic by verifying it blocks obvious directory traversals
    // assuming it defaults to the user's config dir.

    // Instead, let's just make sure get_safe_path returns None for absolute or traversal paths
    assert!(super::get_safe_path("/etc/passwd").is_none());
    assert!(super::get_safe_path("../../../../../etc/passwd").is_none());

    #[cfg(windows)]
    assert!(super::get_safe_path("C:\\Windows\\System32\\config\\SAM").is_none());
}
