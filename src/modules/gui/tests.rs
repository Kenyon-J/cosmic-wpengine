#![cfg(test)]

#[test]
fn test_is_safe_path() {
    // Valid paths
    assert!(super::is_safe_path("config.toml"));
    assert!(super::is_safe_path("shaders/theme.toml"));
    assert!(super::is_safe_path("shaders/nested/theme.toml"));

    // Path traversal
    assert!(!super::is_safe_path("../test.txt"));
    assert!(!super::is_safe_path("shaders/../../etc/passwd"));
    assert!(!super::is_safe_path(".."));

    // Absolute paths
    assert!(!super::is_safe_path("/etc/passwd"));
    #[cfg(windows)]
    assert!(!super::is_safe_path("C:\\Windows\\System32\\config\\SAM"));
}
