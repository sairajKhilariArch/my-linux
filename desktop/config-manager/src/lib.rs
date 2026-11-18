use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

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

/// Creates a backup without overwriting an existing backup.
///
/// The backup is first written to a unique temporary file.
/// A hard link publishes it atomically without replacing an existing file.
pub fn backup_file(
    original: impl AsRef<Path>,
    backup: impl AsRef<Path>,
) -> Result<(), ConfigError> {
    let original = original.as_ref();
    let backup = backup.as_ref();

    let parent = backup.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;

    let file_name = backup.file_name().unwrap_or_default().to_string_lossy();

    let mut source_file = File::open(original)?;

    let (temporary, mut temporary_file) = loop {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

        let temporary = parent.join(format!(
            ".{}.{}.{}.backup.tmp",
            file_name,
            std::process::id(),
            counter
        ));

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),

            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                continue;
            }

            Err(error) => return Err(error.into()),
        }
    };

    let result = (|| -> Result<(), ConfigError> {
        io::copy(&mut source_file, &mut temporary_file)?;
        temporary_file.sync_all()?;

        drop(temporary_file);

        match fs::hard_link(&temporary, backup) {
            Ok(()) => Ok(()),

            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                Err(ConfigError::BackupAlreadyExists(backup.to_path_buf()))
            }

            Err(error) => Err(error.into()),
        }
    })();

    let _ = fs::remove_file(&temporary);

    result
}

/// Installs a configuration file using an atomic replacement.

/// Installs a configuration file using a temporary file and atomic rename.

/// Installs a configuration file using a unique temporary file
/// and atomic rename.
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

    let source_file = File::open(source)?;

    let (temporary, mut temporary_file) = loop {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

        let temporary = parent.join(format!(
            ".{}.{}.{}.tmp",
            file_name,
            std::process::id(),
            counter
        ));

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),

            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                continue;
            }

            Err(error) => return Err(error.into()),
        }
    };

    let result = (|| -> Result<(), ConfigError> {
        let mut source_file = source_file;

        io::copy(&mut source_file, &mut temporary_file)?;
        temporary_file.sync_all()?;

        drop(temporary_file);

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

/// Installs text or binary content using a unique temporary file
/// and atomic rename.
pub fn install_content(content: &[u8], destination: impl AsRef<Path>) -> Result<(), ConfigError> {
    let destination = destination.as_ref();

    let parent = destination.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;

    let file_name = destination
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let (temporary, mut temporary_file) = loop {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

        let temporary = parent.join(format!(
            ".{}.{}.{}.tmp",
            file_name,
            std::process::id(),
            counter
        ));

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    };

    let result = (|| -> Result<(), ConfigError> {
        temporary_file.write_all(content)?;
        temporary_file.sync_all()?;
        drop(temporary_file);

        fs::rename(&temporary, destination)?;

        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }

    result
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

    #[test]
    fn installs_content_directly() {
        let dir =
            std::env::temp_dir().join(format!("my-linux-content-test-{}", std::process::id()));

        fs::create_dir_all(&dir).unwrap();

        let destination = dir.join("theme.css");

        install_content(b"Cyberpunk theme", &destination).unwrap();

        assert_eq!(fs::read_to_string(&destination).unwrap(), "Cyberpunk theme");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn concurrent_backups_do_not_overwrite_each_other() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let dir =
            std::env::temp_dir().join(format!("my-linux-concurrent-backup-{}", std::process::id()));

        fs::create_dir_all(&dir).unwrap();

        let original_a = dir.join("original-a.css");
        let original_b = dir.join("original-b.css");
        let backup = dir.join("backup.css");

        fs::write(&original_a, "theme A").unwrap();
        fs::write(&original_b, "theme B").unwrap();

        let barrier = Arc::new(Barrier::new(2));

        let handle_a = {
            let barrier = Arc::clone(&barrier);
            let original = original_a.clone();
            let backup = backup.clone();

            thread::spawn(move || {
                barrier.wait();
                backup_file(original, backup)
            })
        };

        let handle_b = {
            let barrier = Arc::clone(&barrier);
            let original = original_b.clone();
            let backup = backup.clone();

            thread::spawn(move || {
                barrier.wait();
                backup_file(original, backup)
            })
        };

        let result_a = handle_a.join().unwrap();
        let result_b = handle_b.join().unwrap();

        assert_ne!(result_a.is_ok(), result_b.is_ok());

        assert!(matches!(
            if result_a.is_err() {
                result_a
            } else {
                result_b
            },
            Err(ConfigError::BackupAlreadyExists(_))
        ));

        let saved = fs::read_to_string(&backup).unwrap();

        assert!(saved == "theme A" || saved == "theme B");

        fs::remove_dir_all(dir).unwrap();
    }
}
