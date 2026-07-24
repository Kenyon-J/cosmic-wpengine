mod app_config;
mod cosmic_bg;
mod theme_layout;

pub use app_config::*;
pub use cosmic_bg::*;
pub use theme_layout::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WallpaperMode {
    AlbumArt,
    AudioVisualiser,
    Weather,
    Auto,
}
