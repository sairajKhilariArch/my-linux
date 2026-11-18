use std::path::Path;

use config_manager::{ConfigError, backup_file, install_content};
pub use theme_engine::loader::{LoadedTheme, ThemeLoadError};

/// Loads a desktop theme by name.
/// Loads a desktop theme by name.
pub fn load_desktop_theme(
    themes_directory: impl AsRef<Path>,
    theme_name: &str,
) -> Result<LoadedTheme, ThemeLoadError> {
    theme_engine::loader::load_theme_by_name(themes_directory, theme_name)
}

/// Installs generated Waybar CSS into the selected destination.
pub fn install_waybar_theme(
    theme: &LoadedTheme,
    destination: impl AsRef<Path>,
    backup: impl AsRef<Path>,
) -> Result<(), ConfigError> {
    let destination = destination.as_ref();

    if destination.exists() && !backup.as_ref().exists() {
        backup_file(destination, backup)?;
    }

    let css = crate::render::render_waybar_css(theme);

    install_content(css.as_bytes(), destination)
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

    #[test]
    fn installs_waybar_theme_and_creates_backup() {
        let dir = std::env::temp_dir().join(format!("my-linux-waybar-test-{}", std::process::id()));

        std::fs::create_dir_all(&dir).unwrap();

        let destination = dir.join("theme.css");
        let backup = dir.join("backups/original.css");

        // Create an existing configuration.
        std::fs::write(&destination, "original waybar theme").unwrap();

        // Load Cyberpunk.
        let theme =
            load_desktop_theme("../../themes", "cyberpunk").expect("Cyberpunk theme should load");

        // Install the new theme.
        install_waybar_theme(&theme, &destination, &backup)
            .expect("Waybar theme installation should succeed");

        // Verify that the original configuration was backed up.
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            "original waybar theme"
        );

        // Verify that the new theme was installed.
        let installed = std::fs::read_to_string(&destination).unwrap();

        assert!(installed.contains("#FF235A"));
        assert!(installed.contains("#00E6FF"));

        std::fs::remove_dir_all(dir).unwrap();
    }
}

pub mod render;
