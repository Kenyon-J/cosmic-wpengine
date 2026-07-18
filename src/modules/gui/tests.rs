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
