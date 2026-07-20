#![cfg(test)]

/// Guards every test in this *binary crate* (`cosmic-wallpaper-gui`) that
/// mutates XDG_CONFIG_HOME: `cargo test` runs tests in parallel threads by
/// default, and env vars are shared process state. The library crate has
/// its own such mutex (`modules::utils::test_support::ENV_MUTEX`), but that
/// module is `#[cfg(test)]` and so isn't compiled in when the library is
/// pulled in as this binary's regular dependency - this binary needs its
/// own, and it must be exactly one: library.rs's tests used to declare a
/// second, independent `static ENV_MUTEX`, which raced against this one
/// (two unsynchronized locks "guarding" the same env var) and intermittently
/// poisoned whichever one lost. Any file in this crate that needs to set
/// XDG_CONFIG_HOME in a test must lock *this* one, via `crate::tests::ENV_MUTEX`.
pub(crate) static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

#[test]
fn theme_template_parses_and_matches_defaults() {
    let parsed: cosmic_wallpaper::modules::config::ThemeLayout =
        toml::from_str(super::THEME_TEMPLATE).expect("template must parse");
    // The template documents the defaults; a drift means the comments lie.
    let defaults = cosmic_wallpaper::modules::config::ThemeLayout::load("no-such-theme");
    assert_eq!(parsed.album_art.position, defaults.album_art.position);
    assert_eq!(parsed.lyrics.size, defaults.lyrics.size);
    assert_eq!(parsed.visualiser.amplitude, defaults.visualiser.amplitude);
    assert_eq!(parsed.effects.lyric_bounce, defaults.effects.lyric_bounce);
}

#[test]
fn theme_edits_apply_and_roundtrip() {
    let mut layout = cosmic_wallpaper::modules::config::ThemeLayout::load("no-such-theme");
    super::apply_theme_edit(&mut layout, 2, super::ThemeEditMsg::Size(1.8));
    super::apply_theme_edit(&mut layout, 2, super::ThemeEditMsg::PosX(0.25));
    super::apply_theme_edit(&mut layout, 3, super::ThemeEditMsg::Shape(1));
    super::apply_theme_edit(&mut layout, 5, super::ThemeEditMsg::BeatPulse(2.0));
    assert_eq!(layout.lyrics.size, 1.8);
    assert_eq!(layout.lyrics.position[0], 0.25);
    assert!(matches!(
        layout.visualiser.shape,
        cosmic_wallpaper::modules::config::VisShape::Circular
    ));
    assert_eq!(layout.effects.beat_pulse, 2.0);

    // Serialise (as the editor's save does) and parse back.
    let text = toml::to_string_pretty(&layout).expect("layout must serialise");
    let reparsed: cosmic_wallpaper::modules::config::ThemeLayout =
        toml::from_str(&text).expect("serialised layout must reparse");
    assert_eq!(reparsed.lyrics.size, 1.8);
}

#[test]
fn gallery_themes_parse() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("themes");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).expect("themes/ exists").flatten() {
        if entry.path().extension().is_some_and(|e| e == "toml") {
            let text = std::fs::read_to_string(entry.path()).unwrap();
            toml::from_str::<cosmic_wallpaper::modules::config::ThemeLayout>(&text)
                .unwrap_or_else(|e| panic!("{:?} must parse: {e}", entry.file_name()));
            checked += 1;
        }
    }
    assert!(checked >= 2, "gallery should contain themes");
}

/// The systemctl-show parser behind the engine row's failure explanations:
/// a failed unit must produce an actionable message (exit 127 gets the
/// missing-libraries diagnosis), anything healthy must produce none.
#[test]
fn parse_unit_failure_maps_failure_states() {
    // Healthy states: no message.
    assert_eq!(
        super::parse_unit_failure("ActiveState=active\nExecMainStatus=0\n"),
        None
    );
    assert_eq!(
        super::parse_unit_failure("ActiveState=inactive\nExecMainStatus=0\n"),
        None
    );
    // Unknown unit / empty output: no message.
    assert_eq!(super::parse_unit_failure(""), None);

    // Exit 127 (dynamic linker refused the binary): the specific diagnosis.
    let msg = super::parse_unit_failure("ActiveState=failed\nExecMainStatus=127\n").unwrap();
    assert!(msg.contains("exit 127"));
    assert!(msg.contains("missing libraries"));

    // Any other failure: generic message that points at the journal.
    let msg = super::parse_unit_failure("ActiveState=failed\nExecMainStatus=1\n").unwrap();
    assert!(msg.contains("exit 1"));
    assert!(msg.contains("journalctl"));
}

#[test]
fn stderr_headline_prefers_the_linker_error() {
    let stderr = "\nsome warning\n\
        /home/u/.local/bin/cosmic-wallpaper: error while loading shared libraries: \
        libavutil.so.58: cannot open shared object file: No such file or directory\n";
    assert!(super::stderr_headline(stderr).contains("libavutil.so.58"));

    // No linker line: first non-empty line wins.
    assert_eq!(super::stderr_headline("\n\npanic: boom\n"), "panic: boom");
    assert_eq!(super::stderr_headline(""), "");
}

/// The prefilled bug-report body: always carries version/OS context, and
/// only grows the error-excerpt section when there's actually something to
/// show - an empty `<details>` block would just be clutter.
#[test]
fn issue_body_includes_version_and_omits_empty_error_section() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let prev = std::env::var("XDG_CONFIG_HOME").ok();
    std::env::set_var("XDG_CONFIG_HOME", tmp.path());

    let body = super::build_issue_body();

    match prev {
        Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }

    assert!(body.contains(env!("CARGO_PKG_VERSION")));
    assert!(
        !body.contains("<details>"),
        "no log files exist, so there's nothing to show"
    );
}

/// With a real error line on disk, the body must fold it into the
/// collapsible section rather than silently dropping it.
#[test]
fn issue_body_attaches_recent_engine_errors_when_present() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let prev = std::env::var("XDG_CONFIG_HOME").ok();
    std::env::set_var("XDG_CONFIG_HOME", tmp.path());

    let log_dir = cosmic_wallpaper::modules::logging::log_dir();
    std::fs::create_dir_all(&log_dir).unwrap();
    std::fs::write(
        log_dir.join("engine.log.2026-07-20"),
        "2026-07-20T00:00:00Z ERROR cosmic_wallpaper::modules::mpris: fetch failed: timed out\n",
    )
    .unwrap();

    let body = super::build_issue_body();

    match prev {
        Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }

    assert!(body.contains("<details>"));
    assert!(body.contains("fetch failed: timed out"));
}

/// `reset_theme_element` restores the *generic* `ThemeLayout::default()`
/// baseline for one element, not the style's own hand-tuned layout (a
/// custom theme's file is the thing being edited, so its own "defaults"
/// would just be whatever's already on disk) - and leaves every other
/// element's edits untouched.
#[test]
fn reset_theme_element_restores_generic_defaults_for_one_element_only() {
    let mut layout = cosmic_wallpaper::modules::config::ThemeLayout::load("monstercat");
    let generic_defaults = cosmic_wallpaper::modules::config::ThemeLayout::default();

    // monstercat's own defaults differ from the generic ones, or this test
    // wouldn't actually be exercising the distinction the function exists for.
    assert_ne!(layout.lyrics.position, generic_defaults.lyrics.position);

    super::apply_theme_edit(&mut layout, 2, super::ThemeEditMsg::PosX(0.1));
    super::apply_theme_edit(&mut layout, 0, super::ThemeEditMsg::Size(0.9));

    super::reset_theme_element(&mut layout, 2);

    assert_eq!(layout.lyrics.position, generic_defaults.lyrics.position);
    assert_eq!(layout.lyrics.align, generic_defaults.lyrics.align);
    // The untouched element keeps its edit.
    assert_eq!(layout.album_art.size, 0.9);
}

#[test]
fn reset_theme_element_ignores_out_of_range_index() {
    let mut layout = cosmic_wallpaper::modules::config::ThemeLayout::load("no-such-theme");
    let before = toml::to_string_pretty(&layout).unwrap();
    super::reset_theme_element(&mut layout, 99);
    let after = toml::to_string_pretty(&layout).unwrap();
    assert_eq!(before, after);
}
