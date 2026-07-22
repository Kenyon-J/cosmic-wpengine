//! Theme packs (`.cwtheme`): a gzipped tar bundling a layout theme with its
//! background video and/or a custom visualiser shader, so a full "look" can
//! be shared as one file - modelled on Wallpaper Engine workshop items.
//!
//! Deliberately a plain tar, not a bespoke container: a user can inspect a
//! pack's shader with the standard `tar` CLI before this app ever reads it.
//! `pack.toml`'s tables are all optional (`#[serde(default)]`, no
//! `deny_unknown_fields`) so a pack sharing one thing stays tiny, and a
//! future field never breaks an older build.

use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};

use super::ThemeLayout;

/// The highest `schema_version` this build understands. A pack declaring a
/// higher version is rejected outright (not just warned about) since the
/// import path can't know what a newer table means, but any table it
/// doesn't recognise yet within a supported version is silently ignored.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// A tar entry's uncompressed size is trusted at face value while iterating,
/// but never read past this cap - a bound against a maliciously-crafted
/// gzip stream claiming (or decompressing to) an enormous entry.
const MAX_ENTRY_BYTES: u64 = 512 * 1024 * 1024;

/// Whole-pack cap, checked two ways: the caller should reject a `.cwtheme`
/// bigger than this via `Path::metadata` *before* ever reading it into
/// memory (see the GUI's pack-drop handler), and `parse` re-checks it
/// against both the input slice's own length and the running total of
/// every entry's declared size while iterating. That second check matters
/// on its own: `MAX_ENTRY_BYTES` only bounds any *one* entry, so a pack
/// with many entries each just under that cap could otherwise still force
/// gigabytes of cumulative decompression work despite never tripping the
/// per-entry check. Generous enough for one `MAX_ENTRY_BYTES` video plus
/// the small theme/shader/manifest entries every pack also carries; a
/// `.cwtheme` bigger than this is never a legitimate export from this app.
pub const MAX_PACK_BYTES: u64 = 600 * 1024 * 1024;

/// Cheap defence against a pack with an absurd number of tiny entries (each
/// individually well under `MAX_ENTRY_BYTES`/`MAX_PACK_BYTES`) built purely
/// to make this loop do unnecessary work - a real pack only ever has four
/// entries (pack.toml, theme.toml, one background, one shader).
const MAX_PACK_ENTRIES: usize = 64;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct PackManifest {
    pub schema_version: u32,
    pub name: String,
    /// This app's version string (`CARGO_PKG_VERSION`) at export time.
    /// Purely informational - `schema_version` is what actually gates
    /// whether a pack is safe to attempt (see `SUPPORTED_SCHEMA_VERSION`'s
    /// doc comment). This just lets a theme.toml that fails to parse for
    /// an unrelated reason (say, a newer `VisShape` variant an older build
    /// has never heard of) say which version produced it, instead of
    /// surfacing a bare TOML error with no context. Empty on a pack built
    /// before this field existed.
    pub app_version: String,
    pub background: Option<PackBackground>,
    pub shader: Option<PackShader>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct PackBackground {
    /// File name under `background/` inside the archive.
    pub file: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct PackShader {
    /// File name under `shader/` inside the archive.
    pub file: String,
    /// Which built-in stage this shader overrides. Only `"visualiser"` is
    /// meaningful today.
    pub replaces: String,
}

/// Everything needed to build a pack, already read into memory by the
/// caller (the GUI reuses its already-loaded `ThemeLayout` and reads
/// video/shader bytes from its own config dir).
pub struct PackContents {
    pub name: String,
    pub theme_toml: String,
    /// (file name, bytes)
    pub background: Option<(String, Vec<u8>)>,
    /// (file name, bytes)
    pub shader: Option<(String, Vec<u8>)>,
}

/// Builds a gzip-compressed tar of `contents` and returns its bytes.
pub fn build(contents: &PackContents) -> Result<Vec<u8>> {
    let manifest = PackManifest {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        name: contents.name.clone(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        background: contents
            .background
            .as_ref()
            .map(|(file, _)| PackBackground { file: file.clone() }),
        shader: contents.shader.as_ref().map(|(file, _)| PackShader {
            file: file.clone(),
            replaces: "visualiser".to_string(),
        }),
    };
    let manifest_toml = toml::to_string_pretty(&manifest).context("serialising pack.toml")?;

    let gz = GzEncoder::new(Vec::new(), Compression::default());
    let mut tar = tar::Builder::new(gz);

    append_file(&mut tar, "pack.toml", manifest_toml.as_bytes())?;
    append_file(&mut tar, "theme.toml", contents.theme_toml.as_bytes())?;
    if let Some((file, bytes)) = &contents.background {
        append_file(&mut tar, &format!("background/{file}"), bytes)?;
    }
    if let Some((file, bytes)) = &contents.shader {
        append_file(&mut tar, &format!("shader/{file}"), bytes)?;
    }

    let gz = tar.into_inner().context("finishing pack tar")?;
    gz.finish().context("finishing pack gzip stream")
}

fn append_file<W: Write>(tar: &mut tar::Builder<W>, name: &str, bytes: &[u8]) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, name, bytes)
        .with_context(|| format!("writing {name} into pack"))
}

/// Everything extracted from a pack, ready for the caller to either write
/// straight to disk (no shader) or stash pending an explicit review gate
/// (has a shader).
#[derive(Debug)]
pub struct ParsedPack {
    pub name: String,
    pub theme_toml: String,
    /// (file name, bytes)
    pub background: Option<(String, Vec<u8>)>,
    /// (file name, bytes)
    pub shader: Option<(String, Vec<u8>)>,
}

/// Parses a `.cwtheme` file's bytes entirely into memory - nothing is
/// written to disk here. Iterates entries with an explicit match on the
/// expected names (never a blind `Archive::unpack()`, which is a tar-slip
/// path-traversal risk); anything else, including any entry outside the
/// allow-listed names, is ignored.
pub fn parse(bytes: &[u8]) -> Result<ParsedPack> {
    if bytes.len() as u64 > MAX_PACK_BYTES {
        bail!(
            "pack file too large ({} bytes, limit {MAX_PACK_BYTES})",
            bytes.len()
        );
    }

    let gz = GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);

    let mut manifest: Option<PackManifest> = None;
    let mut theme_toml: Option<String> = None;
    let mut background: Option<(String, Vec<u8>)> = None;
    let mut shader: Option<(String, Vec<u8>)> = None;

    let mut entry_count: usize = 0;
    let mut cumulative_bytes: u64 = 0;

    for entry in archive.entries().context("reading pack archive")? {
        let mut entry = entry.context("reading pack archive entry")?;

        entry_count += 1;
        if entry_count > MAX_PACK_ENTRIES {
            bail!("pack has too many entries (limit {MAX_PACK_ENTRIES})");
        }

        if entry.size() > MAX_ENTRY_BYTES {
            bail!("pack entry too large");
        }
        // Bounds total decompression work across the whole archive, not
        // just any single entry - see `MAX_PACK_BYTES`'s doc comment.
        cumulative_bytes = cumulative_bytes.saturating_add(entry.size());
        if cumulative_bytes > MAX_PACK_BYTES {
            bail!("pack's total entry size exceeds the {MAX_PACK_BYTES}-byte limit");
        }
        let path = entry
            .path()
            .context("reading pack entry path")?
            .into_owned();
        let path_str = path.to_string_lossy().into_owned();

        if path_str == "pack.toml" {
            let text = read_capped(&mut entry)?;
            manifest = Some(toml::from_str(&text).context("parsing pack.toml")?);
        } else if path_str == "theme.toml" {
            // Not validated here: doing that only after the schema-version
            // check below means an unsupported-schema pack is rejected by
            // that check's clearer message even when the entries happen to
            // appear in an unusual order, and lets a theme parse failure
            // below name the app version that produced the pack.
            theme_toml = Some(read_capped(&mut entry)?);
        } else if let Some(name) = path_str.strip_prefix("background/") {
            // Only the file name is trusted (strips any directory
            // component a crafted archive might smuggle in); this crate
            // has no notion of "is this actually a video", so the caller
            // is expected to re-validate that before writing it to disk -
            // see the GUI's pack import handler.
            let Some(name) = Path::new(name).file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            background = Some((name.to_string(), bytes));
        } else if let Some(name) = path_str.strip_prefix("shader/") {
            let Some(name) = Path::new(name).file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.to_ascii_lowercase().ends_with(".wgsl") {
                continue;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            shader = Some((name.to_string(), bytes));
        }
        // Anything else is ignored, per the extensibility design: a future
        // asset kind needs a new match arm here, not a format redesign.
    }

    let manifest = manifest.context("pack is missing pack.toml")?;
    if manifest.schema_version > SUPPORTED_SCHEMA_VERSION {
        bail!(
            "pack requires a newer version of this app (schema {}, supported {})",
            manifest.schema_version,
            SUPPORTED_SCHEMA_VERSION
        );
    }
    let theme_toml = theme_toml.context("pack is missing theme.toml")?;
    if let Err(e) = toml::from_str::<ThemeLayout>(&theme_toml) {
        return Err(if manifest.app_version.is_empty() {
            anyhow::Error::new(e).context("parsing theme.toml")
        } else {
            anyhow::anyhow!(
                "parsing theme.toml (this pack was made with cosmic-wallpaper {} - you may need \
                 to update): {e}",
                manifest.app_version
            )
        });
    }

    Ok(ParsedPack {
        name: manifest.name,
        theme_toml,
        background,
        shader,
    })
}

fn read_capped(entry: &mut tar::Entry<'_, impl Read>) -> Result<String> {
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    String::from_utf8(bytes).context("pack entry is not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_contents() -> PackContents {
        PackContents {
            name: "my-look".to_string(),
            theme_toml: toml::to_string_pretty(&ThemeLayout::default()).unwrap(),
            background: Some(("clip.mp4".to_string(), b"fake video bytes".to_vec())),
            shader: Some(("cool.wgsl".to_string(), b"// fake shader".to_vec())),
        }
    }

    #[test]
    fn round_trip_recovers_theme_background_and_shader() {
        let contents = sample_contents();
        let bytes = build(&contents).unwrap();
        let parsed = parse(&bytes).unwrap();

        assert_eq!(parsed.name, contents.name);
        assert_eq!(parsed.theme_toml, contents.theme_toml);
        assert_eq!(parsed.background, contents.background);
        assert_eq!(parsed.shader, contents.shader);
    }

    #[test]
    fn round_trip_with_no_background_or_shader() {
        let contents = PackContents {
            name: "bare".to_string(),
            theme_toml: toml::to_string_pretty(&ThemeLayout::default()).unwrap(),
            background: None,
            shader: None,
        };
        let bytes = build(&contents).unwrap();
        let parsed = parse(&bytes).unwrap();

        assert_eq!(parsed.name, "bare");
        assert!(parsed.background.is_none());
        assert!(parsed.shader.is_none());
    }

    /// A crafted archive entry named e.g. `shader/../../../etc/evil.wgsl`
    /// must not escape - `parse` only ever looks at the entry's final path
    /// component, mirroring the same `Path::file_name()` sanitisation used
    /// elsewhere in this codebase for user-controlled file names.
    #[test]
    fn path_traversal_in_an_entry_name_is_reduced_to_its_file_name() {
        let manifest = PackManifest {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            name: "evil".to_string(),
            shader: Some(PackShader {
                file: "evil.wgsl".to_string(),
                replaces: "visualiser".to_string(),
            }),
            ..Default::default()
        };
        let manifest_toml = toml::to_string_pretty(&manifest).unwrap();
        let theme_toml = toml::to_string_pretty(&ThemeLayout::default()).unwrap();

        let gz = GzEncoder::new(Vec::new(), Compression::default());
        let mut tar = tar::Builder::new(gz);
        append_file(&mut tar, "pack.toml", manifest_toml.as_bytes()).unwrap();
        append_file(&mut tar, "theme.toml", theme_toml.as_bytes()).unwrap();

        // `Header::set_path`/`Builder::append_data` both refuse a `..`
        // component outright, so a real attacker crafting bytes by hand
        // (not through this crate's own safe builder) is the threat model
        // here - write the malicious name straight into the raw header
        // field via the lower-level `Builder::append`, bypassing that
        // validation, to prove `parse` itself is the backstop.
        let malicious = b"shader/../../../../../../tmp/evil.wgsl\0";
        let mut header = tar::Header::new_gnu();
        header.as_gnu_mut().unwrap().name[..malicious.len()].copy_from_slice(malicious);
        header.set_size(b"malicious".len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, &b"malicious"[..]).unwrap();

        let bytes = tar.into_inner().unwrap().finish().unwrap();

        let parsed = parse(&bytes).unwrap();
        let (file, shader_bytes) = parsed.shader.unwrap();
        assert_eq!(file, "evil.wgsl");
        assert_eq!(shader_bytes, b"malicious");
    }

    /// Regression test for a pack built purely to make `parse`'s loop do
    /// unnecessary work: many small, unrecognised entries, none of them
    /// anywhere near `MAX_ENTRY_BYTES`/`MAX_PACK_BYTES` individually or
    /// even summed. Only the entry *count* makes this pack abusive, so
    /// only the count cap (not the byte caps) can catch it.
    #[test]
    fn a_pack_with_too_many_entries_is_rejected() {
        let manifest_toml = toml::to_string_pretty(&PackManifest {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            name: "many-entries".to_string(),
            ..Default::default()
        })
        .unwrap();
        let theme_toml = toml::to_string_pretty(&ThemeLayout::default()).unwrap();

        let gz = GzEncoder::new(Vec::new(), Compression::default());
        let mut tar = tar::Builder::new(gz);
        append_file(&mut tar, "pack.toml", manifest_toml.as_bytes()).unwrap();
        append_file(&mut tar, "theme.toml", theme_toml.as_bytes()).unwrap();
        for i in 0..MAX_PACK_ENTRIES {
            append_file(&mut tar, &format!("junk/{i}"), b"x").unwrap();
        }
        let bytes = tar.into_inner().unwrap().finish().unwrap();

        let err = parse(&bytes).unwrap_err();
        assert!(
            err.to_string().contains("too many entries"),
            "unexpected error: {err}"
        );
    }

    /// A legitimately-sized pack (comfortably under every new cap) must
    /// keep working - the caps exist for abuse, not normal use.
    #[test]
    fn a_normally_sized_pack_is_unaffected_by_the_new_caps() {
        let contents = sample_contents();
        let bytes = build(&contents).unwrap();
        assert!((bytes.len() as u64) < MAX_PACK_BYTES);
        assert!(parse(&bytes).is_ok());
    }

    #[test]
    fn a_schema_version_newer_than_supported_is_rejected() {
        // A manifest claiming a schema version this build has never heard
        // of, built by hand.
        let manifest = PackManifest {
            schema_version: SUPPORTED_SCHEMA_VERSION + 1,
            name: "future".to_string(),
            ..Default::default()
        };
        let manifest_toml = toml::to_string_pretty(&manifest).unwrap();
        let theme_toml = toml::to_string_pretty(&ThemeLayout::default()).unwrap();
        let gz = GzEncoder::new(Vec::new(), Compression::default());
        let mut tar = tar::Builder::new(gz);
        append_file(&mut tar, "pack.toml", manifest_toml.as_bytes()).unwrap();
        append_file(&mut tar, "theme.toml", theme_toml.as_bytes()).unwrap();
        let bytes = tar.into_inner().unwrap().finish().unwrap();

        let err = parse(&bytes).unwrap_err();
        assert!(err.to_string().contains("newer version"));
    }

    #[test]
    fn a_pack_missing_pack_toml_is_rejected_not_panicked() {
        let gz = GzEncoder::new(Vec::new(), Compression::default());
        let mut tar = tar::Builder::new(gz);
        append_file(
            &mut tar,
            "theme.toml",
            toml::to_string_pretty(&ThemeLayout::default())
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        let bytes = tar.into_inner().unwrap().finish().unwrap();

        let err = parse(&bytes).unwrap_err();
        assert!(err.to_string().contains("pack.toml"));
    }

    #[test]
    fn a_pack_with_invalid_theme_toml_is_rejected_not_panicked() {
        let manifest = PackManifest {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            name: "broken".to_string(),
            ..Default::default()
        };
        let gz = GzEncoder::new(Vec::new(), Compression::default());
        let mut tar = tar::Builder::new(gz);
        append_file(
            &mut tar,
            "pack.toml",
            toml::to_string_pretty(&manifest).unwrap().as_bytes(),
        )
        .unwrap();
        append_file(&mut tar, "theme.toml", b"this is not valid toml {{{").unwrap();
        let bytes = tar.into_inner().unwrap().finish().unwrap();

        assert!(parse(&bytes).is_err());
    }

    /// When a pack's `theme.toml` fails to parse - e.g. a future app
    /// version added a `VisShape`/`VisAlign` variant this build has never
    /// heard of - and the pack does carry an `app_version`, the error
    /// should name it rather than surface a bare TOML error with no
    /// context for what the user should actually do about it.
    #[test]
    fn a_broken_theme_toml_names_the_app_version_that_made_the_pack() {
        let manifest = PackManifest {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            name: "future-theme".to_string(),
            app_version: "1.9.9".to_string(),
            ..Default::default()
        };
        let gz = GzEncoder::new(Vec::new(), Compression::default());
        let mut tar = tar::Builder::new(gz);
        append_file(
            &mut tar,
            "pack.toml",
            toml::to_string_pretty(&manifest).unwrap().as_bytes(),
        )
        .unwrap();
        append_file(
            &mut tar,
            "theme.toml",
            b"[visualiser]\nshape = \"hexagonal\"\n",
        )
        .unwrap();
        let bytes = tar.into_inner().unwrap().finish().unwrap();

        let err = parse(&bytes).unwrap_err();
        assert!(err.to_string().contains("1.9.9"));
    }

    /// The drop handler only checks the `.cwtheme` extension before calling
    /// `parse` - nothing stops a user renaming an arbitrary file. These
    /// prove garbage input is rejected cleanly (an `Err`, never a panic),
    /// covering the shapes a renamed random file could plausibly take:
    /// nothing at all, noise that's neither gzip nor tar, a real gzip
    /// stream of unrelated data, and a real tar with no gzip layer at all.
    #[test]
    fn empty_input_is_rejected_not_panicked() {
        assert!(parse(&[]).is_err());
    }

    #[test]
    fn random_bytes_are_rejected_not_panicked() {
        let garbage: Vec<u8> = (0u32..4096).map(|i| (i % 251) as u8).collect();
        assert!(parse(&garbage).is_err());
    }

    #[test]
    fn a_valid_gzip_stream_of_unrelated_data_is_rejected_not_panicked() {
        use std::io::Write as _;
        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(b"just some unrelated gzipped text, not a tar at all")
            .unwrap();
        let bytes = gz.finish().unwrap();
        assert!(parse(&bytes).is_err());
    }

    #[test]
    fn an_uncompressed_tar_with_no_gzip_layer_is_rejected_not_panicked() {
        // A real, well-formed tar - just missing the gzip wrapper `parse`
        // expects. Exercises the "valid tar, wrong container" case
        // distinctly from plain noise.
        let mut tar = tar::Builder::new(Vec::new());
        append_file(
            &mut tar,
            "theme.toml",
            toml::to_string_pretty(&ThemeLayout::default())
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        let bytes = tar.into_inner().unwrap();
        assert!(parse(&bytes).is_err());
    }
}
