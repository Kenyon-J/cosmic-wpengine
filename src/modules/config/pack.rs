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

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct PackManifest {
    pub schema_version: u32,
    pub name: String,
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
    let gz = GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);

    let mut manifest: Option<PackManifest> = None;
    let mut theme_toml: Option<String> = None;
    let mut background: Option<(String, Vec<u8>)> = None;
    let mut shader: Option<(String, Vec<u8>)> = None;

    for entry in archive.entries().context("reading pack archive")? {
        let mut entry = entry.context("reading pack archive entry")?;
        if entry.size() > MAX_ENTRY_BYTES {
            bail!("pack entry too large");
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
            let text = read_capped(&mut entry)?;
            // Validated by the caller's toml::from_str::<ThemeLayout> pass
            // too, but a syntax error should fail the whole import loudly
            // rather than land a bad theme file no one asked for.
            toml::from_str::<ThemeLayout>(&text).context("parsing theme.toml")?;
            theme_toml = Some(text);
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
            background: None,
            shader: Some(PackShader {
                file: "evil.wgsl".to_string(),
                replaces: "visualiser".to_string(),
            }),
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

    #[test]
    fn a_schema_version_newer_than_supported_is_rejected() {
        // A manifest claiming a schema version this build has never heard
        // of, built by hand.
        let manifest = PackManifest {
            schema_version: SUPPORTED_SCHEMA_VERSION + 1,
            name: "future".to_string(),
            background: None,
            shader: None,
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
            background: None,
            shader: None,
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
}
