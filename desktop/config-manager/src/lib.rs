use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
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
    MissingBackup(PathBuf),
    InvalidPath(PathBuf),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(error) => write!(f, "I/O error: {}", error),
            ConfigError::BackupAlreadyExists(path) => {
                write!(f, "Backup already exists: {}", path.display())
            }
            ConfigError::MissingBackup(path) => {
                write!(
                    f,
                    "No backup found at {}; nothing to restore",
                    path.display()
                )
            }
            ConfigError::InvalidPath(path) => {
                write!(f, "Invalid path: {}", path.display())
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Checks whether a path is safe to use as a configuration target.
///
/// Empty paths and paths that traverse upwards (`..`) are rejected so that a
/// theme can never write outside the destination it was given.
pub fn validate_path(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();

    if path.as_os_str().is_empty() {
        return false;
    }

    !path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
}

/// Resolves a symbolic link destination to its target so that installing a
/// configuration never replaces the link with a regular file.
fn resolve_destination(destination: &Path) -> Result<PathBuf, ConfigError> {
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_symlink() => Ok(fs::canonicalize(destination)?),
        Ok(_) => Ok(destination.to_path_buf()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(destination.to_path_buf()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(unix)]
fn permissions_of(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions().mode() & 0o7777)
}

#[cfg(not(unix))]
fn permissions_of(_path: &Path) -> Option<u32> {
    None
}

fn apply_permissions(path: &Path, mode: Option<u32>) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if let Some(mode) = mode {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
        }
    }

    #[cfg(not(unix))]
    let _ = (path, mode);
}

/// Creates a unique temporary file next to `destination`.
fn create_temporary(
    parent: &Path,
    name: &str,
    suffix: &str,
) -> Result<(PathBuf, File), ConfigError> {
    loop {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

        let temporary = parent.join(format!(
            ".{}.{}.{}.{}.tmp",
            name,
            std::process::id(),
            counter,
            suffix
        ));

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

/// Creates a backup without overwriting an existing backup.
///
/// The backup is first written to a unique temporary file. A hard link then
/// publishes it atomically and fails if a backup already exists, so the very
/// first original configuration is never lost.
pub fn backup_file(
    original: impl AsRef<Path>,
    backup: impl AsRef<Path>,
) -> Result<(), ConfigError> {
    let original = original.as_ref();
    let backup = backup.as_ref();

    if !validate_path(original) {
        return Err(ConfigError::InvalidPath(original.to_path_buf()));
    }

    if !validate_path(backup) {
        return Err(ConfigError::InvalidPath(backup.to_path_buf()));
    }

    let parent = backup.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;

    let file_name = backup.file_name().unwrap_or_default().to_string_lossy();
    let original_mode = permissions_of(original);

    let mut source_file = File::open(original)?;

    let (temporary, mut temporary_file) = create_temporary(parent, &file_name, "backup")?;

    let result = (|| -> Result<(), ConfigError> {
        io::copy(&mut source_file, &mut temporary_file)?;
        temporary_file.sync_all()?;

        drop(temporary_file);

        apply_permissions(&temporary, original_mode);

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

/// Installs a configuration file using a temporary file and atomic rename.
pub fn install_file(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<(), ConfigError> {
    let source = source.as_ref();

    if !validate_path(source) {
        return Err(ConfigError::InvalidPath(source.to_path_buf()));
    }

    let content = fs::read(source)?;

    install_content(&content, destination)
}

/// Restores a file from its backup using atomic installation.
pub fn restore_file(
    backup: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<(), ConfigError> {
    let backup = backup.as_ref();

    if !validate_path(backup) {
        return Err(ConfigError::InvalidPath(backup.to_path_buf()));
    }

    if !backup.exists() {
        return Err(ConfigError::MissingBackup(backup.to_path_buf()));
    }

    let content = fs::read(backup)?;

    install_content(&content, destination)
}

/// Installs text or binary content using a unique temporary file and atomic
/// rename, preserving the permissions of an existing destination.
pub fn install_content(content: &[u8], destination: impl AsRef<Path>) -> Result<(), ConfigError> {
    let requested = destination.as_ref();

    if !validate_path(requested) {
        return Err(ConfigError::InvalidPath(requested.to_path_buf()));
    }

    let destination = resolve_destination(requested)?;

    let parent = destination.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;

    let existing_mode = permissions_of(&destination);

    let file_name = destination
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let (temporary, mut temporary_file) = create_temporary(parent, &file_name, "install")?;

    let result = (|| -> Result<(), ConfigError> {
        temporary_file.write_all(content)?;
        temporary_file.sync_all()?;
        drop(temporary_file);

        apply_permissions(&temporary, existing_mode);

        fs::rename(&temporary, &destination)?;

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

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "my-linux-{}-{}-{}",
            name,
            std::process::id(),
            TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));

        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn accepts_valid_path() {
        assert!(validate_path("config/waybar/theme.css"));
    }

    #[test]
    fn rejects_empty_path() {
        assert!(!validate_path(""));
    }

    #[test]
    fn rejects_parent_directory_traversal() {
        assert!(!validate_path("../outside.css"));
        assert!(!validate_path("config/../../outside.css"));
    }

    #[test]
    fn creates_backup() {
        let dir = test_dir("backup");

        let original = dir.join("original.css");
        let backup = dir.join("backup.css");

        fs::write(&original, "original theme").unwrap();

        backup_file(&original, &backup).unwrap();

        assert_eq!(fs::read_to_string(&backup).unwrap(), "original theme");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_backup() {
        let dir = test_dir("backup-existing");

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
    fn backup_of_missing_file_fails_without_creating_backup() {
        let dir = test_dir("backup-missing");

        let original = dir.join("missing.css");
        let backup = dir.join("backup.css");

        let result = backup_file(&original, &backup);

        assert!(matches!(result, Err(ConfigError::Io(_))));
        assert!(!backup.exists());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn restores_backup() {
        let dir = test_dir("restore");

        let backup = dir.join("backup.css");
        let destination = dir.join("restored.css");

        fs::write(&backup, "original theme").unwrap();

        restore_file(&backup, &destination).unwrap();

        assert_eq!(fs::read_to_string(&destination).unwrap(), "original theme");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn restoring_missing_backup_fails() {
        let dir = test_dir("restore-missing");

        let backup = dir.join("backup.css");
        let destination = dir.join("theme.css");

        let result = restore_file(&backup, &destination);

        assert!(matches!(result, Err(ConfigError::MissingBackup(_))));
        assert!(!destination.exists());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn installs_new_configuration() {
        let dir = test_dir("install");

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
        let dir = test_dir("replace");

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
        let dir = test_dir("content");

        let destination = dir.join("theme.css");

        install_content(b"Cyberpunk theme", &destination).unwrap();

        assert_eq!(fs::read_to_string(&destination).unwrap(), "Cyberpunk theme");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_install_into_invalid_path() {
        let dir = test_dir("invalid-destination");

        let destination = dir.join("../escaped.css");

        let result = install_content(b"content", &destination);

        assert!(matches!(result, Err(ConfigError::InvalidPath(_))));
        assert!(!dir.join("../escaped.css").exists());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_install_leaves_existing_content_untouched() {
        let dir = test_dir("failed-install");

        // A directory can never be replaced by a file rename, which is a
        // reliable way to force an installation failure.
        let destination = dir.join("theme.css");
        fs::create_dir_all(destination.join("nested")).unwrap();
        fs::write(destination.join("nested/keep.css"), "keep me").unwrap();

        let result = install_content(b"new theme", &destination);

        assert!(result.is_err());
        assert!(destination.is_dir());
        assert_eq!(
            fs::read_to_string(destination.join("nested/keep.css")).unwrap(),
            "keep me"
        );

        // No temporary file may be left behind.
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().ends_with(".tmp"))
            .collect();

        assert!(leftovers.is_empty());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_install_reports_missing_source() {
        let dir = test_dir("missing-source");

        let result = install_file(dir.join("missing.css"), dir.join("theme.css"));

        assert!(matches!(result, Err(ConfigError::Io(_))));
        assert!(!dir.join("theme.css").exists());

        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn preserves_permissions_when_replacing_configuration() {
        use std::os::unix::fs::PermissionsExt;

        let dir = test_dir("permissions");

        let destination = dir.join("theme.css");
        fs::write(&destination, "old theme").unwrap();
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o640)).unwrap();

        install_content(b"new theme", &destination).unwrap();

        let mode = fs::metadata(&destination).unwrap().permissions().mode() & 0o777;

        assert_eq!(mode, 0o640);
        assert_eq!(fs::read_to_string(&destination).unwrap(), "new theme");

        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn backup_preserves_source_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = test_dir("backup-permissions");

        let original = dir.join("original.css");
        let backup = dir.join("backup.css");

        fs::write(&original, "original").unwrap();
        fs::set_permissions(&original, fs::Permissions::from_mode(0o600)).unwrap();

        backup_file(&original, &backup).unwrap();

        let mode = fs::metadata(&backup).unwrap().permissions().mode() & 0o777;

        assert_eq!(mode, 0o600);

        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn installs_through_symlink_without_replacing_the_link() {
        use std::os::unix::fs::symlink;

        let dir = test_dir("symlink");

        let real = dir.join("dotfiles/theme.css");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        fs::write(&real, "original theme").unwrap();

        let link = dir.join("theme.css");
        symlink(&real, &link).unwrap();

        install_content(b"new theme", &link).unwrap();

        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&real).unwrap(), "new theme");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn concurrent_backups_do_not_overwrite_each_other() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let dir = test_dir("concurrent-backup");

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
