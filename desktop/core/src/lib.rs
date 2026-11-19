use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub use config_manager::ConfigError as DesktopConfigError;
use config_manager::{ConfigError, backup_file, install_content};
pub use theme_engine::Theme;
pub use theme_engine::loader::{LoadedTheme, ThemeLoadError};

/// Location of the generated Waybar configuration and its original backup
/// inside an installation root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaybarPaths {
    pub destination: PathBuf,
    pub backup: PathBuf,
}

impl WaybarPaths {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();

        Self {
            destination: root.join("waybar.css"),
            backup: root.join("backups").join("waybar.css.original"),
        }
    }
}

/// Configuration destinations for the Waybar integration under `root`.
pub fn waybar_paths(root: impl AsRef<Path>) -> WaybarPaths {
    WaybarPaths::new(root)
}

/// Loads a desktop theme by name.
pub fn load_desktop_theme(
    themes_directory: impl AsRef<Path>,
    theme_name: &str,
) -> Result<LoadedTheme, ThemeLoadError> {
    theme_engine::loader::load_theme_by_name(themes_directory, theme_name)
}

/// Installs generated Waybar CSS into the selected destination.
///
/// The destination is backed up first, but only once: the very first original
/// configuration is preserved even when a theme is applied repeatedly.
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

/// Restores the original Waybar configuration from its backup.
pub fn restore_waybar_theme(
    destination: impl AsRef<Path>,
    backup: impl AsRef<Path>,
) -> Result<(), ConfigError> {
    config_manager::restore_file(backup, destination)
}

fn active_theme_file(root: &Path) -> PathBuf {
    root.join("state").join("active-theme")
}

/// Records the theme that is currently installed under `root`.
pub fn write_active_theme(root: impl AsRef<Path>, theme_id: &str) -> Result<(), ConfigError> {
    install_content(
        format!("{theme_id}\n").as_bytes(),
        active_theme_file(root.as_ref()),
    )
}

/// Reads the theme that is currently installed under `root`, if any.
pub fn read_active_theme(root: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(active_theme_file(root.as_ref()))
        .ok()
        .map(|contents| contents.trim().to_string())
        .filter(|contents| !contents.is_empty())
}

/// Clears the recorded active theme, for example after a restore.
pub fn clear_active_theme(root: impl AsRef<Path>) -> Result<(), ConfigError> {
    let path = active_theme_file(root.as_ref());

    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("my-linux-core-{}-{}", name, std::process::id()));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

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
    fn waybar_paths_live_under_the_root() {
        let paths = waybar_paths("/tmp/my-linux-root");

        assert_eq!(
            paths.destination,
            PathBuf::from("/tmp/my-linux-root/waybar.css")
        );
        assert_eq!(
            paths.backup,
            PathBuf::from("/tmp/my-linux-root/backups/waybar.css.original")
        );
    }

    #[test]
    fn installs_waybar_theme_and_creates_backup() {
        let dir = test_dir("waybar-install");

        let paths = waybar_paths(&dir);

        fs::write(&paths.destination, "original waybar theme").unwrap();

        let theme =
            load_desktop_theme("../../themes", "cyberpunk").expect("Cyberpunk theme should load");

        install_waybar_theme(&theme, &paths.destination, &paths.backup)
            .expect("Waybar theme installation should succeed");

        assert_eq!(
            fs::read_to_string(&paths.backup).unwrap(),
            "original waybar theme"
        );

        let installed = fs::read_to_string(&paths.destination).unwrap();

        assert!(installed.contains("#FF235A"));
        assert!(installed.contains("#00E6FF"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn repeated_installs_keep_the_first_original_backup() {
        let dir = test_dir("waybar-repeated");

        let paths = waybar_paths(&dir);

        fs::write(&paths.destination, "original waybar theme").unwrap();

        let cyberpunk =
            load_desktop_theme("../../themes", "cyberpunk").expect("Cyberpunk should load");
        let nature = load_desktop_theme("../../themes", "nature").expect("Nature should load");

        install_waybar_theme(&cyberpunk, &paths.destination, &paths.backup).unwrap();
        install_waybar_theme(&cyberpunk, &paths.destination, &paths.backup).unwrap();
        install_waybar_theme(&nature, &paths.destination, &paths.backup).unwrap();

        assert_eq!(
            fs::read_to_string(&paths.backup).unwrap(),
            "original waybar theme"
        );

        let installed = fs::read_to_string(&paths.destination).unwrap();

        assert!(installed.contains("#6EBE5A"));
        assert!(!installed.contains("#FF235A"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn restores_the_original_configuration() {
        let dir = test_dir("waybar-restore");

        let paths = waybar_paths(&dir);

        fs::write(&paths.destination, "original waybar theme").unwrap();

        let theme = load_desktop_theme("../../themes", "cyberpunk").expect("Cyberpunk should load");

        install_waybar_theme(&theme, &paths.destination, &paths.backup).unwrap();

        restore_waybar_theme(&paths.destination, &paths.backup).unwrap();

        assert_eq!(
            fs::read_to_string(&paths.destination).unwrap(),
            "original waybar theme"
        );
        assert!(paths.backup.exists());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn restoring_without_a_backup_is_an_error() {
        let dir = test_dir("waybar-restore-missing");

        let paths = waybar_paths(&dir);

        let result = restore_waybar_theme(&paths.destination, &paths.backup);

        assert!(matches!(result, Err(ConfigError::MissingBackup(_))));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn active_theme_round_trips_and_clears() {
        let dir = test_dir("active-theme");

        assert_eq!(read_active_theme(&dir), None);

        write_active_theme(&dir, "cyberpunk").unwrap();
        assert_eq!(read_active_theme(&dir).as_deref(), Some("cyberpunk"));

        clear_active_theme(&dir).unwrap();
        assert_eq!(read_active_theme(&dir), None);

        // Clearing twice is not an error.
        clear_active_theme(&dir).unwrap();

        fs::remove_dir_all(dir).unwrap();
    }
}

pub mod render;
