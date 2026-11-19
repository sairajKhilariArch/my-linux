//! Transaction-style configuration application.
//!
//! Every operation follows the same sequence:
//!
//! 1. generate the configuration content,
//! 2. validate it,
//! 3. write an installation journal,
//! 4. back up the original (once) and snapshot the current state,
//! 5. install every file atomically,
//! 6. commit the new state, or roll everything back on failure.
//!
//! The journal makes an interrupted operation recoverable: the next command
//! that touches the installation restores the snapshots first.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use config_manager::{
    ConfigError, backup_file, digest, install_content, install_file, remove_file_if_exists,
    snapshot_file,
};
use theme_engine::loader::LoadedTheme;

use crate::render::render_waybar_css;

/// Path of the generated Waybar configuration, relative to the installation root.
pub const WAYBAR_RELATIVE_PATH: &str = "waybar.css";

#[derive(Debug)]
pub enum ApplyError {
    Config(ConfigError),
    Io(io::Error),
    Validation(String),
    RollbackIncomplete(String),
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyError::Config(error) => write!(f, "{error}"),
            ApplyError::Io(error) => write!(f, "I/O error: {error}"),
            ApplyError::Validation(error) => {
                write!(f, "Generated configuration is invalid: {error}")
            }
            ApplyError::RollbackIncomplete(error) => write!(
                f,
                "The operation failed and the rollback did not complete: {error}. Run `my-linux-theme doctor`."
            ),
        }
    }
}

impl std::error::Error for ApplyError {}

impl From<ConfigError> for ApplyError {
    fn from(error: ConfigError) -> Self {
        ApplyError::Config(error)
    }
}

impl From<io::Error> for ApplyError {
    fn from(error: io::Error) -> Self {
        ApplyError::Io(error)
    }
}

#[derive(Debug, Clone)]
pub struct ApplyReport {
    pub theme_id: String,
    pub installed_files: Vec<String>,
    /// Full paths of backups that were created by this operation.
    pub created_backups: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledEntry {
    pub relative_path: String,
    pub digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Unchanged,
    Modified,
    Missing,
}

impl FileState {
    pub const fn label(self) -> &'static str {
        match self {
            FileState::Unchanged => "unmodified",
            FileState::Modified => "modified by you",
            FileState::Missing => "missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modification {
    pub relative_path: String,
    pub state: FileState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub timestamp: u64,
    pub action: String,
    pub theme: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub label: String,
    pub detail: String,
    pub ok: bool,
}

struct Prepared {
    relative_path: String,
    content: Vec<u8>,
}

/// Destination of a generated configuration inside the installation root.
pub fn destination(root: impl AsRef<Path>, relative_path: &str) -> PathBuf {
    root.as_ref().join(relative_path)
}

/// First original configuration. Created once and never overwritten.
pub fn original_backup(root: impl AsRef<Path>, relative_path: &str) -> PathBuf {
    root.as_ref()
        .join("backups")
        .join(format!("{relative_path}.original"))
}

/// State before the most recent operation, used by `rollback`.
pub fn previous_backup(root: impl AsRef<Path>, relative_path: &str) -> PathBuf {
    root.as_ref()
        .join("backups")
        .join(format!("{relative_path}.previous"))
}

fn state_file(root: &Path, name: &str) -> PathBuf {
    root.join("state").join(name)
}

fn installed_file(root: &Path) -> PathBuf {
    state_file(root, "installed.tsv")
}

fn journal_file(root: &Path) -> PathBuf {
    state_file(root, "journal.tsv")
}

fn history_file(root: &Path) -> PathBuf {
    state_file(root, "history.log")
}

fn previous_theme_file(root: &Path) -> PathBuf {
    state_file(root, "previous-theme")
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Formats a Unix timestamp as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn format_timestamp(timestamp: u64) -> String {
    fn civil_from_days(days: i64) -> (i64, u32, u32) {
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = (z - era * 146_097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let year = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;

        (if month <= 2 { year + 1 } else { year }, month, day)
    }

    let days = (timestamp / 86_400) as i64;
    let seconds = timestamp % 86_400;
    let (year, month, day) = civil_from_days(days);

    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds / 3600,
        (seconds % 3600) / 60,
        seconds % 60
    )
}

fn validate(relative_path: &str, content: &str) -> Result<(), ApplyError> {
    if content.trim().is_empty() {
        return Err(ApplyError::Validation(format!("{relative_path} is empty")));
    }

    if relative_path.ends_with(".css") && !content.contains("@define-color") {
        return Err(ApplyError::Validation(format!(
            "{relative_path} does not define any colours"
        )));
    }

    Ok(())
}

fn prepare(theme: &LoadedTheme) -> Result<Vec<Prepared>, ApplyError> {
    let css = render_waybar_css(theme);

    validate(WAYBAR_RELATIVE_PATH, &css)?;

    Ok(vec![Prepared {
        relative_path: WAYBAR_RELATIVE_PATH.to_string(),
        content: css.into_bytes(),
    }])
}

fn write_journal(root: &Path, entries: &[(String, bool)]) -> Result<(), ConfigError> {
    let mut content = String::new();

    for (relative_path, existed) in entries {
        content.push_str(relative_path);
        content.push('\t');
        content.push(if *existed { '1' } else { '0' });
        content.push('\n');
    }

    install_content(content.as_bytes(), journal_file(root))
}

fn read_journal(root: &Path) -> Vec<(String, bool)> {
    fs::read_to_string(journal_file(root))
        .map(|contents| {
            contents
                .lines()
                .filter_map(|line| {
                    let (path, existed) = line.split_once('\t')?;

                    if path.is_empty() {
                        return None;
                    }

                    Some((path.to_string(), existed == "1"))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Restores the state recorded in the journal. Used both for a failed
/// operation and for an installation interrupted by a crash.
fn rollback_journal(root: &Path) -> Result<Vec<String>, ConfigError> {
    let mut restored = Vec::new();

    for (relative_path, existed_before) in read_journal(root) {
        let previous = previous_backup(root, &relative_path);
        let target = destination(root, &relative_path);

        if previous.exists() {
            install_file(&previous, &target)?;
            restored.push(relative_path);
        } else if !existed_before {
            remove_file_if_exists(&target)?;
            restored.push(relative_path);
        }
    }

    Ok(restored)
}

/// Ends a previously interrupted operation, if one is recorded.
pub fn recover_interrupted(root: impl AsRef<Path>) -> Result<Vec<String>, ApplyError> {
    let root = root.as_ref();

    if !journal_file(root).exists() {
        return Ok(Vec::new());
    }

    let restored = rollback_journal(root)?;

    remove_file_if_exists(journal_file(root))?;

    append_history(root, "recover", "-", &restored)?;

    Ok(restored)
}

fn run_transaction(root: &Path, prepared: &[Prepared]) -> Result<Vec<String>, ApplyError> {
    let entries: Vec<(String, bool)> = prepared
        .iter()
        .map(|file| {
            (
                file.relative_path.clone(),
                destination(root, &file.relative_path).exists(),
            )
        })
        .collect();

    write_journal(root, &entries)?;

    let mut created_backups = Vec::new();

    let outcome = (|| -> Result<(), ApplyError> {
        for file in prepared {
            let target = destination(root, &file.relative_path);
            let original = original_backup(root, &file.relative_path);

            if target.exists() {
                if !original.exists() {
                    backup_file(&target, &original)?;
                    created_backups.push(original.display().to_string());
                }

                snapshot_file(&target, previous_backup(root, &file.relative_path))?;
            }

            install_content(&file.content, &target)?;
        }

        Ok(())
    })();

    match outcome {
        Ok(()) => {
            remove_file_if_exists(journal_file(root))?;

            Ok(created_backups)
        }
        Err(error) => {
            let recovery = rollback_journal(root);
            let _ = remove_file_if_exists(journal_file(root));

            match recovery {
                Ok(_) => Err(error),
                Err(recovery_error) => {
                    Err(ApplyError::RollbackIncomplete(recovery_error.to_string()))
                }
            }
        }
    }
}

fn write_installed(root: &Path, prepared: &[Prepared]) -> Result<(), ConfigError> {
    let mut content = String::new();

    for file in prepared {
        content.push_str(&file.relative_path);
        content.push('\t');
        content.push_str(&digest(&file.content));
        content.push('\n');
    }

    install_content(content.as_bytes(), installed_file(root))
}

/// Files that the current installation is responsible for.
pub fn installed_entries(root: impl AsRef<Path>) -> Vec<InstalledEntry> {
    fs::read_to_string(installed_file(root.as_ref()))
        .map(|contents| {
            contents
                .lines()
                .filter_map(|line| {
                    let (path, digest) = line.split_once('\t')?;

                    if path.is_empty() || digest.is_empty() {
                        return None;
                    }

                    Some(InstalledEntry {
                        relative_path: path.to_string(),
                        digest: digest.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Reports configuration files that were changed or removed after installation.
pub fn detect_modifications(root: impl AsRef<Path>) -> Vec<Modification> {
    let root = root.as_ref();

    installed_entries(root)
        .into_iter()
        .map(|entry| {
            let state = match fs::read(destination(root, &entry.relative_path)) {
                Ok(content) if digest(&content) == entry.digest => FileState::Unchanged,
                Ok(_) => FileState::Modified,
                Err(_) => FileState::Missing,
            };

            Modification {
                relative_path: entry.relative_path,
                state,
            }
        })
        .collect()
}

fn append_history(
    root: &Path,
    action: &str,
    theme: &str,
    files: &[String],
) -> Result<(), ConfigError> {
    let mut line = format!(
        "{}\t{}\t{}\t{}\n",
        timestamp(),
        action,
        theme,
        files.join(",")
    );

    if let Ok(existing) = fs::read_to_string(history_file(root)) {
        line = format!("{existing}{line}");
    }

    install_content(line.as_bytes(), history_file(root))
}

/// Every recorded installation action, oldest first.
pub fn read_history(root: impl AsRef<Path>) -> Vec<HistoryEntry> {
    fs::read_to_string(history_file(root.as_ref()))
        .map(|contents| {
            contents
                .lines()
                .filter_map(|line| {
                    let mut fields = line.split('\t');

                    let timestamp = fields.next()?.parse().ok()?;
                    let action = fields.next()?.to_string();
                    let theme = fields.next()?.to_string();
                    let files = fields
                        .next()
                        .unwrap_or_default()
                        .split(',')
                        .filter(|file| !file.is_empty() && *file != "-")
                        .map(str::to_string)
                        .collect();

                    Some(HistoryEntry {
                        timestamp,
                        action,
                        theme,
                        files,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn relatives_to_restore(root: &Path) -> Vec<String> {
    let installed: Vec<String> = installed_entries(root)
        .into_iter()
        .map(|entry| entry.relative_path)
        .collect();

    if installed.is_empty() {
        vec![WAYBAR_RELATIVE_PATH.to_string()]
    } else {
        installed
    }
}

/// Applies a theme as a single transaction.
pub fn apply_theme(root: impl AsRef<Path>, theme: &LoadedTheme) -> Result<ApplyReport, ApplyError> {
    let root = root.as_ref();

    fs::create_dir_all(root)?;
    recover_interrupted(root)?;

    let prepared = prepare(theme)?;
    let previous_theme = crate::read_active_theme(root);

    let created_backups = run_transaction(root, &prepared)?;

    write_installed(root, &prepared)?;

    if let Some(previous_theme) = previous_theme {
        install_content(
            format!("{previous_theme}\n").as_bytes(),
            previous_theme_file(root),
        )?;
    }

    crate::write_active_theme(root, &theme.metadata.id)?;

    let files: Vec<String> = prepared
        .iter()
        .map(|file| file.relative_path.clone())
        .collect();

    append_history(root, "apply", &theme.metadata.id, &files)?;

    Ok(ApplyReport {
        theme_id: theme.metadata.id.clone(),
        installed_files: files,
        created_backups,
    })
}

/// Restores every installed configuration from its original backup.
pub fn restore_theme(root: impl AsRef<Path>) -> Result<Vec<String>, ApplyError> {
    let root = root.as_ref();

    recover_interrupted(root)?;

    let relatives = relatives_to_restore(root);
    let mut prepared = Vec::new();

    for relative_path in &relatives {
        let original = original_backup(root, relative_path);

        if !original.exists() {
            return Err(ApplyError::Config(ConfigError::MissingBackup(original)));
        }

        prepared.push(Prepared {
            relative_path: relative_path.clone(),
            content: fs::read(&original)?,
        });
    }

    run_transaction(root, &prepared)?;

    remove_file_if_exists(installed_file(root))?;
    crate::clear_active_theme(root)?;
    remove_file_if_exists(previous_theme_file(root))?;

    append_history(root, "restore", "-", &relatives)?;

    Ok(relatives)
}

/// Reverts the most recent applied theme to the state before it.
pub fn rollback_theme(root: impl AsRef<Path>) -> Result<Vec<String>, ApplyError> {
    let root = root.as_ref();

    if journal_file(root).exists() {
        return recover_interrupted(root);
    }

    let relatives = relatives_to_restore(root);
    let mut prepared = Vec::new();

    for relative_path in &relatives {
        let previous = previous_backup(root, relative_path);

        if !previous.exists() {
            return Err(ApplyError::Config(ConfigError::MissingBackup(previous)));
        }

        prepared.push(Prepared {
            relative_path: relative_path.clone(),
            content: fs::read(&previous)?,
        });
    }

    run_transaction(root, &prepared)?;

    let previous_theme = fs::read_to_string(previous_theme_file(root))
        .ok()
        .map(|contents| contents.trim().to_string())
        .filter(|contents| !contents.is_empty());

    match previous_theme {
        Some(theme) => crate::write_active_theme(root, &theme)?,
        None => crate::clear_active_theme(root)?,
    }

    // The restored files are the new installed state, so modification
    // detection keeps working after a rollback.
    write_installed(root, &prepared)?;
    remove_file_if_exists(previous_theme_file(root))?;

    append_history(root, "rollback", "-", &relatives)?;

    Ok(relatives)
}

/// Diagnostics for the installation root.
pub fn diagnose(root: impl AsRef<Path>, themes_directory: impl AsRef<Path>) -> Vec<Diagnostic> {
    let root = root.as_ref();
    let themes_directory = themes_directory.as_ref();

    let mut diagnostics = Vec::new();

    diagnostics.push(Diagnostic {
        label: "Installation root".to_string(),
        detail: root.display().to_string(),
        ok: root.is_dir(),
    });

    diagnostics.push(Diagnostic {
        label: "Themes directory".to_string(),
        detail: themes_directory.display().to_string(),
        ok: themes_directory.is_dir(),
    });

    for theme in crate::Theme::all() {
        let name = theme.name().to_lowercase();

        match crate::load_desktop_theme(themes_directory, &name) {
            Ok(loaded) => diagnostics.push(Diagnostic {
                label: format!("Theme {name}"),
                detail: format!("{} v{}", loaded.metadata.name, loaded.metadata.version),
                ok: true,
            }),
            Err(error) => diagnostics.push(Diagnostic {
                label: format!("Theme {name}"),
                detail: error.to_string(),
                ok: false,
            }),
        }
    }

    let active = crate::read_active_theme(root);

    diagnostics.push(Diagnostic {
        label: "Active theme".to_string(),
        detail: active.clone().unwrap_or_else(|| "none".to_string()),
        ok: active
            .as_deref()
            .map(|name| crate::load_desktop_theme(themes_directory, name).is_ok())
            .unwrap_or(true),
    });

    diagnostics.push(Diagnostic {
        label: "Interrupted operation".to_string(),
        detail: if journal_file(root).exists() {
            "journal found; run `my-linux-theme rollback`".to_string()
        } else {
            "none".to_string()
        },
        ok: !journal_file(root).exists(),
    });

    let modifications = detect_modifications(root);

    if modifications.is_empty() {
        diagnostics.push(Diagnostic {
            label: "Installed configuration".to_string(),
            detail: "no generated configuration recorded".to_string(),
            ok: true,
        });
    }

    for modification in modifications {
        let state = modification.state;

        diagnostics.push(Diagnostic {
            label: format!("File {}", modification.relative_path),
            detail: format!(
                "{} ({})",
                state.label(),
                destination(root, &modification.relative_path).display()
            ),
            ok: state == FileState::Unchanged,
        });
    }

    let original = original_backup(root, WAYBAR_RELATIVE_PATH);

    diagnostics.push(Diagnostic {
        label: "Original backup".to_string(),
        detail: if original.exists() {
            original.display().to_string()
        } else {
            "not created yet".to_string()
        },
        ok: true,
    });

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "my-linux-transaction-{}-{}",
            name,
            std::process::id()
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    fn theme(name: &str) -> LoadedTheme {
        crate::load_desktop_theme("../../themes", name).expect("theme should load")
    }

    #[test]
    fn applies_a_theme_and_records_state() {
        let dir = test_dir("apply");
        fs::write(destination(&dir, WAYBAR_RELATIVE_PATH), "original").unwrap();

        let report = apply_theme(&dir, &theme("cyberpunk")).unwrap();

        assert_eq!(report.theme_id, "cyberpunk");
        assert_eq!(report.installed_files, vec!["waybar.css"]);
        assert_eq!(report.created_backups.len(), 1);

        assert_eq!(
            fs::read_to_string(original_backup(&dir, WAYBAR_RELATIVE_PATH)).unwrap(),
            "original"
        );
        assert_eq!(crate::read_active_theme(&dir).as_deref(), Some("cyberpunk"));
        assert_eq!(installed_entries(&dir).len(), 1);
        assert_eq!(read_history(&dir).len(), 1);
        assert!(!journal_file(&dir).exists());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rollback_returns_to_the_previous_theme() {
        let dir = test_dir("rollback");
        fs::write(destination(&dir, WAYBAR_RELATIVE_PATH), "original").unwrap();

        apply_theme(&dir, &theme("cyberpunk")).unwrap();
        let cyberpunk = fs::read_to_string(destination(&dir, WAYBAR_RELATIVE_PATH)).unwrap();

        apply_theme(&dir, &theme("nature")).unwrap();
        rollback_theme(&dir).unwrap();

        assert_eq!(
            fs::read_to_string(destination(&dir, WAYBAR_RELATIVE_PATH)).unwrap(),
            cyberpunk
        );
        assert_eq!(crate::read_active_theme(&dir).as_deref(), Some("cyberpunk"));
        assert_eq!(
            fs::read_to_string(original_backup(&dir, WAYBAR_RELATIVE_PATH)).unwrap(),
            "original"
        );
        assert_eq!(detect_modifications(&dir)[0].state, FileState::Unchanged);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rollback_without_previous_state_is_an_error() {
        let dir = test_dir("rollback-missing");

        let result = rollback_theme(&dir);

        assert!(matches!(result, Err(ApplyError::Config(_))));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn restore_returns_the_original_and_clears_state() {
        let dir = test_dir("restore");
        fs::write(destination(&dir, WAYBAR_RELATIVE_PATH), "original").unwrap();

        apply_theme(&dir, &theme("cyberpunk")).unwrap();
        apply_theme(&dir, &theme("nature")).unwrap();

        restore_theme(&dir).unwrap();

        assert_eq!(
            fs::read_to_string(destination(&dir, WAYBAR_RELATIVE_PATH)).unwrap(),
            "original"
        );
        assert_eq!(crate::read_active_theme(&dir), None);
        assert!(installed_entries(&dir).is_empty());
        assert_eq!(read_history(&dir).len(), 3);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_operation_leaves_the_installation_untouched() {
        let dir = test_dir("failed");
        let target = destination(&dir, WAYBAR_RELATIVE_PATH);

        // Reading the previous configuration fails when it is a directory,
        // which forces a failure after the journal was written.
        fs::create_dir_all(target.join("nested")).unwrap();
        fs::write(target.join("nested/keep.css"), "keep me").unwrap();

        let result = apply_theme(&dir, &theme("cyberpunk"));

        assert!(result.is_err());
        assert!(target.is_dir());
        assert_eq!(
            fs::read_to_string(target.join("nested/keep.css")).unwrap(),
            "keep me"
        );
        assert!(!journal_file(&dir).exists());
        assert!(installed_entries(&dir).is_empty());
        assert!(!states_committed(&dir));

        fs::remove_dir_all(dir).unwrap();
    }

    fn states_committed(root: &Path) -> bool {
        crate::read_active_theme(root).is_some() || !read_history(root).is_empty()
    }

    #[test]
    fn interrupted_operation_is_recovered() {
        let dir = test_dir("recover");
        let target = destination(&dir, WAYBAR_RELATIVE_PATH);
        fs::write(&target, "original").unwrap();

        fs::create_dir_all(dir.join("state")).unwrap();
        fs::create_dir_all(dir.join("backups")).unwrap();
        fs::write(previous_backup(&dir, WAYBAR_RELATIVE_PATH), "original").unwrap();
        fs::write(journal_file(&dir), "waybar.css\t1\n").unwrap();
        fs::write(&target, "half written").unwrap();

        let restored = recover_interrupted(&dir).unwrap();

        assert_eq!(restored, vec!["waybar.css"]);
        assert_eq!(fs::read_to_string(&target).unwrap(), "original");
        assert!(!journal_file(&dir).exists());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn detects_modified_configuration() {
        let dir = test_dir("modified");
        fs::write(destination(&dir, WAYBAR_RELATIVE_PATH), "original").unwrap();

        apply_theme(&dir, &theme("cyberpunk")).unwrap();

        assert_eq!(detect_modifications(&dir)[0].state, FileState::Unchanged);

        fs::write(
            destination(&dir, WAYBAR_RELATIVE_PATH),
            "/* my hand edited theme */",
        )
        .unwrap();

        assert_eq!(detect_modifications(&dir)[0].state, FileState::Modified);

        fs::remove_file(destination(&dir, WAYBAR_RELATIVE_PATH)).unwrap();
        assert_eq!(detect_modifications(&dir)[0].state, FileState::Missing);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn validates_generated_configuration() {
        assert!(validate("waybar.css", "/* empty */").is_err());
        assert!(validate("waybar.css", "@define-color main-bg #0C0812;").is_ok());
    }

    #[test]
    fn formats_timestamps() {
        assert_eq!(format_timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_timestamp(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn doctor_reports_a_healthy_installation() {
        let dir = test_dir("doctor");
        fs::write(destination(&dir, WAYBAR_RELATIVE_PATH), "original").unwrap();

        apply_theme(&dir, &theme("cyberpunk")).unwrap();

        let diagnostics = diagnose(&dir, "../../themes");

        let failures: Vec<&Diagnostic> = diagnostics.iter().filter(|item| !item.ok).collect();

        assert!(failures.is_empty(), "unexpected problems: {failures:?}");

        fs::remove_dir_all(dir).unwrap();
    }
}
