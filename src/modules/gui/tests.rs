#![cfg(test)]
use std::fs;

#[test]
fn test_resolve_safe_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    let base_dir = temp_dir.path();

    let shaders_dir = base_dir.join("shaders");
    fs::create_dir_all(&shaders_dir).unwrap();

    let shaders_nested_dir = shaders_dir.join("nested");
    fs::create_dir_all(&shaders_nested_dir).unwrap();

    // Valid paths
    assert!(super::resolve_safe_path("config.toml", base_dir).is_some());
    assert!(super::resolve_safe_path("shaders/theme.toml", base_dir).is_some());
    assert!(super::resolve_safe_path("shaders/nested/theme.toml", base_dir).is_some());

    // Path traversal
    assert!(super::resolve_safe_path("../test.txt", base_dir).is_none());
    assert!(super::resolve_safe_path("shaders/../../etc/passwd", base_dir).is_none());
    assert!(super::resolve_safe_path("..", base_dir).is_none());

    // Absolute paths
    assert!(super::resolve_safe_path("/etc/passwd", base_dir).is_none());
    #[cfg(windows)]
    assert!(super::resolve_safe_path("C:\\Windows\\System32\\config\\SAM", base_dir).is_none());
}
