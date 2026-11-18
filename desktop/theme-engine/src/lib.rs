#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Futuristic,
    Minimal,
    Cyberpunk,
    Nature,
}

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
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub muted_text: Color,
    pub accent: Color,
    pub accent_secondary: Color,
}

impl Theme {
    pub const fn all() -> [Theme; 4] {
        [
            Theme::Futuristic,
            Theme::Minimal,
            Theme::Cyberpunk,
            Theme::Nature,
        ]
    }

    pub const fn name(self) -> &'static str {
        match self {
            Theme::Futuristic => "Futuristic",
            Theme::Minimal => "Minimal",
            Theme::Cyberpunk => "Cyberpunk",
            Theme::Nature => "Nature",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "futuristic" => Some(Theme::Futuristic),
            "minimal" => Some(Theme::Minimal),
            "cyberpunk" => Some(Theme::Cyberpunk),
            "nature" => Some(Theme::Nature),
            _ => None,
        }
    }

    pub const fn palette(self) -> Palette {
        match self {
            Theme::Futuristic => Palette {
                background: Color::new(11, 16, 32),
                surface: Color::new(24, 34, 56),
                text: Color::new(235, 242, 255),
                muted_text: Color::new(145, 165, 195),
                accent: Color::new(0, 210, 255),
                accent_secondary: Color::new(150, 80, 255),
            },

            Theme::Minimal => Palette {
                background: Color::new(242, 244, 247),
                surface: Color::new(255, 255, 255),
                text: Color::new(35, 40, 48),
                muted_text: Color::new(115, 123, 135),
                accent: Color::new(80, 100, 125),
                accent_secondary: Color::new(145, 155, 170),
            },

            Theme::Cyberpunk => Palette {
                background: Color::new(12, 8, 18),
                surface: Color::new(30, 15, 35),
                text: Color::new(255, 240, 250),
                muted_text: Color::new(180, 140, 180),
                accent: Color::new(255, 35, 90),
                accent_secondary: Color::new(0, 230, 255),
            },

            Theme::Nature => Palette {
                background: Color::new(18, 35, 27),
                surface: Color::new(31, 55, 40),
                text: Color::new(235, 245, 225),
                muted_text: Color::new(165, 190, 155),
                accent: Color::new(110, 190, 90),
                accent_secondary: Color::new(205, 175, 105),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_four_themes_exist() {
        assert_eq!(Theme::all().len(), 4);
    }

    #[test]
    fn theme_names_are_recognized() {
        assert_eq!(Theme::from_name("futuristic"), Some(Theme::Futuristic));
        assert_eq!(Theme::from_name("MINIMAL"), Some(Theme::Minimal));
        assert_eq!(Theme::from_name("Cyberpunk"), Some(Theme::Cyberpunk));
        assert_eq!(Theme::from_name("nature"), Some(Theme::Nature));
        assert_eq!(Theme::from_name("unknown"), None);
    }

    #[test]
    fn every_theme_has_a_palette() {
        for theme in Theme::all() {
            let palette = theme.palette();

            assert_ne!(palette.background, palette.accent);
        }
    }

    #[test]
    fn colors_convert_to_hex() {
        let color = Color::new(0, 210, 255);

        assert_eq!(color.to_hex(), "#00D2FF");
    }
}

pub mod loader;
