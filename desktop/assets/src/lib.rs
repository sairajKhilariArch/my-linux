//! Wallpapers and desktop assets.
//!
//! This crate owns three things the rest of My Linux never touches directly:
//!
//! * the wallpaper library and the recorded wallpaper state,
//! * palette extraction from a wallpaper,
//! * discovery and settings for fonts, icon themes and cursor themes.
//!
//! Nothing here writes to a live desktop directory. Wallpapers are copied into
//! a My Linux library and wallpaper tools are only called when a command asks
//! for it.

use std::io;

pub mod assets;
pub mod palette;
pub mod preview;
pub mod testing;
pub mod wallpaper;

pub use assets::{
    AssetKind, AssetSettings, DiscoveredAsset, apply_cursor, discover, discover_in,
    hyprland_assets_file, install_asset, installed_assets, library_dir, read_settings,
    remove_asset, render_hyprland_assets, write_settings,
};
pub use palette::{Proposal, extract_palette, save_theme};
pub use preview::render_preview;
pub use wallpaper::{
    Wallpaper, WallpaperHistoryEntry, WallpaperLibrary, WallpaperState, WallpaperTool, detect_tool,
    set_wallpaper,
};

#[derive(Debug)]
pub enum AssetError {
    Io(io::Error),
    Image(String),
    Unsupported(String),
    Invalid(String),
    Config(String),
    Tool(String),
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Image(error) => write!(f, "Could not read the image: {error}"),
            Self::Unsupported(extension) => write!(
                f,
                "Unsupported image format `.{}`; My Linux reads PNG and JPEG wallpapers",
                extension
            ),
            Self::Invalid(message) => write!(f, "{message}"),
            Self::Config(message) => write!(f, "{message}"),
            Self::Tool(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for AssetError {}

impl From<io::Error> for AssetError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<config_manager::ConfigError> for AssetError {
    fn from(error: config_manager::ConfigError) -> Self {
        Self::Config(error.to_string())
    }
}

impl From<image::ImageError> for AssetError {
    fn from(error: image::ImageError) -> Self {
        Self::Image(error.to_string())
    }
}
