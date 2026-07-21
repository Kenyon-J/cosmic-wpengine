//! Video library support for the Live Wallpapers page: scanning the videos
//! folder, extracting cached thumbnails with ffmpeg, and importing files
//! dropped onto the window.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use cosmic_wallpaper::modules::config::Config;
use ffmpeg_next as ffmpeg;
use serde::{Deserialize, Serialize};

pub(crate) const VIDEO_EXTENSIONS: [&str; 5] = ["mp4", "webm", "mkv", "mov", "avi"];

/// Thumbnails are rendered at tile size; decoding is capped at this width.
const THUMB_WIDTH: u32 = 320;

#[derive(Debug, Clone)]
pub(crate) struct VideoEntry {
    /// File name inside the videos folder - the value stored in
    /// `appearance.video_background_path`.
    pub file_name: String,
    /// "m:ss", when the container reports a duration.
    pub duration: Option<String>,
    /// Cached first-frame PNG, when extraction succeeded.
    pub thumbnail: Option<PathBuf>,
}

pub(crate) fn videos_dir() -> PathBuf {
    Config::config_dir().join("videos")
}

/// Where exported `.cwtheme` packs are written - mirrors the `videos`/
/// `shaders` folder convention.
pub(crate) fn packs_dir() -> PathBuf {
    Config::config_dir().join("packs")
}

fn installed_packs_dir() -> PathBuf {
    packs_dir().join("installed")
}

/// One imported pack's bookkeeping - just enough to reapply it in one
/// click from the Packs page's gallery. This is local-only bookkeeping,
/// not part of the shareable `.cwtheme` format itself (`config::pack`):
/// a theme file has no notion of "which video came bundled with it", so
/// that association has to live somewhere once the pack's pieces are
/// unpacked into the usual `shaders`/`videos` folders.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct InstalledPackRecord {
    background: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct InstalledPack {
    /// Also the layout theme's name (`shaders/<name>.toml`).
    pub name: String,
    /// File name inside `videos_dir()`, when this pack bundled one.
    pub background: Option<String>,
}

/// Records that `name` was imported from a pack, so the Packs page's
/// gallery can offer it back with a single "Apply" click. Called only from
/// the pack-import path - a theme created or edited directly, or a plain
/// `.toml` drop, is never "installed" this way.
pub(crate) fn record_installed_pack(name: &str, background: Option<&str>) -> std::io::Result<()> {
    let dir = installed_packs_dir();
    std::fs::create_dir_all(&dir)?;
    let record = InstalledPackRecord {
        background: background.map(str::to_string),
    };
    let text = toml::to_string_pretty(&record).unwrap_or_default();
    std::fs::write(dir.join(format!("{name}.toml")), text)
}

/// Every pack ever imported into this profile, for the Packs page's
/// gallery. A record whose `background` no longer exists in `videos_dir()`
/// (the video was deleted by hand) is still listed - Apply then just
/// leaves the video setting alone rather than pointing at a missing file.
pub(crate) fn scan_installed_packs() -> Vec<InstalledPack> {
    let mut packs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(installed_packs_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml") {
                let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let record: InstalledPackRecord = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| toml::from_str(&text).ok())
                    .unwrap_or_default();
                packs.push(InstalledPack {
                    name: name.to_string(),
                    background: record.background,
                });
            }
        }
    }
    packs.sort_by(|a, b| a.name.cmp(&b.name));
    packs
}

fn thumbs_dir() -> PathBuf {
    videos_dir().join(".thumbs")
}

pub(crate) fn is_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| VIDEO_EXTENSIONS.iter().any(|v| ext.eq_ignore_ascii_case(v)))
}

/// Removes cached thumbnails whose source video no longer exists in
/// `videos_dir()` - the only way a thumbnail goes stale, since it's
/// generated once per video and never re-derived (no dynamic re-fetch, so
/// there's no "keep the last N" history to prune: exactly one thumbnail
/// per currently-existing video is both the minimum and the maximum this
/// cache ever holds). A video deleted from outside the app (a file
/// manager, `rm`, ...) previously left its thumbnail behind forever.
fn prune_orphaned_thumbnails(current_videos: &[String]) {
    let Ok(entries) = std::fs::read_dir(thumbs_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        // Thumbnails are named "<video file name>.png"; strip that suffix
        // to recover the video name it was generated for.
        let Some(video_name) = name
            .to_string_lossy()
            .strip_suffix(".png")
            .map(str::to_string)
        else {
            continue;
        };
        if !current_videos.contains(&video_name) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Scans the library, extracting any missing thumbnails and pruning
/// thumbnails for videos that no longer exist. Blocking (ffmpeg decode) -
/// run under `spawn_blocking`.
pub(crate) fn scan() -> Vec<VideoEntry> {
    let _ = std::fs::create_dir_all(thumbs_dir());
    let _ = ffmpeg::init();

    let available = Config::available_videos();
    prune_orphaned_thumbnails(&available);

    available
        .into_iter()
        .filter_map(|file_name| {
            let path = videos_dir().join(&file_name);
            if !is_video_file(&path) {
                return None;
            }
            let thumb_path = thumbs_dir().join(format!("{file_name}.png"));
            let duration = match probe_and_thumbnail(&path, &thumb_path) {
                Ok(duration) => duration,
                Err(e) => {
                    tracing::warn!("Failed to read video {}: {}", file_name, e);
                    None
                }
            };
            Some(VideoEntry {
                file_name,
                duration,
                thumbnail: thumb_path.exists().then_some(thumb_path),
            })
        })
        .collect()
}

/// Reads the container duration and, when no cached thumbnail exists yet,
/// decodes the first video frame into one.
fn probe_and_thumbnail(path: &Path, thumb_path: &Path) -> Result<Option<String>, ffmpeg::Error> {
    let mut ictx = ffmpeg::format::input(path)?;

    let duration = match ictx.duration() {
        d if d > 0 => {
            let secs = d / i64::from(ffmpeg::ffi::AV_TIME_BASE);
            Some(format!("{}:{:02}", secs / 60, secs % 60))
        }
        _ => None,
    };

    if thumb_path.exists() {
        return Ok(duration);
    }

    let input = ictx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;
    let stream_index = input.index();

    let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
    let mut decoder = context_decoder.decoder().video()?;

    let (src_w, src_h) = (decoder.width(), decoder.height());
    if src_w == 0 || src_h == 0 {
        return Ok(duration);
    }
    let dst_w = src_w.min(THUMB_WIDTH);
    let dst_h = ((u64::from(src_h) * u64::from(dst_w) / u64::from(src_w)) as u32).max(1);

    let mut scaler = ffmpeg::software::scaling::Context::get(
        decoder.format(),
        src_w,
        src_h,
        ffmpeg::format::Pixel::RGBA,
        dst_w,
        dst_h,
        ffmpeg::software::scaling::Flags::BILINEAR,
    )?;

    let mut decoded = ffmpeg::frame::Video::empty();
    for (stream, packet) in ictx.packets() {
        if stream.index() != stream_index {
            continue;
        }
        decoder.send_packet(&packet)?;
        if decoder.receive_frame(&mut decoded).is_ok() {
            let mut rgba = ffmpeg::frame::Video::empty();
            scaler.run(&decoded, &mut rgba)?;

            // The scaler may pad rows; copy row by row at the packed width.
            let stride = rgba.stride(0);
            let row_len = (dst_w * 4) as usize;
            let mut pixels = Vec::with_capacity(row_len * dst_h as usize);
            for row in 0..dst_h as usize {
                let start = row * stride;
                pixels.extend_from_slice(&rgba.data(0)[start..start + row_len]);
            }

            if let Some(img) = image::RgbaImage::from_raw(dst_w, dst_h, pixels) {
                if let Err(e) = img.save(thumb_path) {
                    tracing::warn!("Failed to save thumbnail: {}", e);
                }
            }
            break;
        }
    }

    Ok(duration)
}

/// Copies dropped video files into the library. Blocking - run under
/// `spawn_blocking`. Returns (imported, skipped) counts.
pub(crate) fn import(paths: Vec<PathBuf>) -> (usize, usize) {
    let dir = videos_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut imported = 0;
    let mut skipped = 0;
    for path in paths {
        let Some(name) = path.file_name() else {
            skipped += 1;
            continue;
        };
        if is_video_file(&path) && std::fs::copy(&path, dir.join(name)).is_ok() {
            imported += 1;
        } else {
            skipped += 1;
        }
    }
    (imported, skipped)
}

/// A `text/uri-list` drag-and-drop payload, decoded to local file paths.
#[derive(Debug, Clone)]
pub(crate) struct DroppedFiles(pub Vec<PathBuf>);

impl TryFrom<(Vec<u8>, String)> for DroppedFiles {
    type Error = std::string::FromUtf8Error;

    fn try_from((data, _mime): (Vec<u8>, String)) -> Result<Self, Self::Error> {
        let text = String::from_utf8(data)?;
        Ok(Self(
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .filter_map(|line| url::Url::parse(line).ok()?.to_file_path().ok())
                .collect(),
        ))
    }
}

impl cosmic::iced::clipboard::mime::AllowedMimeTypes for DroppedFiles {
    fn allowed() -> Cow<'static, [String]> {
        Cow::Owned(vec!["text/uri-list".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_temp_config_dir<R>(f: impl FnOnce() -> R) -> R {
        // XDG_CONFIG_HOME is shared process state; every test in this
        // binary that mutates it locks the one shared mutex in
        // gui/tests.rs, not a file-local one - see that static's doc
        // comment for why a second lock here previously raced against it.
        let _guard = crate::tests::ENV_MUTEX.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let prev = std::env::var("XDG_CONFIG_HOME").ok();
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        let result = f();
        match prev {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        result
    }

    #[test]
    fn prune_removes_thumbnails_for_videos_that_no_longer_exist() {
        with_temp_config_dir(|| {
            std::fs::create_dir_all(thumbs_dir()).unwrap();
            std::fs::write(thumbs_dir().join("gone.mp4.png"), b"stale").unwrap();
            std::fs::write(thumbs_dir().join("still-here.mp4.png"), b"current").unwrap();

            prune_orphaned_thumbnails(&["still-here.mp4".to_string()]);

            assert!(!thumbs_dir().join("gone.mp4.png").exists());
            assert!(thumbs_dir().join("still-here.mp4.png").exists());
        });
    }

    #[test]
    fn prune_is_a_no_op_when_thumbs_dir_does_not_exist_yet() {
        with_temp_config_dir(|| {
            // No create_dir_all call: must not panic on a missing directory.
            prune_orphaned_thumbnails(&["anything.mp4".to_string()]);
        });
    }

    #[test]
    fn prune_ignores_non_png_entries_in_the_thumbs_dir() {
        with_temp_config_dir(|| {
            std::fs::create_dir_all(thumbs_dir()).unwrap();
            let stray = thumbs_dir().join(".gitkeep");
            std::fs::write(&stray, b"").unwrap();

            prune_orphaned_thumbnails(&[]);

            assert!(stray.exists(), "non-thumbnail files must be left alone");
        });
    }

    #[test]
    fn installed_pack_round_trips_through_scan() {
        with_temp_config_dir(|| {
            record_installed_pack("my-look", Some("clip.mp4")).unwrap();
            record_installed_pack("bare-layout", None).unwrap();

            let mut packs = scan_installed_packs();
            packs.sort_by(|a, b| a.name.cmp(&b.name));

            assert_eq!(packs.len(), 2);
            assert_eq!(packs[0].name, "bare-layout");
            assert_eq!(packs[0].background, None);
            assert_eq!(packs[1].name, "my-look");
            assert_eq!(packs[1].background.as_deref(), Some("clip.mp4"));
        });
    }

    #[test]
    fn scan_installed_packs_is_a_no_op_when_the_dir_does_not_exist_yet() {
        with_temp_config_dir(|| {
            assert!(scan_installed_packs().is_empty());
        });
    }

    #[test]
    fn re_recording_the_same_pack_name_overwrites_its_entry() {
        with_temp_config_dir(|| {
            record_installed_pack("my-look", Some("old.mp4")).unwrap();
            record_installed_pack("my-look", Some("new.mp4")).unwrap();

            let packs = scan_installed_packs();
            assert_eq!(packs.len(), 1);
            assert_eq!(packs[0].background.as_deref(), Some("new.mp4"));
        });
    }
}
