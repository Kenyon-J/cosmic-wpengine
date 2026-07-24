use super::app_config::AppearanceConfig;
use serde::Deserialize;
use std::path::PathBuf;

/// The desktop background the frosted-glass mode should render behind the
/// scene: either an image on disk or a colour/gradient the renderer must
/// synthesise itself (cosmic-bg draws those directly, there is no file).
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedBackground {
    Image(String),
    Colour([f32; 3]),
    Gradient {
        colors: Vec<[f32; 3]>,
        /// cosmic-bg stores this in a field named `radius`, but renders it as
        /// a linear-gradient angle in degrees (0 = bottom-to-top, clockwise).
        angle_deg: f32,
    },
}

/// Mirrors of cosmic-bg's RON config types; variant and field names must
/// match cosmic-bg's `Source`/`Color`/`Gradient` exactly to deserialize.
#[derive(Debug, Clone, PartialEq, Deserialize)]
enum CosmicBgSource {
    Path(PathBuf),
    Color(CosmicBgColor),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
enum CosmicBgColor {
    Single([f32; 3]),
    Gradient(CosmicBgGradient),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CosmicBgGradient {
    colors: Vec<[f32; 3]>,
    radius: f32,
}

/// A cosmic-bg per-output `Entry`; every field except `source` is irrelevant
/// here and ignored by serde.
#[derive(Deserialize)]
struct CosmicBgEntry {
    source: CosmicBgSource,
}

/// Extracts the wallpaper source from one cosmic-bg config file. Files in the
/// v1 dir hold either a full `Entry` struct or (in older setups and our
/// tests) a bare `Source`; anything else falls back to scanning for a
/// `Path("...")` substring, which tolerates value shapes we can't fully parse.
fn parse_bg_source(contents: &str) -> Option<CosmicBgSource> {
    if let Ok(entry) = ron::from_str::<CosmicBgEntry>(contents) {
        return Some(entry.source);
    }
    if let Ok(source) = ron::from_str::<CosmicBgSource>(contents) {
        return Some(source);
    }
    let start_idx = contents.find("Path(\"")?;
    let path_start = start_idx + 6;
    let end_offset = contents[path_start..].find("\")")?;
    Some(CosmicBgSource::Path(PathBuf::from(
        &contents[path_start..path_start + end_offset],
    )))
}

impl AppearanceConfig {
    pub async fn resolved_background(&self) -> Option<ResolvedBackground> {
        if let Some(path) = &self.custom_background_path {
            return Some(ResolvedBackground::Image(path.clone()));
        }

        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_default();
                PathBuf::from(home).join(".config")
            });

        let cosmic_bg_dir = base.join("cosmic/com.system76.CosmicBackground/v1");

        let mut entries_with_time = Vec::new();

        if let Ok(mut entries) = tokio::fs::read_dir(cosmic_bg_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(meta) = entry.metadata().await {
                    if let Ok(modified) = meta.modified() {
                        entries_with_time.push((entry.path(), modified));
                    }
                }
            }
        }

        entries_with_time.sort_unstable_by_key(|b| std::cmp::Reverse(b.1));

        for (path, _) in entries_with_time {
            let Ok(contents) = tokio::fs::read_to_string(&path).await else {
                continue;
            };
            match parse_bg_source(&contents) {
                Some(CosmicBgSource::Path(img_path)) => {
                    // Skip entries whose image has since been deleted so an
                    // older config file can still win.
                    if tokio::fs::metadata(&img_path).await.is_ok() {
                        return Some(ResolvedBackground::Image(
                            img_path.to_string_lossy().into_owned(),
                        ));
                    }
                }
                Some(CosmicBgSource::Color(CosmicBgColor::Single(colour))) => {
                    return Some(ResolvedBackground::Colour(colour));
                }
                Some(CosmicBgSource::Color(CosmicBgColor::Gradient(gradient)))
                    if !gradient.colors.is_empty() =>
                {
                    return Some(ResolvedBackground::Gradient {
                        colors: gradient.colors,
                        angle_deg: gradient.radius,
                    });
                }
                _ => {}
            }
        }

        None
    }
}
