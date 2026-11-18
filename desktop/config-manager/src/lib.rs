use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigOperation {
    Install,
    Backup,
    Restore,
}

#[derive(Debug, Clone)]
pub struct ConfigFile {
    pub source: String,
    pub destination: String,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    BackupAlreadyExists(PathBuf),
}

impl From<io::Error> for ConfigError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Checks whether a path is non-empty.
pub fn validate_path(path: impl AsRef<Path>) -> bool {
    !path.as_ref().as_os_str().is_empty()
}

/// Creates a backup without overwriting an existing backup.
pub fn backup_file(
    original: impl AsRef<Path>,
    backup: impl AsRef<Path>,
) -> Result<(), ConfigError> {
    let original = original.as_ref();
    let backup = backup.as_ref();

    if backup.exists() {
        return Err(ConfigError::BackupAlreadyExists(backup.to_path_buf()));
    }

    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(original, backup)?;

    Ok(())
}
/// Installs a configuration file using an atomic replacement.

/// Installs a configuration file using a temporary file and atomic rename.
pub fn install_file(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<(), ConfigError> {
    let source = source.as_ref();
    let destination = destination.as_ref();

    let parent = destination.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;

    let file_name = destination
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let temporary = parent.join(format!(".{}.{}.tmp", file_name, std::process::id()));

    let result = (|| -> Result<(), ConfigError> {
        fs::copy(source, &temporary)?;

        // Rename is atomic when source and destination are on the same filesystem.
        fs::rename(&temporary, destination)?;

        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }

    result
}

/// Restores a file from its backup.
pub fn restore_file(
    backup: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<(), ConfigError> {
    let backup = backup.as_ref();
    let destination = destination.as_ref();

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(backup, destination)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_path() {
        assert!(validate_path("config/waybar/theme.css"));
    }

    #[test]
    fn rejects_empty_path() {
        assert!(!validate_path(""));
    }

    #[test]
    fn creates_backup() {
        let dir = std::env::temp_dir().join(format!("my-linux-backup-test-{}", std::process::id()));

        fs::create_dir_all(&dir).unwrap();

        let original = dir.join("original.css");
        let backup = dir.join("backup.css");

        fs::write(&original, "original theme").unwrap();

        backup_file(&original, &backup).unwrap();

        assert_eq!(fs::read_to_string(&backup).unwrap(), "original theme");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_backup() {
        let dir =
            std::env::temp_dir().join(format!("my-linux-backup-existing-{}", std::process::id()));

        fs::create_dir_all(&dir).unwrap();

        let original = dir.join("original.css");
        let backup = dir.join("backup.css");

        fs::write(&original, "new theme").unwrap();
        fs::write(&backup, "old backup").unwrap();

        let result = backup_file(&original, &backup);

        assert!(matches!(result, Err(ConfigError::BackupAlreadyExists(_))));

        assert_eq!(fs::read_to_string(&backup).unwrap(), "old backup");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn restores_backup() {
        let dir =
            std::env::temp_dir().join(format!("my-linux-restore-test-{}", std::process::id()));

        fs::create_dir_all(&dir).unwrap();

        let backup = dir.join("backup.css");
        let destination = dir.join("restored.css");

        fs::write(&backup, "original theme").unwrap();

        restore_file(&backup, &destination).unwrap();

        assert_eq!(fs::read_to_string(&destination).unwrap(), "original theme");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn installs_new_configuration() {
        let dir =
            std::env::temp_dir().join(format!("my-linux-install-test-{}", std::process::id()));

        fs::create_dir_all(&dir).unwrap();

        let source = dir.join("generated.css");
        let destination = dir.join("waybar/theme.css");

        fs::write(&source, "new cyberpunk theme").unwrap();

        install_file(&source, &destination).unwrap();

        assert_eq!(
            fs::read_to_string(&destination).unwrap(),
            "new cyberpunk theme"
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn replaces_existing_configuration() {
        let dir =
            std::env::temp_dir().join(format!("my-linux-replace-test-{}", std::process::id()));

        fs::create_dir_all(&dir).unwrap();

        let source = dir.join("generated.css");
        let destination = dir.join("theme.css");

        fs::write(&source, "new theme").unwrap();
        fs::write(&destination, "old theme").unwrap();

        install_file(&source, &destination).unwrap();

        assert_eq!(fs::read_to_string(&destination).unwrap(), "new theme");

        fs::remove_dir_all(dir).unwrap();
    }
}
