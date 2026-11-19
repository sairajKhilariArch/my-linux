//! Transaction-style configuration application.
//!
//! Every operation follows the same sequence:
//!
//! 1. render every selected integration,
//! 2. validate the generated files,
//! 3. write an installation journal,
//! 4. back up the original (once) and snapshot the current state,
//! 5. install (or remove) every file atomically,
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
use desktop_integrations::Integration;
use theme_engine::LoadedTheme;

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
            ApplyError::Validation(error) => write!(f, "{error}"),
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

#[derive(Debug)]
struct Prepared {
    relative_path: String,
    /// `None` removes the generated file (a file that had no original).
    content: Option<Vec<u8>>,
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

fn prepare(
    theme: &LoadedTheme,
    selected: &[&'static dyn Integration],
    extra: &[(String, String)],
) -> Result<Vec<Prepared>, ApplyError> {
    let mut prepared = Vec::new();

    for integration in selected {
        let content = integration.render(theme);

        integration.validate(&content).map_err(|error| {
            ApplyError::Validation(format!(
                "Generated {} configuration is invalid: {error}",
                integration.id()
            ))
        })?;

        prepared.push(Prepared {
            relative_path: integration.relative_path().to_string(),
            content: Some(content.into_bytes()),
        });
    }

    for (relative_path, content) in extra {
        if relative_path.trim().is_empty() || content.trim().is_empty() {
            return Err(ApplyError::Validation(format!(
                "Generated `{relative_path}` is empty"
            )));
        }

        prepared.push(Prepared {
            relative_path: relative_path.clone(),
            content: Some(content.clone().into_bytes()),
        });
    }

    if prepared.is_empty() {
        return Err(ApplyError::Validation(
            "No integrations were selected".to_string(),
        ));
    }

    Ok(prepared)
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

    // Files we installed ourselves are not user originals, so a later apply
    // must never turn a generated configuration into an "original" backup.
    let ours: Vec<String> = installed_entries(root)
        .into_iter()
        .map(|entry| entry.relative_path)
        .collect();

    let outcome = (|| -> Result<(), ApplyError> {
        for file in prepared {
            let target = destination(root, &file.relative_path);
            let original = original_backup(root, &file.relative_path);

            if target.exists() {
                if !original.exists() && !ours.contains(&file.relative_path) {
                    backup_file(&target, &original)?;
                    created_backups.push(original.display().to_string());
                }

                snapshot_file(&target, previous_backup(root, &file.relative_path))?;
            }

            match &file.content {
                Some(content) => install_content(content, &target)?,
                None => remove_file_if_exists(&target)?,
            }
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
        let Some(bytes) = &file.content else {
            continue;
        };

        content.push_str(&file.relative_path);
        content.push('\t');
        content.push_str(&digest(bytes));
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

/// Refuses to delete a generated file that the user changed and that has no
/// original backup to restore.
fn guard_local_changes(root: &Path, relative_path: &str) -> Result<(), ApplyError> {
    let target = destination(root, relative_path);

    if !target.exists() {
        return Ok(());
    }

    let recorded = installed_entries(root)
        .into_iter()
        .find(|entry| entry.relative_path == relative_path);

    let Some(entry) = recorded else {
        return Err(ApplyError::Validation(format!(
            "{} has no original backup and no recorded state; refusing to remove it",
            target.display()
        )));
    };

    let current = fs::read(&target)?;

    if digest(&current) != entry.digest {
        return Err(ApplyError::Validation(format!(
            "{} has been modified by you and has no original backup; refusing to remove it",
            target.display()
        )));
    }

    Ok(())
}

/// Applies a theme to every integration as a single transaction.
pub fn apply_theme(root: impl AsRef<Path>, theme: &LoadedTheme) -> Result<ApplyReport, ApplyError> {
    apply_theme_with(root, theme, desktop_integrations::all())
}

/// Applies a theme to the selected integrations as a single transaction.
pub fn apply_theme_with(
    root: impl AsRef<Path>,
    theme: &LoadedTheme,
    selected: &[&'static dyn Integration],
) -> Result<ApplyReport, ApplyError> {
    apply_theme_with_extra(root, theme, selected, &[])
}

/// Applies a theme plus additional generated files as one transaction.
///
/// Desktop profiles use this for the Hyprland layout snippet, so the snippet is
/// backed up, rolled back and restored exactly like integration files.
pub fn apply_theme_with_extra(
    root: impl AsRef<Path>,
    theme: &LoadedTheme,
    selected: &[&'static dyn Integration],
    extra: &[(String, String)],
) -> Result<ApplyReport, ApplyError> {
    let root = root.as_ref();

    fs::create_dir_all(root)?;
    recover_interrupted(root)?;

    let prepared = prepare(theme, selected, extra)?;
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
///
/// A generated file that never displaced a user file (no original backup) is
/// removed instead, but only when it is still unmodified.
pub fn restore_theme(root: impl AsRef<Path>) -> Result<Vec<String>, ApplyError> {
    let root = root.as_ref();

    recover_interrupted(root)?;

    let relatives = relatives_to_restore(root);
    let mut prepared = Vec::new();

    for relative_path in &relatives {
        let original = original_backup(root, relative_path);

        if original.exists() {
            prepared.push(Prepared {
                relative_path: relative_path.clone(),
                content: Some(fs::read(&original)?),
            });
        } else {
            guard_local_changes(root, relative_path)?;

            prepared.push(Prepared {
                relative_path: relative_path.clone(),
                content: None,
            });
        }
    }

    let nothing_to_restore = prepared
        .iter()
        .all(|file| file.content.is_none() && !destination(root, &file.relative_path).exists());

    if nothing_to_restore {
        return Err(ApplyError::Validation(
            "Nothing to restore: no original backup and no generated configuration".to_string(),
        ));
    }

    run_transaction(root, &prepared)?;

    remove_file_if_exists(installed_file(root))?;
    crate::clear_active_theme(root)?;
    crate::clear_active_profile(root)?;
    remove_file_if_exists(previous_theme_file(root))?;

    // The original state is now live, so the rollback snapshots are obsolete.
    for relative_path in &relatives {
        remove_file_if_exists(previous_backup(root, relative_path))?;
    }

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

        if previous.exists() {
            prepared.push(Prepared {
                relative_path: relative_path.clone(),
                content: Some(fs::read(&previous)?),
            });
        } else {
            guard_local_changes(root, relative_path)?;

            prepared.push(Prepared {
                relative_path: relative_path.clone(),
                content: None,
            });
        }
    }

    let nothing_to_do = prepared
        .iter()
        .all(|file| file.content.is_none() && !destination(root, &file.relative_path).exists());

    if nothing_to_do {
        return Err(ApplyError::Validation(
            "Nothing to roll back: no previous state is recorded".to_string(),
        ));
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

    let registry = crate::Registry::new(vec![themes_directory.to_path_buf()]);
    let discovery = registry.list();

    for theme in &discovery.themes {
        diagnostics.push(Diagnostic {
            label: format!("Theme {}", theme.id),
            detail: format!(
                "{} v{} ({})",
                theme.name,
                theme.version,
                theme.source.label()
            ),
            ok: true,
        });
    }

    for broken in &discovery.broken {
        diagnostics.push(Diagnostic {
            label: format!("Theme {}", broken.path.display()),
            detail: broken.error.clone(),
            ok: false,
        });
    }

    let active = crate::read_active_theme(root);

    diagnostics.push(Diagnostic {
        label: "Active theme".to_string(),
        detail: active.clone().unwrap_or_else(|| "none".to_string()),
        ok: active
            .as_deref()
            .map(|name| registry.load(name).is_ok())
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

    for status in desktop_integrations::status() {
        diagnostics.push(Diagnostic {
            label: format!("Integration {}", status.id),
            detail: match &status.detected {
                Some(path) => format!("available ({})", path.display()),
                None => format!("not installed (looked for {})", status.programs.join(", ")),
            },
            ok: true,
        });
    }

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
        assert!(report.installed_files.contains(&"waybar.css".to_string()));
        assert!(
            report
                .installed_files
                .contains(&"rofi/theme.rasi".to_string())
        );
        assert!(
            report
                .installed_files
                .contains(&"hyprland/my-linux.conf".to_string())
        );
        assert_eq!(report.created_backups.len(), 1);

        assert_eq!(
            fs::read_to_string(original_backup(&dir, WAYBAR_RELATIVE_PATH)).unwrap(),
            "original"
        );
        assert_eq!(crate::read_active_theme(&dir).as_deref(), Some("cyberpunk"));
        assert_eq!(
            installed_entries(&dir).len(),
            desktop_integrations::all().len()
        );
        assert_eq!(read_history(&dir).len(), 1);
        assert!(!journal_file(&dir).exists());

        // Every integration wrote its file.
        for integration in desktop_integrations::all() {
            assert!(
                destination(&dir, integration.relative_path()).is_file(),
                "{} should be installed",
                integration.id()
            );
        }

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn applies_only_the_selected_integrations() {
        let dir = test_dir("apply-selected");
        let selected = desktop_integrations::select(&["rofi".to_string()]).unwrap();

        let report = apply_theme_with(&dir, &theme("nature"), &selected).unwrap();

        assert_eq!(report.installed_files, vec!["rofi/theme.rasi"]);
        assert!(destination(&dir, "rofi/theme.rasi").is_file());
        assert!(!destination(&dir, WAYBAR_RELATIVE_PATH).exists());

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

        assert!(matches!(result, Err(ApplyError::Validation(_))));

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
        // Files that never replaced a user file are removed again.
        assert!(!destination(&dir, "rofi/theme.rasi").exists());
        assert_eq!(crate::read_active_theme(&dir), None);
        assert!(installed_entries(&dir).is_empty());
        assert_eq!(read_history(&dir).len(), 3);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn restore_refuses_to_delete_a_hand_edited_generated_file() {
        let dir = test_dir("restore-modified");

        apply_theme(&dir, &theme("cyberpunk")).unwrap();
        fs::write(destination(&dir, "rofi/theme.rasi"), "/* mine */").unwrap();

        let result = restore_theme(&dir);

        assert!(matches!(result, Err(ApplyError::Validation(_))));
        assert_eq!(
            fs::read_to_string(destination(&dir, "rofi/theme.rasi")).unwrap(),
            "/* mine */"
        );

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
