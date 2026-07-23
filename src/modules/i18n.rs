//! Fluent-backed translation catalog for this app's own strings, wired the
//! same way libcosmic wires its own (see its `src/localize.rs`): a
//! `FluentLanguageLoader` over an embedded `i18n/` directory, selected
//! against the desktop's locale on first use. Only `i18n/en/` ships today
//! (see docs/ROADMAP.md's i18n groundwork entry for the contribution flow) -
//! any other requested language, or any key a future catalog hasn't caught
//! up on yet, falls back to English.

use i18n_embed::fluent::{fluent_language_loader, FluentLanguageLoader};
use i18n_embed::{DefaultLocalizer, LanguageLoader, Localizer};
use rust_embed::RustEmbed;
use std::sync::{LazyLock, OnceLock};

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();
    loader
        .load_fallback_language(&Localizations)
        .expect("failed to load the fallback (en) translation catalog");
    loader
});

static LOCALIZATION_INITIALIZED: OnceLock<()> = OnceLock::new();

fn localizer() -> Box<dyn Localizer> {
    Box::from(DefaultLocalizer::new(&*LANGUAGE_LOADER, &Localizations))
}

/// Selects the desktop's requested language against the embedded catalogs.
/// Idempotent and cheap to call from every entry point - `fl!` also calls
/// this on first use, so both binaries pick up the right language the
/// moment they render their first string, with no separate startup wiring.
///
/// This is the *desktop-follows* default only. A saved manual override
/// (`Config.language`, set from the General page's picker) is applied by
/// each binary's own startup code calling `set_language()` explicitly
/// after loading config, before this has a chance to run - see
/// `set_language()`'s doc comment for why the ordering matters.
pub fn localize() {
    LOCALIZATION_INITIALIZED.get_or_init(|| {
        let requested = i18n_embed::DesktopLanguageRequester::requested_languages();
        if let Err(e) = localizer().select(&requested) {
            tracing::warn!("Could not select a translation for the desktop's locale: {e}");
        }
    });
}

/// Explicit language override, e.g. from the General page's picker (or
/// each binary's startup code, applying `Config.language`). `None` follows
/// the desktop's own locale, same as `localize()`.
///
/// Calls `localize()` first, unconditionally: `fl!`'s embedded call to
/// `localize()` only *ever* does anything the very first time it runs
/// (guarded by a `OnceLock`) - but if that first run happens *after* this
/// function has already selected an explicit override, it would silently
/// clobber the override back to the desktop locale. Forcing it to run here
/// first means whichever happens first, this override always wins.
pub fn set_language(tag: Option<&str>) {
    localize();
    let requested: Vec<i18n_embed::unic_langid::LanguageIdentifier> = match tag {
        Some(tag) => match tag.parse() {
            Ok(id) => vec![id],
            Err(e) => {
                tracing::warn!("Invalid language tag {tag:?}, ignoring: {e}");
                return;
            }
        },
        None => i18n_embed::DesktopLanguageRequester::requested_languages(),
    };
    if let Err(e) = localizer().select(&requested) {
        tracing::warn!("Could not select language {tag:?}: {e}");
    }
}

/// (tag, native display name) for every catalog embedded in this binary,
/// sorted by display name - built once, by asking each catalog for its own
/// `language-name` message (see any `i18n/<tag>/*.ftl`'s first entry) and
/// restoring whatever was selected before probing. Backs the General
/// page's language picker: a future community catalog for a language the
/// desktop environment itself doesn't offer appears here automatically,
/// with no code change.
pub static AVAILABLE_LANGUAGES: LazyLock<Vec<(String, String)>> = LazyLock::new(|| {
    let Ok(available) = LANGUAGE_LOADER.available_languages(&Localizations) else {
        return Vec::new();
    };
    let previous = LANGUAGE_LOADER.current_languages();
    let mut list: Vec<(String, String)> = available
        .iter()
        .filter_map(|id| {
            LANGUAGE_LOADER
                .load_languages(&Localizations, std::slice::from_ref(id))
                .ok()?;
            Some((id.to_string(), LANGUAGE_LOADER.get("language-name")))
        })
        .collect();
    list.sort_by(|a, b| a.1.cmp(&b.1));
    if !previous.is_empty() {
        let _ = LANGUAGE_LOADER.load_languages(&Localizations, &previous);
    }
    list
});

/// Looks up a Fluent message by id (with optional named arguments) in the
/// currently selected language. Usable from anywhere in this crate as
/// `crate::fl!`, and from the GUI/engine binaries as `cosmic_wallpaper::fl!`
/// (macro_export always places it at the defining crate's root).
#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        $crate::modules::i18n::localize();
        i18n_embed_fl::fl!($crate::modules::i18n::LANGUAGE_LOADER, $message_id)
    }};
    ($message_id:literal, $($args:expr),* $(,)?) => {{
        $crate::modules::i18n::localize();
        i18n_embed_fl::fl!($crate::modules::i18n::LANGUAGE_LOADER, $message_id, $($args), *)
    }};
}

#[cfg(test)]
mod tests {
    use i18n_embed::LanguageLoader;
    use std::sync::Mutex;

    // These tests all select a language on the shared LANGUAGE_LOADER, so
    // they must not interleave with each other. They also all call
    // `LANGUAGE_LOADER.get(...)` directly rather than through the `fl!`
    // macro: `fl!` calls `localize()`, which - the *first* time it is ever
    // called in the process - re-selects the language from the real
    // desktop locale (via DesktopLanguageRequester) and would silently
    // clobber whatever this test just selected. Calling `get`/`get_args`
    // directly on the loader sidesteps that one-time side effect entirely.
    static LOCALE_TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn lang(tag: &str) -> i18n_embed::unic_langid::LanguageIdentifier {
        tag.parse().unwrap()
    }

    fn select(tag: &str) {
        super::localizer()
            .select(&[lang(tag)])
            .unwrap_or_else(|e| panic!("{tag}: select failed: {e}"));
    }

    // A requested language with no matching catalog at all (unlike the six
    // real ones below) must fall back to English rather than panicking or
    // leaving a message unresolved - this is the behaviour the whole sweep
    // depends on ahead of any real translation landing.
    #[test]
    fn unavailable_requested_language_falls_back_to_english() {
        let _guard = LOCALE_TEST_MUTEX.lock().unwrap();
        select("ja");
        assert_eq!(super::LANGUAGE_LOADER.get("tray-title"), "COSMIC Wallpaper");
        assert_eq!(super::LANGUAGE_LOADER.get("status-ready"), "Ready.");
    }

    #[test]
    fn named_argument_interpolates() {
        let _guard = LOCALE_TEST_MUTEX.lock().unwrap();
        select("en");
        // Fluent wraps interpolated values in bidi isolation marks (U+2068/
        // U+2069) by design, so this checks substring presence rather than
        // exact equality.
        assert!(crate::fl!("status-applied-theme", theme = "Neon").contains("Neon"));
    }

    // Every community-drafted catalog must actually be selected over the
    // English fallback when requested, and must translate (not just leave
    // untouched) the strings this test exercises.
    #[test]
    fn every_drafted_locale_loads_and_translates() {
        let _guard = LOCALE_TEST_MUTEX.lock().unwrap();
        for tag in ["es", "fr", "de", "it", "nl", "pt"] {
            select(tag);
            assert_eq!(
                super::LANGUAGE_LOADER.current_language().language.as_str(),
                tag,
                "{tag}: catalog did not become the current language"
            );
            let title = super::LANGUAGE_LOADER.get("tray-title");
            assert_eq!(
                title, "COSMIC Wallpaper",
                "{tag}: app name should stay untranslated"
            );
            let translated = super::LANGUAGE_LOADER.get("status-ready");
            assert_ne!(
                translated, "Ready.",
                "{tag}: still showing the English fallback"
            );
        }
        select("en");
    }

    // Every plural message must resolve both its CLDR categories without
    // panicking, in every drafted locale - not just the `other` arm that a
    // single manual click-through would happen to exercise.
    #[test]
    fn plural_messages_resolve_both_categories_in_every_locale() {
        let _guard = LOCALE_TEST_MUTEX.lock().unwrap();
        for tag in ["en", "es", "fr", "de", "it", "nl", "pt"] {
            select(tag);
            let one = super::LANGUAGE_LOADER
                .get_args_concrete("status-imported-videos", [("n", 1_i64.into())].into());
            let other = super::LANGUAGE_LOADER
                .get_args_concrete("status-imported-videos", [("n", 3_i64.into())].into());
            assert!(
                !one.is_empty() && !other.is_empty(),
                "{tag}: empty plural result"
            );
        }
        select("en");
    }

    // Guards against a stray unescaped brace/quote in a hand-edited catalog
    // silently dropping entries at parse time (fluent_bundle keeps parsing
    // past an error and only logs it via the `log` crate, which this
    // project never bridges to `tracing` - a broken message would otherwise
    // fail silently instead of failing this test).
    #[test]
    fn every_drafted_locale_is_valid_fluent_syntax() {
        for tag in ["es", "fr", "de", "it", "nl", "pt"] {
            let path = format!("i18n/{tag}/io.github.kenyon_j.cosmic_wpengine.ftl");
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{tag}: could not read {path}: {e}"));
            if let Err((_, errors)) = fluent_bundle::FluentResource::try_new(content) {
                panic!("{tag}: {} Fluent parse error(s): {errors:?}", errors.len());
            }
        }
    }

    // Backs the General page's picker: every catalog must be listed, under
    // its own name (not e.g. every entry collapsing to the same fallback
    // string), and probing them must not leave some other test's selection
    // clobbered behind it.
    #[test]
    fn available_languages_lists_every_catalog_with_its_own_name() {
        let _guard = LOCALE_TEST_MUTEX.lock().unwrap();
        select("fr");
        let langs = &*super::AVAILABLE_LANGUAGES;
        let expected: std::collections::HashMap<&str, &str> = [
            ("en", "English"),
            ("es", "Español"),
            ("fr", "Français"),
            ("de", "Deutsch"),
            ("it", "Italiano"),
            ("nl", "Nederlands"),
            ("pt", "Português"),
        ]
        .into_iter()
        .collect();
        assert_eq!(langs.len(), expected.len(), "unexpected catalog count");
        for (tag, name) in langs {
            assert_eq!(
                expected.get(tag.as_str()),
                Some(&name.as_str()),
                "{tag}: unexpected or missing native name"
            );
        }
        // Probing every catalog's `language-name` must not have clobbered
        // the selection this test made before touching AVAILABLE_LANGUAGES.
        assert_eq!(
            super::LANGUAGE_LOADER.current_language().language.as_str(),
            "fr"
        );
        select("en");
    }

    // A bogus/unrecognised tag (a typo'd config value, or a catalog that
    // got removed) must be ignored rather than panicking - the picker falls
    // back to whatever was already selected.
    #[test]
    fn set_language_with_unknown_tag_does_not_panic() {
        let _guard = LOCALE_TEST_MUTEX.lock().unwrap();
        select("es");
        super::set_language(Some("not a real tag!!"));
        assert_eq!(
            super::LANGUAGE_LOADER.current_language().language.as_str(),
            "es"
        );
        select("en");
    }

    // The manual override must win regardless of whether `localize()`'s
    // one-time desktop-locale init has already fired by the time it runs -
    // this is the exact bug the sweep's first pass at these tests hit.
    #[test]
    fn set_language_overrides_regardless_of_localize_ordering() {
        let _guard = LOCALE_TEST_MUTEX.lock().unwrap();
        super::localize(); // simulate the OnceLock already having fired
        super::set_language(Some("de"));
        assert_eq!(
            super::LANGUAGE_LOADER.current_language().language.as_str(),
            "de"
        );
        select("en");
    }
}
