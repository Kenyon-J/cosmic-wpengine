use crate::{is_safe_path, library};
use cosmic_wallpaper::modules::config;

/// Writes `bytes` under `dir/<filename>` - or, only when a *different*
/// file already claims that exact name, a numbered variant instead
/// (`stem-1.ext`, `stem-2.ext`, ...). Two unrelated packs that happen to
/// share a theme/shader/video file name can't silently clobber (or be
/// silently shadowed by) each other this way. Writing the *same* bytes
/// under a name that already holds them is a no-op that reuses the
/// existing name, so re-dropping a pack you've already imported doesn't
/// clutter the folder with numbered duplicates every time. Returns the
/// file name actually used.
fn write_deduped(dir: &std::path::Path, filename: &str, bytes: &[u8]) -> std::io::Result<String> {
    let (stem, ext) = match filename.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), Some(ext.to_string())),
        _ => (filename.to_string(), None),
    };
    let mut n = 0u32;
    loop {
        let candidate = match (n, &ext) {
            (0, _) => filename.to_string(),
            (n, Some(ext)) => format!("{stem}-{n}.{ext}"),
            (n, None) => format!("{stem}-{n}"),
        };
        let path = dir.join(&candidate);
        if !path.exists() {
            std::fs::write(&path, bytes)?;
            return Ok(candidate);
        }
        if std::fs::read(&path).is_ok_and(|existing| existing == bytes) {
            return Ok(candidate);
        }
        n += 1;
    }
}

/// Rewrites `theme_toml`'s `[visualiser] shader` field from `old_name` to
/// `new_name`, after `write_deduped` had to rename the shader file to
/// avoid clobbering an unrelated existing one under the same name. Round-
/// trips through `ThemeLayout` rather than string-patching so this can't
/// drift from the format's actual shape; on the rare pack whose theme.toml
/// no longer parses, or whose `shader` field doesn't match, the text is
/// left untouched instead. Any hand-written comments in the original file
/// are lost in this rename path, an accepted tradeoff against silently
/// pointing at the wrong shader.
fn repoint_theme_shader(theme_toml: &str, old_name: &str, new_name: &str) -> String {
    let Ok(mut layout) = toml::from_str::<config::ThemeLayout>(theme_toml) else {
        return theme_toml.to_string();
    };
    if layout.visualiser.shader.as_deref() != Some(old_name) {
        return theme_toml.to_string();
    }
    layout.visualiser.shader = Some(new_name.to_string());
    toml::to_string_pretty(&layout).unwrap_or_else(|_| theme_toml.to_string())
}

/// Reads `name`'s layout plus (when set) `video_background_path`'s bytes
/// from the video library and the visualiser's custom shader bytes from the
/// shaders dir, and packs them into a `.cwtheme` archive's raw bytes. A free
/// function (rather than a `SettingsApp` method) so it's testable without a
/// live `Core`.
pub(crate) fn build_pack_bytes(
    name: &str,
    video_background_path: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    let layout = config::ThemeLayout::load(name);
    let theme_toml = toml::to_string_pretty(&layout)?;

    let background = video_background_path.and_then(|file| {
        let bytes = std::fs::read(library::videos_dir().join(file)).ok()?;
        Some((file.to_string(), bytes))
    });
    let shader = layout.visualiser.shader.as_ref().and_then(|file| {
        let file_name = std::path::Path::new(file)
            .file_name()?
            .to_str()?
            .to_string();
        let bytes = std::fs::read(
            config::Config::config_dir()
                .join("shaders")
                .join(&file_name),
        )
        .ok()?;
        Some((file_name, bytes))
    });

    let contents = config::pack::PackContents {
        name: name.to_string(),
        theme_toml,
        background,
        shader,
    };
    config::pack::build(&contents)
}

/// Writes a parsed pack's theme (and, when present, its background video
/// and/or shader) to disk. A free function (rather than a `SettingsApp`
/// method) so it's testable without a live `Core`.
///
/// Nothing here overwrites an unrelated file that happens to share a name -
/// built-in style names (`bars`, `monstercat`, ...) are very plausible pack
/// names, and a shared shader/video name is equally plausible, so silently
/// clobbering (or being silently shadowed by) an existing file would be a
/// real way to lose or corrupt someone's unrelated theme with no warning.
/// `write_deduped` handles the theme, shader and video writes alike; the
/// actual theme name used is returned so the caller can tell the user when
/// their import landed under a different name than the pack's own.
pub(crate) fn write_pack_to_disk(
    name: &str,
    theme_toml: &str,
    background: Option<(String, Vec<u8>)>,
    shader: Option<(String, Vec<u8>)>,
) -> Result<String, String> {
    if !is_safe_path(&format!("shaders/{name}.toml")) {
        return Err(format!("unsafe theme name '{name}'"));
    }
    let shaders_dir = config::Config::config_dir().join("shaders");
    std::fs::create_dir_all(&shaders_dir).map_err(|e| e.to_string())?;

    // Shader goes first: if it needs a dedup rename, theme.toml's own
    // `shader` field must be repointed to match before theme.toml is
    // written, or the imported theme would reference a file that isn't
    // the one actually sitting next to it.
    let mut theme_toml = theme_toml.to_string();
    if let Some((file, bytes)) = shader {
        let written = write_deduped(&shaders_dir, &file, &bytes).map_err(|e| e.to_string())?;
        if written != file {
            theme_toml = repoint_theme_shader(&theme_toml, &file, &written);
        }
    }

    let theme_file = write_deduped(&shaders_dir, &format!("{name}.toml"), theme_toml.as_bytes())
        .map_err(|e| e.to_string())?;
    let written_as = theme_file
        .strip_suffix(".toml")
        .unwrap_or(&theme_file)
        .to_string();

    let background_file = match background {
        Some((file, bytes)) => {
            let dir = library::videos_dir();
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            Some(write_deduped(&dir, &file, &bytes).map_err(|e| e.to_string())?)
        }
        None => None,
    };

    // Recorded regardless of whether this pack bundled a video, so the
    // Packs gallery lists every import - a layout-only or shader-only pack
    // is still one click away next time, not just the ones with video.
    library::record_installed_pack(&written_as, background_file.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(written_as)
}
