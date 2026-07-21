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

/// `reset_theme_element` restores the *style's own shipped* layout for one
/// element - monstercat's hand-tuned lyrics position, not the generic
/// `ThemeLayout::default()` baseline - and leaves every other element's
/// edits untouched. Regression test: this used to reset to the generic
/// baseline regardless of style, so resetting e.g. the Visualiser tab on
/// monstercat replaced its Linear/0.6/1.5 look with the generic
/// Circular/0.25/1.0 one instead of restoring what monstercat ships with.
#[test]
fn reset_theme_element_restores_this_styles_own_shipped_defaults_for_one_element_only() {
    let mut layout = cosmic_wallpaper::modules::config::ThemeLayout::load("monstercat");
    let monstercat_defaults =
        cosmic_wallpaper::modules::config::ThemeLayout::builtin_default("monstercat");
    let generic_defaults = cosmic_wallpaper::modules::config::ThemeLayout::default();

    // monstercat's own defaults differ from the generic ones, or this test
    // wouldn't actually be exercising the distinction the function exists for.
    assert_ne!(layout.lyrics.position, generic_defaults.lyrics.position);

    super::apply_theme_edit(&mut layout, 2, super::ThemeEditMsg::PosX(0.1));
    super::apply_theme_edit(&mut layout, 0, super::ThemeEditMsg::Size(0.9));

    super::reset_theme_element(&mut layout, "monstercat", 2);

    assert_eq!(layout.lyrics.position, monstercat_defaults.lyrics.position);
    assert_eq!(layout.lyrics.align, monstercat_defaults.lyrics.align);
    // The untouched element keeps its edit.
    assert_eq!(layout.album_art.size, 0.9);
}

/// Regression test for the actual reported bug: by the time a built-in
/// style has been edited in the theme editor at all, its autosave has
/// already written a `shaders/<style>.toml` reflecting those edits, so
/// `reset_theme_element` restoring via `ThemeLayout::load` (which reads
/// that file) would just hand the edited values right back instead of the
/// style's shipped look. It must use `builtin_default`, which ignores the
/// file entirely.
#[test]
fn reset_theme_element_restores_shipped_defaults_even_after_the_style_file_was_saved() {
    let _guard = crate::tests::ENV_MUTEX.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let prev = std::env::var("XDG_CONFIG_HOME").ok();
    std::env::set_var("XDG_CONFIG_HOME", tmp.path());

    let result = {
        let shaders_dir = cosmic_wallpaper::modules::config::Config::config_dir().join("shaders");
        std::fs::create_dir_all(&shaders_dir).unwrap();
        // Simulate the editor's own autosave: monstercat's shipped lyrics
        // position edited away and written to disk, same as a real
        // dragged-slider debounce would.
        let mut edited = cosmic_wallpaper::modules::config::ThemeLayout::load("monstercat");
        edited.lyrics.position = [0.1, 0.1];
        std::fs::write(
            shaders_dir.join("monstercat.toml"),
            toml::to_string_pretty(&edited).unwrap(),
        )
        .unwrap();

        // Loading now returns the edited-and-saved value, not the shipped one.
        let mut layout = cosmic_wallpaper::modules::config::ThemeLayout::load("monstercat");
        assert_eq!(layout.lyrics.position, [0.1, 0.1]);

        super::reset_theme_element(&mut layout, "monstercat", 2);

        let shipped = cosmic_wallpaper::modules::config::ThemeLayout::builtin_default("monstercat");
        layout.lyrics.position == shipped.lyrics.position
    };

    match prev {
        Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }
    assert!(
        result,
        "reset must restore monstercat's shipped lyrics position, not the saved-over-it value"
    );
}

#[test]
fn reset_theme_element_ignores_out_of_range_index() {
    let mut layout = cosmic_wallpaper::modules::config::ThemeLayout::load("no-such-theme");
    let before = toml::to_string_pretty(&layout).unwrap();
    super::reset_theme_element(&mut layout, "no-such-theme", 99);
    let after = toml::to_string_pretty(&layout).unwrap();
    assert_eq!(before, after);
}

// ------------------------------------------------------------------ Packs

/// Full export-then-import round trip: `build_pack_bytes` reads a theme's
/// video and shader off disk into a pack, `config::pack::parse` recovers
/// them, and `write_pack_to_disk` lands them back in the same folders a
/// plain (non-pack) theme drop or a video import would use.
#[test]
fn export_then_import_round_trip_recovers_theme_video_and_shader() {
    let _guard = crate::tests::ENV_MUTEX.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let prev = std::env::var("XDG_CONFIG_HOME").ok();
    std::env::set_var("XDG_CONFIG_HOME", tmp.path());

    {
        let shaders_dir = cosmic_wallpaper::modules::config::Config::config_dir().join("shaders");
        std::fs::create_dir_all(&shaders_dir).unwrap();
        let mut layout = cosmic_wallpaper::modules::config::ThemeLayout::load("my-look");
        layout.visualiser.shader = Some("cool.wgsl".to_string());
        std::fs::write(
            shaders_dir.join("my-look.toml"),
            toml::to_string_pretty(&layout).unwrap(),
        )
        .unwrap();
        std::fs::write(shaders_dir.join("cool.wgsl"), b"// a custom shader").unwrap();

        let videos_dir = super::library::videos_dir();
        std::fs::create_dir_all(&videos_dir).unwrap();
        std::fs::write(videos_dir.join("clip.mp4"), b"fake video bytes").unwrap();

        let bytes = super::build_pack_bytes("my-look", Some("clip.mp4")).unwrap();
        let parsed = cosmic_wallpaper::modules::config::pack::parse(&bytes).unwrap();

        assert_eq!(parsed.name, "my-look");
        assert_eq!(
            parsed.background.as_ref().map(|(f, _)| f.as_str()),
            Some("clip.mp4")
        );
        assert_eq!(
            parsed.shader.as_ref().map(|(f, _)| f.as_str()),
            Some("cool.wgsl")
        );

        // Import into a clean folder (as if a different machine).
        std::fs::remove_file(shaders_dir.join("my-look.toml")).unwrap();
        std::fs::remove_file(shaders_dir.join("cool.wgsl")).unwrap();
        std::fs::remove_file(videos_dir.join("clip.mp4")).unwrap();

        let written_as = super::write_pack_to_disk(
            &parsed.name,
            &parsed.theme_toml,
            parsed.background.clone(),
            parsed.shader.clone(),
        )
        .unwrap();

        assert_eq!(written_as, "my-look");
        assert!(shaders_dir.join("my-look.toml").exists());
        assert_eq!(
            std::fs::read(shaders_dir.join("cool.wgsl")).unwrap(),
            b"// a custom shader"
        );
        assert_eq!(
            std::fs::read(videos_dir.join("clip.mp4")).unwrap(),
            b"fake video bytes"
        );

        // The gallery on the Packs page reads this back to offer a 1-click
        // re-apply, including which video came bundled with it.
        let installed = super::library::scan_installed_packs();
        let entry = installed.iter().find(|p| p.name == "my-look").unwrap();
        assert_eq!(entry.background.as_deref(), Some("clip.mp4"));
    }

    match prev {
        Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }
}

/// `write_pack_to_disk` must refuse a pack whose theme name would escape
/// the shaders directory, the same guard `is_safe_path` gives every other
/// disk-writing entry point in this file.
#[test]
fn write_pack_to_disk_rejects_a_path_traversing_theme_name() {
    let _guard = crate::tests::ENV_MUTEX.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let prev = std::env::var("XDG_CONFIG_HOME").ok();
    std::env::set_var("XDG_CONFIG_HOME", tmp.path());

    let result = super::write_pack_to_disk("../../evil", "font_family = \"x\"", None, None);

    match prev {
        Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }
    assert!(result.is_err());
}

/// A pack named after a theme that already exists must not silently
/// clobber it - very plausible in practice, since built-in style names
/// (`bars`, `monstercat`, ...) are exactly the kind of name an exported
/// pack keeps by default. It should land under a de-duplicated name
/// instead, the same convention `export_pack` already uses for output
/// files, and the caller must be told the actual name used.
#[test]
fn write_pack_to_disk_does_not_clobber_an_existing_theme_of_the_same_name() {
    let _guard = crate::tests::ENV_MUTEX.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let prev = std::env::var("XDG_CONFIG_HOME").ok();
    std::env::set_var("XDG_CONFIG_HOME", tmp.path());

    {
        let shaders_dir = cosmic_wallpaper::modules::config::Config::config_dir().join("shaders");
        std::fs::create_dir_all(&shaders_dir).unwrap();
        std::fs::write(shaders_dir.join("bars.toml"), "font_family = \"MyOwnEdit\"").unwrap();

        let written_as =
            super::write_pack_to_disk("bars", "font_family = \"SomeoneElses\"", None, None)
                .unwrap();

        assert_eq!(written_as, "bars-1");
        assert_eq!(
            std::fs::read_to_string(shaders_dir.join("bars.toml")).unwrap(),
            "font_family = \"MyOwnEdit\"",
            "the importer's own bars.toml must be untouched"
        );
        assert_eq!(
            std::fs::read_to_string(shaders_dir.join("bars-1.toml")).unwrap(),
            "font_family = \"SomeoneElses\""
        );

        // A second same-name import doesn't collide with the first
        // dedup either.
        let written_as_again =
            super::write_pack_to_disk("bars", "font_family = \"AThirdOne\"", None, None).unwrap();
        assert_eq!(written_as_again, "bars-2");
    }

    match prev {
        Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }
}

/// A background entry that isn't actually a video file must be dropped
/// rather than imported - this is the GUI's job (not `config::pack::parse`,
/// which has no notion of what a video is), exercised the same way
/// `Message::PackFilesDropped`'s handler applies the filter.
#[test]
fn a_non_video_background_entry_is_filtered_out_before_writing() {
    let background = Some(("notes.txt".to_string(), b"not a video".to_vec()));
    let filtered =
        background.filter(|(name, _)| super::library::is_video_file(std::path::Path::new(name)));
    assert!(filtered.is_none());

    let background = Some(("clip.mp4".to_string(), b"fake video bytes".to_vec()));
    let filtered =
        background.filter(|(name, _)| super::library::is_video_file(std::path::Path::new(name)));
    assert!(filtered.is_some());
}
