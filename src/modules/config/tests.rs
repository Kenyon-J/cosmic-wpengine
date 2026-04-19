#![cfg(test)]

use crate::modules::config::types::*;
use std::path::PathBuf;
use std::sync::Mutex;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// A helper function to run a test with environment variables locked.
/// It saves the original state of XDG_CONFIG_HOME and HOME, sets the new ones,
/// runs the test, and then restores the original state, even if the test panics.
fn with_env_lock<F>(xdg_config: Option<&str>, home: Option<&str>, test: F)
where
    F: FnOnce() + std::panic::UnwindSafe,
{
    let _guard = ENV_MUTEX.lock().unwrap();

    let orig_xdg = std::env::var("XDG_CONFIG_HOME").ok();
    let orig_home = std::env::var("HOME").ok();

    if let Some(val) = xdg_config {
        std::env::set_var("XDG_CONFIG_HOME", val);
    } else {
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    if let Some(val) = home {
        std::env::set_var("HOME", val);
    } else {
        std::env::remove_var("HOME");
    }

    let result = std::panic::catch_unwind(test);

    if let Some(val) = orig_xdg {
        std::env::set_var("XDG_CONFIG_HOME", val);
    } else {
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    if let Some(val) = orig_home {
        std::env::set_var("HOME", val);
    } else {
        std::env::remove_var("HOME");
    }

    if let Err(err) = result {
        std::panic::resume_unwind(err);
    }
}

/// Sets up a temporary directory mocking the COSMIC wallpaper config directory.
fn setup_mock_cosmic_dir(base_dir: &std::path::Path) -> PathBuf {
    let cosmic_dir = base_dir.join("cosmic/com.system76.CosmicBackground/v1");
    std::fs::create_dir_all(&cosmic_dir).unwrap();
    cosmic_dir
}

#[test]
fn test_custom_background_path_returns_early() {
    let config = AppearanceConfig {
        custom_background_path: Some("/my/custom/path.jpg".to_string()),
        ..Default::default()
    };

    // Even with no env variables or mock directories set, it should return the custom path.
    with_env_lock(None, None, || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(
            rt.block_on(config.resolved_background_path()),
            Some("/my/custom/path.jpg".to_string())
        );
    });
}

#[test]
fn test_fallback_to_xdg_config_home() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_home = temp_dir.path().join("config_home");
    let cosmic_dir = setup_mock_cosmic_dir(&config_home);

    let img_path = temp_dir.path().join("image.jpg");
    std::fs::write(&img_path, "fake image data").unwrap();

    let ron_content = format!(r#"Path("{}")"#, img_path.display());
    std::fs::write(cosmic_dir.join("bg.ron"), ron_content).unwrap();

    let config = AppearanceConfig::default();

    with_env_lock(
        Some(config_home.to_str().unwrap()),
        Some("/fake/home"),
        || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            assert_eq!(
                rt.block_on(config.resolved_background_path()),
                Some(img_path.to_string_lossy().to_string())
            );
        },
    );
}

#[test]
fn test_fallback_to_home_dir() {
    let temp_dir = tempfile::tempdir().unwrap();
    let home_dir = temp_dir.path().join("home_dir");
    let expected_config_dir = home_dir.join(".config");
    let cosmic_dir = setup_mock_cosmic_dir(&expected_config_dir);

    let img_path = temp_dir.path().join("image.jpg");
    std::fs::write(&img_path, "fake image data").unwrap();

    let ron_content = format!(r#"Path("{}")"#, img_path.display());
    std::fs::write(cosmic_dir.join("bg.ron"), ron_content).unwrap();

    let config = AppearanceConfig::default();

    // XDG_CONFIG_HOME is unset, so it should fall back to HOME/.config
    with_env_lock(None, Some(home_dir.to_str().unwrap()), || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(
            rt.block_on(config.resolved_background_path()),
            Some(img_path.to_string_lossy().to_string())
        );
    });
}

#[test]
fn test_parses_cosmic_ron_format_and_verifies_existence() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_home = temp_dir.path().join("config_home");
    let cosmic_dir = setup_mock_cosmic_dir(&config_home);

    // Path that exists
    let existing_img = temp_dir.path().join("exists.jpg");
    std::fs::write(&existing_img, "fake image").unwrap();

    // Path that does not exist
    let missing_img = temp_dir.path().join("missing.jpg");

    // First write the RON referencing a missing image. We make it older.
    let ron_missing = format!(r#"Path("{}")"#, missing_img.display());
    let missing_ron_path = cosmic_dir.join("missing_bg.ron");
    std::fs::write(&missing_ron_path, ron_missing).unwrap();

    // Wait a small amount to ensure modification times are distinct
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Write the RON referencing an existing image. We make it newer so it is checked first.
    let ron_exists = format!(r#"Path("{}")"#, existing_img.display());
    let exists_ron_path = cosmic_dir.join("exists_bg.ron");
    std::fs::write(&exists_ron_path, ron_exists).unwrap();

    let config = AppearanceConfig::default();

    with_env_lock(Some(config_home.to_str().unwrap()), None, || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(
            rt.block_on(config.resolved_background_path()),
            Some(existing_img.to_string_lossy().to_string())
        );
    });
}

#[test]
fn test_falls_back_to_older_config_if_newer_is_invalid() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_home = temp_dir.path().join("config_home");
    let cosmic_dir = setup_mock_cosmic_dir(&config_home);

    let valid_img = temp_dir.path().join("valid.jpg");
    std::fs::write(&valid_img, "fake image").unwrap();

    let missing_img = temp_dir.path().join("missing.jpg");

    // Write the OLDER RON referencing the VALID image.
    let ron_valid = format!(r#"Path("{}")"#, valid_img.display());
    let valid_ron_path = cosmic_dir.join("older_valid_bg.ron");
    std::fs::write(&valid_ron_path, ron_valid).unwrap();

    // Ensure the newer file has a strictly later modification time.
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Write the NEWER RON referencing the MISSING image.
    let ron_missing = format!(r#"Path("{}")"#, missing_img.display());
    let missing_ron_path = cosmic_dir.join("newer_missing_bg.ron");
    std::fs::write(&missing_ron_path, ron_missing).unwrap();

    let config = AppearanceConfig::default();

    with_env_lock(Some(config_home.to_str().unwrap()), None, || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        // It should skip the newer RON (since its image is missing)
        // and pick the older RON (whose image exists).
        assert_eq!(
            rt.block_on(config.resolved_background_path()),
            Some(valid_img.to_string_lossy().to_string())
        );
    });
}

#[test]
fn test_selects_most_recently_modified_config() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_home = temp_dir.path().join("config_home");
    let cosmic_dir = setup_mock_cosmic_dir(&config_home);

    let older_img = temp_dir.path().join("older.jpg");
    std::fs::write(&older_img, "old").unwrap();

    let newer_img = temp_dir.path().join("newer.jpg");
    std::fs::write(&newer_img, "new").unwrap();

    let ron_older = format!(r#"Path("{}")"#, older_img.display());
    let older_ron_path = cosmic_dir.join("older_bg.ron");
    std::fs::write(&older_ron_path, ron_older).unwrap();

    // Ensure the newer file has a strictly later modification time.
    std::thread::sleep(std::time::Duration::from_millis(50));

    let ron_newer = format!(r#"Path("{}")"#, newer_img.display());
    let newer_ron_path = cosmic_dir.join("newer_bg.ron");
    std::fs::write(&newer_ron_path, ron_newer).unwrap();

    let config = AppearanceConfig::default();

    with_env_lock(Some(config_home.to_str().unwrap()), None, || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        // It should pick the path from the newer RON file.
        assert_eq!(
            rt.block_on(config.resolved_background_path()),
            Some(newer_img.to_string_lossy().to_string())
        );
    });
}

#[test]
fn test_both_env_vars_missing() {
    let config = AppearanceConfig::default();
    // With both XDG_CONFIG_HOME and HOME unset, it should not panic
    // and should return None.
    with_env_lock(None, None, || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(rt.block_on(config.resolved_background_path()), None);
    });
}

#[test]
fn test_cosmic_bg_dir_missing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_home = temp_dir.path().join("config_home");
    // Do NOT create the cosmic_dir so it will fail the read_dir.

    let config = AppearanceConfig::default();

    with_env_lock(Some(config_home.to_str().unwrap()), None, || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(rt.block_on(config.resolved_background_path()), None);
    });
}

#[test]
fn test_cosmic_bg_dir_empty() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_home = temp_dir.path().join("config_home");
    // Create an empty dir
    let _cosmic_dir = setup_mock_cosmic_dir(&config_home);

    let config = AppearanceConfig::default();

    with_env_lock(Some(config_home.to_str().unwrap()), None, || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(rt.block_on(config.resolved_background_path()), None);
    });
}

#[test]
fn test_invalid_ron_format() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_home = temp_dir.path().join("config_home");
    let cosmic_dir = setup_mock_cosmic_dir(&config_home);

    // Write a file with an invalid format (no Path("..."))
    let ron_content = r#"NotTheRightFormat("/path/that/does/not/exist.jpg")"#;
    std::fs::write(cosmic_dir.join("bg.ron"), ron_content).unwrap();

    let config = AppearanceConfig::default();

    with_env_lock(Some(config_home.to_str().unwrap()), None, || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(rt.block_on(config.resolved_background_path()), None);
    });
}

#[test]
fn test_fallback_to_home_dir_with_xdg_config_home_unset() {
    let temp_dir = tempfile::tempdir().unwrap();
    let home_dir = temp_dir.path().join("home_dir");
    let expected_config_dir = home_dir.join(".config");
    let cosmic_dir = setup_mock_cosmic_dir(&expected_config_dir);

    let img_path = temp_dir.path().join("image.jpg");
    std::fs::write(&img_path, "fake image data").unwrap();

    let ron_content = format!(r#"Path("{}")"#, img_path.display());
    std::fs::write(cosmic_dir.join("bg.ron"), ron_content).unwrap();

    let config = AppearanceConfig::default();

    with_env_lock(None, Some(home_dir.to_str().unwrap()), || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(
            rt.block_on(config.resolved_background_path()),
            Some(img_path.to_string_lossy().to_string())
        );
    });
}

#[test]
fn test_fallback_with_both_env_vars_missing() {
    let config = AppearanceConfig::default();

    with_env_lock(None, None, || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        // With both vars missing, HOME is not set, so it should fall back to an empty string,
        // producing `.config/...` relatively, which will probably fail to read.
        // This tests that we handle this missing case without panicking.
        assert_eq!(rt.block_on(config.resolved_background_path()), None);
    });
}
