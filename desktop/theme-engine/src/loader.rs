use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::Color;

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeMetadata {
    pub name: String,
    pub id: String,
    pub version: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawColors {
    pub background: String,
    pub surface: String,
    pub surface_alt: String,
    pub primary: String,
    pub secondary: String,
    pub text: String,
    pub text_muted: String,
    pub border: String,
    pub success: String,
    pub warning: String,
    pub error: String,
    pub states: RawStates,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawStates {
    pub active: String,
    pub hover: String,
    pub selected: String,
}

#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub background: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub primary: Color,
    pub secondary: Color,
    pub text: Color,
    pub text_muted: Color,
    pub border: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub active: Color,
    pub hover: Color,
    pub selected: Color,
}

#[derive(Debug, Clone)]
pub struct LoadedTheme {
    pub metadata: ThemeMetadata,
    pub colors: ThemeColors,
}

#[derive(Debug)]
pub enum ThemeLoadError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    InvalidColor(String),
    UnknownTheme(String),
}

impl std::fmt::Display for ThemeLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Could not read theme file: {error}"),
            Self::Toml(error) => write!(f, "Invalid TOML: {error}"),
            Self::InvalidColor(color) => write!(f, "Invalid hexadecimal color: {color}"),
            Self::UnknownTheme(name) => write!(f, "Unknown theme: {name}"),
        }
    }
}

impl std::error::Error for ThemeLoadError {}

impl From<std::io::Error> for ThemeLoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<toml::de::Error> for ThemeLoadError {
    fn from(error: toml::de::Error) -> Self {
        Self::Toml(error)
    }
}

fn parse_color(value: &str) -> Result<Color, ThemeLoadError> {
    let hex = value.strip_prefix('#').unwrap_or(value);

    if hex.len() != 6 {
        return Err(ThemeLoadError::InvalidColor(value.to_string()));
    }

    let red = u8::from_str_radix(&hex[0..2], 16)
        .map_err(|_| ThemeLoadError::InvalidColor(value.to_string()))?;
    let green = u8::from_str_radix(&hex[2..4], 16)
        .map_err(|_| ThemeLoadError::InvalidColor(value.to_string()))?;
    let blue = u8::from_str_radix(&hex[4..6], 16)
        .map_err(|_| ThemeLoadError::InvalidColor(value.to_string()))?;

    Ok(Color::new(red, green, blue))
}

pub fn load_theme(path: impl AsRef<Path>) -> Result<LoadedTheme, ThemeLoadError> {
    let contents = fs::read_to_string(path)?;
    let raw: RawTheme = toml::from_str(&contents)?;

    let c = raw.colors;
    let s = c.states;

    let colors = ThemeColors {
        background: parse_color(&c.background)?,
        surface: parse_color(&c.surface)?,
        surface_alt: parse_color(&c.surface_alt)?,
        primary: parse_color(&c.primary)?,
        secondary: parse_color(&c.secondary)?,
        text: parse_color(&c.text)?,
        text_muted: parse_color(&c.text_muted)?,
        border: parse_color(&c.border)?,
        success: parse_color(&c.success)?,
        warning: parse_color(&c.warning)?,
        error: parse_color(&c.error)?,
        active: parse_color(&s.active)?,
        hover: parse_color(&s.hover)?,
        selected: parse_color(&s.selected)?,
    };

    Ok(LoadedTheme {
        metadata: raw.theme,
        colors,
    })
}

#[derive(Debug, Deserialize)]
struct RawTheme {
    theme: ThemeMetadata,
    colors: RawColors,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_futuristic_theme() {
        let theme = load_theme("../../themes/futuristic/colors/palette.toml")
            .expect("Futuristic theme should load");

        assert_eq!(theme.metadata.id, "futuristic");
        assert_eq!(theme.colors.background.to_hex(), "#0B1020");
        assert_eq!(theme.colors.primary.to_hex(), "#A78BFA");
        assert_eq!(theme.colors.hover.to_hex(), "#22D3EE");
    }

    #[test]
    fn loads_all_four_themes() {
        for name in ["futuristic", "minimal", "cyberpunk", "nature"] {
            let path = format!("../../themes/{name}/colors/palette.toml");
            let theme = load_theme(path).expect("Theme should load");

            assert_eq!(theme.metadata.id, name);
        }
    }

    #[test]
    fn rejects_invalid_colors() {
        assert!(parse_color("#GG0000").is_err());
        assert!(parse_color("#123").is_err());
    }
}


/// Loads a theme by its name from the themes directory.
pub fn load_theme_by_name(
    themes_dir: impl AsRef<Path>,
    name: &str,
) -> Result<LoadedTheme, ThemeLoadError> {
    let theme = crate::Theme::from_name(name)
        .ok_or_else(|| ThemeLoadError::UnknownTheme(name.to_string()))?;

    let directory = theme.name().to_lowercase();

    let palette_path = themes_dir
        .as_ref()
        .join(directory)
        .join("colors")
        .join("palette.toml");

    load_theme(palette_path)
}

#[cfg(test)]
mod named_loader_tests {
    use super::*;

    #[test]
    fn loads_theme_by_name() {
        for name in ["futuristic", "minimal", "cyberpunk", "nature"] {
            let theme = load_theme_by_name("../../themes", name)
                .expect("Theme should load by name");

            assert_eq!(theme.metadata.id, name);
        }
    }

    #[test]
    fn rejects_unknown_theme_names() {
        let result = load_theme_by_name("../../themes", "unknown");

        assert!(matches!(result, Err(ThemeLoadError::UnknownTheme(_))));
    }
}
