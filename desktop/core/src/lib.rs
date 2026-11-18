use std::path::Path;

pub use theme_engine::loader::{LoadedTheme, ThemeLoadError};

/// Loads a desktop theme by name.
pub fn load_desktop_theme(
    themes_directory: impl AsRef<Path>,
    theme_name: &str,
) -> Result<LoadedTheme, ThemeLoadError> {
    theme_engine::loader::load_theme_by_name(themes_directory, theme_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_core_loads_cyberpunk_theme() {
        let theme = load_desktop_theme("../../themes", "cyberpunk")
            .expect("Desktop core should load Cyberpunk");

        assert_eq!(theme.metadata.name, "Cyberpunk");
        assert_eq!(theme.colors.primary.to_hex(), "#FF235A");
    }

    #[test]
    fn desktop_core_loads_all_themes() {
        for name in ["futuristic", "minimal", "cyberpunk", "nature"] {
            let theme = load_desktop_theme("../../themes", name)
                .expect("Desktop core should load every theme");

            assert_eq!(theme.metadata.id, name);
        }
    }
}

pub mod render;
