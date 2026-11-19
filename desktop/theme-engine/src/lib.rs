//! Theme definitions, discovery and colour handling.
//!
//! A theme is a TOML palette on disk. Nothing about a theme is compiled into
//! this crate, so a user can add a theme without touching Rust source code.

pub mod generate;
pub mod loader;
pub mod template;

pub use generate::generate_palette;
pub use loader::{
    BrokenTheme, Discovery, LoadedTheme, Registry, ThemeColors, ThemeLoadError, ThemeLocation,
    ThemeMetadata, ThemeSource, ThemeSummary, load_theme, load_theme_by_id, load_theme_by_name,
    parse_color, validate_id,
};
pub use template::{render_from_parts, render_theme_toml};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Color {
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.red, self.green, self.blue)
    }

    /// Parses `#RRGGBB` or `RRGGBB`.
    pub fn from_hex(value: &str) -> Option<Self> {
        let hex = value.strip_prefix('#').unwrap_or(value);

        if hex.len() != 6 || !hex.chars().all(|character| character.is_ascii_hexdigit()) {
            return None;
        }

        Some(Self::new(
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        ))
    }

    /// Blends towards `other`; `amount` is clamped to `0.0..=1.0`.
    pub fn mix(self, other: Color, amount: f32) -> Color {
        let amount = amount.clamp(0.0, 1.0);

        let blend = |from: u8, to: u8| {
            (f32::from(from) * (1.0 - amount) + f32::from(to) * amount).round() as u8
        };

        Color::new(
            blend(self.red, other.red),
            blend(self.green, other.green),
            blend(self.blue, other.blue),
        )
    }

    pub fn lighten(self, amount: f32) -> Color {
        self.mix(Color::new(255, 255, 255), amount)
    }

    pub fn darken(self, amount: f32) -> Color {
        self.mix(Color::new(0, 0, 0), amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_convert_to_hex() {
        assert_eq!(Color::new(0, 210, 255).to_hex(), "#00D2FF");
    }

    #[test]
    fn parses_hex_colours() {
        assert_eq!(Color::from_hex("#00D2FF"), Some(Color::new(0, 210, 255)));
        assert_eq!(Color::from_hex("00d2ff"), Some(Color::new(0, 210, 255)));
        assert_eq!(Color::from_hex("#GG0000"), None);
        assert_eq!(Color::from_hex("#123"), None);
    }

    #[test]
    fn mixes_colours() {
        let black = Color::new(0, 0, 0);
        let white = Color::new(255, 255, 255);

        assert_eq!(black.mix(white, 0.5), Color::new(128, 128, 128));
        assert_eq!(black.mix(white, 0.0), black);
        assert_eq!(black.mix(white, 1.0), white);
        assert_eq!(black.lighten(1.0), white);
        assert_eq!(white.darken(1.0), black);
    }
}
