//! Safe deployment of generated configurations into a live config directory.
//!
//! Applying a theme only writes into the installation root. Deployment is the
//! explicit second step that copies the generated files into the user's live
//! config directory. Every replaced file is backed up, every written file is
//! recorded, and `undeploy` restores the original files.
//!
//! The live state lives beside the deployed files in
//! `<config-dir>/my-linux/deploy/`, so undeploying does not depend on the
//! installation root still being present.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use config_manager::{
    ConfigError, backup_file, digest, install_content, remove_file_if_exists, restore_file,
    snapshot_file, validate_path,
};

use crate::log::{Level, append_event};
use crate::transaction::{destination, installed_entries};

const DEPLOY_DIRECTORY: &str = "my-linux/deploy";
const DEPLOYED_FILE: &str = "deployed.tsv";
const JOURNAL_FILE: &str = "journal.tsv";
const BACKUP_DIRECTORY: &str = "backups";

#[derive(Debug)]
pub enum DeployError {
    Config(ConfigError),
    Io(io::Error),
    Validation(String),
    Conflict(Vec<PathBuf>),
    MissingBackup(PathBuf),
    RollbackIncomplete(String),
}

impl std::fmt::Display for DeployError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeployError::Config(error) => write!(f, "{error}"),
            DeployError::Io(error) => write!(f, "I/O error: {error}"),
            DeployError::Validation(error) => write!(f, "{error}"),
            DeployError::Conflict(paths) => {
                write!(
                    f,
                    "These live files were changed after deployment: {}. Re-run with `--force` only after reviewing them.",
                    paths
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
            DeployError::MissingBackup(path) => write!(
                f,
                "The original backup for {} is missing; refusing to overwrite a file that cannot be restored",
                path.display()
            ),
            DeployError::RollbackIncomplete(error) => write!(
                f,
                "Live deployment failed and its rollback did not complete: {error}. Inspect the live config directory before retrying."
            ),
        }
    }
}

impl std::error::Error for DeployError {}

impl From<ConfigError> for DeployError {
    fn from(error: ConfigError) -> Self {
        DeployError::Config(error)
    }
}

impl From<io::Error> for DeployError {
    fn from(error: io::Error) -> Self {
        DeployError::Io(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployAction {
    Create,
    Update,
    Unchanged,
}

impl DeployAction {
    pub const fn label(self) -> &'static str {
        match self {
            DeployAction::Create => "create",
            DeployAction::Update => "update",
            DeployAction::Unchanged => "unchanged",
        }
    }

    pub const fn writes(self) -> bool {
        !matches!(self, DeployAction::Unchanged)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndeployAction {
    Restore,
    Remove,
    Missing,
}

impl UndeployAction {
    pub const fn label(self) -> &'static str {
        match self {
            UndeployAction::Restore => "restore",
            UndeployAction::Remove => "remove",
            UndeployAction::Missing => "missing",
        }
    }

    pub const fn writes(self) -> bool {
        !matches!(self, UndeployAction::Missing)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupState {
    Original,
    Absent,
}

impl BackupState {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "original" => Some(BackupState::Original),
            "absent" => Some(BackupState::Absent),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            BackupState::Original => "original",
            BackupState::Absent => "absent",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeployEntry {
    pub relative_path: String,
    pub live_path: PathBuf,
    pub action: DeployAction,
    pub backup: BackupState,
    pub integration: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct SkippedFile {
    pub relative_path: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct DeployReport {
    pub config_dir: PathBuf,
    pub entries: Vec<DeployEntry>,
    pub skipped: Vec<SkippedFile>,
    pub conflicts: Vec<PathBuf>,
    pub dry_run: bool,
}

impl DeployReport {
    pub fn changed(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.action.writes())
            .count()
    }
}

#[derive(Debug, Clone)]
pub struct UndeployEntry {
    pub relative_path: String,
    pub live_path: PathBuf,
    pub action: UndeployAction,
    pub backup: BackupState,
}

#[derive(Debug, Clone)]
pub struct UndeployReport {
    pub config_dir: PathBuf,
    pub entries: Vec<UndeployEntry>,
    pub conflicts: Vec<PathBuf>,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct DeploymentStatus {
    pub relative_path: String,
    pub live_path: PathBuf,
    pub backup: BackupState,
    pub exists: bool,
    pub modified: bool,
}

#[derive(Debug, Clone)]
struct Deployment {
    live_path: PathBuf,
    relative_path: String,
    digest: String,
    backup: BackupState,
}

#[derive(Debug, Clone)]
struct PlannedDeploy {
    entry: DeployEntry,
    content: Vec<u8>,
}

#[derive(Debug, Clone)]
struct JournalEntry {
    live_path: PathBuf,
    snapshot: Option<PathBuf>,
}

/// The config directory used by normal desktop sessions.
pub fn default_config_dir() -> Result<PathBuf, DeployError> {
    if let Some(directory) = env::var_os("XDG_CONFIG_HOME")
        && !directory.is_empty()
    {
        return Ok(PathBuf::from(directory));
    }

    if let Some(home) = env::var_os("HOME")
        && !home.is_empty()
    {
        return Ok(PathBuf::from(home).join(".config"));
    }

    Err(DeployError::Validation(
        "Cannot determine the live config directory; set XDG_CONFIG_HOME, HOME or pass --config-dir"
            .to_string(),
    ))
}

/// Resolves and validates a config directory without creating it.
pub fn resolve_config_dir(directory: impl AsRef<Path>) -> Result<PathBuf, DeployError> {
    let directory = absolute_path(directory.as_ref())?;

    if !validate_path(&directory) {
        return Err(DeployError::Validation(format!(
            "Invalid live config directory: {}",
            directory.display()
        )));
    }

    Ok(directory)
}

fn absolute_path(path: &Path) -> Result<PathBuf, DeployError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

fn canonical_for_comparison(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }

    if let (Some(parent), Some(name)) = (path.parent(), path.file_name())
        && let Ok(canonical) = fs::canonicalize(parent)
    {
        return canonical.join(name);
    }

    path.to_path_buf()
}

fn deploy_directory(config_dir: &Path) -> PathBuf {
    config_dir.join(DEPLOY_DIRECTORY)
}

fn deployed_file(config_dir: &Path) -> PathBuf {
    deploy_directory(config_dir).join(DEPLOYED_FILE)
}

fn journal_file(config_dir: &Path) -> PathBuf {
    deploy_directory(config_dir).join(JOURNAL_FILE)
}

fn backup_directory(config_dir: &Path) -> PathBuf {
    deploy_directory(config_dir).join(BACKUP_DIRECTORY)
}

fn backup_path(config_dir: &Path, live_path: &Path, kind: &str) -> PathBuf {
    let key = digest(live_path.to_string_lossy().as_bytes());

    backup_directory(config_dir).join(format!("{key}.{kind}"))
}

/// Maps a generated path to its destination under a live config directory.
///
/// Most integrations already use their live relative path. Waybar and Hyprland
/// use different names in the installation root because they are snippets.
/// Firefox is deliberately not mapped: its profile directory is specific to
/// each Firefox installation and must be handled by the user.
pub fn live_relative_path(relative_path: &str) -> Option<String> {
    let path = Path::new(relative_path);

    if path.is_absolute()
        || relative_path.is_empty()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return None;
    }

    let mapped = match relative_path {
        "firefox/userChrome.css" => return None,
        "waybar.css" => "waybar/style.css",
        "hyprland/my-linux.conf" => "hypr/my-linux.conf",
        "hyprland/layout.conf" => "hypr/layout.conf",
        "hyprland/assets.conf" => "hypr/assets.conf",
        "hyprlock/hyprlock.conf" => "hypr/hyprlock.conf",
        other => other,
    };

    Some(mapped.to_string())
}

fn integration_for(relative_path: &str) -> Option<&'static str> {
    crate::integrations::all()
        .iter()
        .find(|integration| integration.relative_path() == relative_path)
        .map(|integration| integration.id())
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, DeployError> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn read_deployments(config_dir: &Path) -> Result<BTreeMap<PathBuf, Deployment>, DeployError> {
    let path = deployed_file(config_dir);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(error.into()),
    };
    let mut deployments = BTreeMap::new();

    for (index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = line.split('\t').collect();

        if fields.len() != 4 {
            return Err(DeployError::Validation(format!(
                "Malformed deployment state in {} line {}",
                path.display(),
                index + 1
            )));
        }

        let live_path = PathBuf::from(fields[0]);

        if !live_path.is_absolute() || !live_path.starts_with(config_dir) {
            return Err(DeployError::Validation(format!(
                "Live path outside {} in {} line {}",
                config_dir.display(),
                path.display(),
                index + 1
            )));
        }

        let backup = BackupState::parse(fields[3]).ok_or_else(|| {
            DeployError::Validation(format!(
                "Unknown backup state in {} line {}",
                path.display(),
                index + 1
            ))
        })?;

        deployments.insert(
            live_path.clone(),
            Deployment {
                live_path,
                relative_path: fields[1].to_string(),
                digest: fields[2].to_string(),
                backup,
            },
        );
    }

    Ok(deployments)
}

fn write_deployments(
    config_dir: &Path,
    deployments: &BTreeMap<PathBuf, Deployment>,
) -> Result<(), DeployError> {
    let mut contents = String::new();

    for deployment in deployments.values() {
        contents.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            deployment.live_path.display(),
            deployment.relative_path,
            deployment.digest,
            deployment.backup.label()
        ));
    }

    install_content(contents.as_bytes(), deployed_file(config_dir))?;

    Ok(())
}

fn write_journal(config_dir: &Path, entries: &[JournalEntry]) -> Result<(), DeployError> {
    let mut contents = String::new();

    for entry in entries {
        contents.push_str(&format!(
            "{}\t{}\n",
            entry.live_path.display(),
            entry
                .snapshot
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "-".to_string())
        ));
    }

    install_content(contents.as_bytes(), journal_file(config_dir))?;

    Ok(())
}

fn read_journal(config_dir: &Path) -> Result<Vec<JournalEntry>, DeployError> {
    let path = journal_file(config_dir);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut entries = Vec::new();

    for (index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let fields: Vec<&str> = line.split('\t').collect();

        if fields.len() != 2 {
            return Err(DeployError::Validation(format!(
                "Malformed deployment journal in {} line {}",
                path.display(),
                index + 1
            )));
        }

        let live_path = PathBuf::from(fields[0]);
        let snapshot = (fields[1] != "-").then(|| PathBuf::from(fields[1]));

        if !live_path.is_absolute() || !live_path.starts_with(config_dir) {
            return Err(DeployError::Validation(format!(
                "Live path outside {} in deployment journal line {}",
                config_dir.display(),
                index + 1
            )));
        }

        if let Some(snapshot) = &snapshot
            && (!snapshot.is_absolute() || !snapshot.starts_with(backup_directory(config_dir)))
        {
            return Err(DeployError::Validation(format!(
                "Snapshot outside the deployment backup directory in {} line {}",
                path.display(),
                index + 1
            )));
        }

        entries.push(JournalEntry {
            live_path,
            snapshot,
        });
    }

    Ok(entries)
}

/// Restores a partial deployment left by an interrupted command.
pub fn recover_deploy(root: &Path, config_dir: &Path) -> Result<Vec<PathBuf>, DeployError> {
    let entries = read_journal(config_dir)?;

    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let mut restored = Vec::new();

    for entry in &entries {
        restore_journal_entry(entry)?;
        restored.push(entry.live_path.clone());
    }

    remove_file_if_exists(journal_file(config_dir))?;

    let _ = append_event(
        root,
        Level::Warn,
        "deploy-recovered",
        &format!("config={} files={}", config_dir.display(), restored.len()),
    );

    Ok(restored)
}

fn restore_journal_entry(entry: &JournalEntry) -> Result<(), DeployError> {
    match &entry.snapshot {
        Some(snapshot) if snapshot.is_file() => {
            restore_file(snapshot, &entry.live_path)?;
        }
        Some(snapshot) => {
            return Err(DeployError::MissingBackup(snapshot.clone()));
        }
        None => {
            remove_file_if_exists(&entry.live_path)?;
        }
    }

    Ok(())
}

fn rollback_deploy(config_dir: &Path) -> Result<(), DeployError> {
    let entries = read_journal(config_dir)?;

    for entry in &entries {
        restore_journal_entry(entry)?;
    }

    remove_file_if_exists(journal_file(config_dir))?;

    Ok(())
}

type DeployPlan = (Vec<PlannedDeploy>, Vec<SkippedFile>, Vec<PathBuf>);

fn plan_deploy(
    root: &Path,
    config_dir: &Path,
    force: bool,
    deployments: &BTreeMap<PathBuf, Deployment>,
) -> Result<DeployPlan, DeployError> {
    let mut planned = Vec::new();
    let mut skipped = Vec::new();
    let mut conflicts = Vec::new();

    for installed in installed_entries(root) {
        let relative_path = installed.relative_path;
        let Some(live_relative) = live_relative_path(&relative_path) else {
            skipped.push(SkippedFile {
                relative_path: relative_path.clone(),
                reason: "Firefox profile styling is machine-specific; copy it into the active Firefox profile's chrome/ directory"
                    .to_string(),
            });
            continue;
        };

        let source = destination(root, &relative_path);
        let Some(content) = read_optional(&source)? else {
            skipped.push(SkippedFile {
                relative_path,
                reason: "generated file is missing from the installation root".to_string(),
            });
            continue;
        };

        let live_path = config_dir.join(&live_relative);

        if live_path.starts_with(deploy_directory(config_dir)) {
            skipped.push(SkippedFile {
                relative_path,
                reason: "the destination is reserved for deployment state".to_string(),
            });
            continue;
        }

        let existing = read_optional(&live_path)?;
        let prior = deployments.get(&live_path).cloned();

        if let Some(prior) = &prior
            && prior.backup == BackupState::Original
        {
            let original = backup_path(config_dir, &live_path, "original");

            if !original.is_file() {
                return Err(DeployError::MissingBackup(original));
            }
        }

        let backup = prior
            .as_ref()
            .map(|deployment| deployment.backup)
            .unwrap_or(if existing.is_some() {
                BackupState::Original
            } else {
                BackupState::Absent
            });

        let action = match &existing {
            None => DeployAction::Create,
            Some(existing) if existing == &content => {
                if prior.is_some() {
                    DeployAction::Unchanged
                } else {
                    // Adopt an identical unmanaged file so undeploy can
                    // restore it, but still write it through the normal
                    // backup path.
                    DeployAction::Update
                }
            }
            Some(existing) => {
                let modified = prior
                    .as_ref()
                    .map(|deployment| digest(existing) != deployment.digest)
                    .unwrap_or(false);

                if modified && !force {
                    conflicts.push(live_path.clone());
                }

                DeployAction::Update
            }
        };

        let integration = integration_for(&relative_path);

        planned.push(PlannedDeploy {
            entry: DeployEntry {
                relative_path,
                live_path,
                action,
                backup,
                integration,
            },
            content,
        });
    }

    Ok((planned, skipped, conflicts))
}

/// Writes the installed generated files into `config_dir`.
///
/// This function performs writes; callers must obtain explicit user
/// authorization first. Use `dry_run` to inspect the plan without writing.
pub fn deploy(
    root: impl AsRef<Path>,
    config_dir: impl AsRef<Path>,
    force: bool,
    dry_run: bool,
) -> Result<DeployReport, DeployError> {
    let root = root.as_ref();
    let config_dir = resolve_config_dir(config_dir.as_ref())?;
    let root_absolute = canonical_for_comparison(&absolute_path(root)?);
    let config_for_comparison = canonical_for_comparison(&config_dir);

    if config_for_comparison == root_absolute || config_for_comparison.starts_with(&root_absolute) {
        return Err(DeployError::Validation(
            "The live config directory must be outside the installation root".to_string(),
        ));
    }

    for relative_path in ["my-linux/deploy", "my-linux/deploy/backups"] {
        let path = config_dir.join(relative_path);

        if path.exists() && !path.is_dir() {
            return Err(DeployError::Validation(format!(
                "{} must be a directory",
                path.display()
            )));
        }
    }

    recover_deploy(root, &config_dir)?;

    let deployments = read_deployments(&config_dir)?;
    let (planned, skipped, conflicts) = plan_deploy(root, &config_dir, force, &deployments)?;
    let report = DeployReport {
        config_dir: config_dir.clone(),
        entries: planned
            .iter()
            .map(|planned| planned.entry.clone())
            .collect(),
        skipped,
        conflicts: conflicts.clone(),
        dry_run,
    };

    if dry_run {
        return Ok(report);
    }

    if planned.is_empty() {
        return Err(DeployError::Validation(
            "No generated files are installed; apply a theme first".to_string(),
        ));
    }

    if !conflicts.is_empty() {
        return Err(DeployError::Conflict(conflicts));
    }

    let previous_snapshots = match execute_deploy(&config_dir, &planned, &deployments) {
        Ok(snapshots) => snapshots,
        Err(error) => {
            let recovery = rollback_deploy(&config_dir);

            return match recovery {
                Ok(()) => Err(error),
                Err(recovery_error) => {
                    Err(DeployError::RollbackIncomplete(recovery_error.to_string()))
                }
            };
        }
    };

    for snapshot in previous_snapshots {
        let _ = remove_file_if_exists(snapshot);
    }

    let _ = append_event(
        root,
        Level::Info,
        "deploy",
        &format!(
            "config={} files={} skipped={}",
            config_dir.display(),
            report.changed(),
            report.skipped.len()
        ),
    );

    Ok(report)
}

fn execute_deploy(
    config_dir: &Path,
    planned: &[PlannedDeploy],
    deployments: &BTreeMap<PathBuf, Deployment>,
) -> Result<Vec<PathBuf>, DeployError> {
    fs::create_dir_all(config_dir)?;

    let mut journal = Vec::new();
    let mut snapshots = Vec::new();

    for planned in planned {
        let live_path = &planned.entry.live_path;
        let existing = read_optional(live_path)?;

        if planned.entry.backup == BackupState::Original && existing.is_some() {
            let original = backup_path(config_dir, live_path, "original");

            if !original.exists() {
                backup_file(live_path, &original)?;
            }
        }

        if planned.entry.action.writes() {
            let snapshot = if existing.is_some() {
                let snapshot = backup_path(config_dir, live_path, "previous");
                snapshot_file(live_path, &snapshot)?;
                snapshots.push(snapshot.clone());
                Some(snapshot)
            } else {
                None
            };

            journal.push(JournalEntry {
                live_path: live_path.clone(),
                snapshot,
            });
        }
    }

    write_journal(config_dir, &journal)?;

    let mut updated = deployments.clone();

    for planned in planned {
        if planned.entry.action.writes() {
            install_content(&planned.content, &planned.entry.live_path)?;
        }

        updated.insert(
            planned.entry.live_path.clone(),
            Deployment {
                live_path: planned.entry.live_path.clone(),
                relative_path: planned.entry.relative_path.clone(),
                digest: digest(&planned.content),
                backup: planned.entry.backup,
            },
        );
    }

    write_deployments(config_dir, &updated)?;

    let _ = remove_file_if_exists(journal_file(config_dir));

    Ok(snapshots)
}

/// Lists the files currently deployed into `config_dir` and their state.
pub fn deploy_status(config_dir: impl AsRef<Path>) -> Result<Vec<DeploymentStatus>, DeployError> {
    let config_dir = resolve_config_dir(config_dir.as_ref())?;
    let deployments = read_deployments(&config_dir)?;
    let mut status = Vec::new();

    for deployment in deployments.values() {
        let current = read_optional(&deployment.live_path)?;

        status.push(DeploymentStatus {
            relative_path: deployment.relative_path.clone(),
            live_path: deployment.live_path.clone(),
            backup: deployment.backup,
            exists: current.is_some(),
            modified: current
                .as_ref()
                .map(|contents| digest(contents) != deployment.digest)
                .unwrap_or(false),
        });
    }

    Ok(status)
}

/// Restores every deployed file from its original backup, or removes files
/// that did not exist before deployment.
pub fn undeploy(
    root: impl AsRef<Path>,
    config_dir: impl AsRef<Path>,
    force: bool,
    dry_run: bool,
) -> Result<UndeployReport, DeployError> {
    let root = root.as_ref();
    let config_dir = resolve_config_dir(config_dir.as_ref())?;

    recover_deploy(root, &config_dir)?;

    let deployments = read_deployments(&config_dir)?;
    let mut entries = Vec::new();
    let mut conflicts = Vec::new();

    for deployment in deployments.values() {
        let current = read_optional(&deployment.live_path)?;
        let modified = current
            .as_ref()
            .map(|contents| digest(contents) != deployment.digest)
            .unwrap_or(false);

        if modified && !force {
            conflicts.push(deployment.live_path.clone());
        }

        let action = if current.is_none() {
            UndeployAction::Missing
        } else if deployment.backup == BackupState::Original {
            let original = backup_path(&config_dir, &deployment.live_path, "original");

            if !original.is_file() {
                return Err(DeployError::MissingBackup(original));
            }

            UndeployAction::Restore
        } else {
            UndeployAction::Remove
        };

        entries.push(UndeployEntry {
            relative_path: deployment.relative_path.clone(),
            live_path: deployment.live_path.clone(),
            action,
            backup: deployment.backup,
        });
    }

    let report = UndeployReport {
        config_dir: config_dir.clone(),
        entries,
        conflicts: conflicts.clone(),
        dry_run,
    };

    if dry_run || deployments.is_empty() {
        return Ok(report);
    }

    if !conflicts.is_empty() {
        return Err(DeployError::Conflict(conflicts));
    }

    let snapshots = match execute_undeploy(&config_dir, &report.entries) {
        Ok(snapshots) => snapshots,
        Err(error) => {
            let recovery = rollback_deploy(&config_dir);

            return match recovery {
                Ok(()) => Err(error),
                Err(recovery_error) => {
                    Err(DeployError::RollbackIncomplete(recovery_error.to_string()))
                }
            };
        }
    };

    for snapshot in snapshots {
        let _ = remove_file_if_exists(snapshot);
    }

    let _ = append_event(
        root,
        Level::Info,
        "undeploy",
        &format!(
            "config={} files={}",
            config_dir.display(),
            report.entries.len()
        ),
    );

    Ok(report)
}

fn execute_undeploy(
    config_dir: &Path,
    entries: &[UndeployEntry],
) -> Result<Vec<PathBuf>, DeployError> {
    let mut journal = Vec::new();
    let mut snapshots = Vec::new();

    for entry in entries {
        if !entry.action.writes() {
            continue;
        }

        let current = read_optional(&entry.live_path)?;

        if current.is_some() {
            let snapshot = backup_path(config_dir, &entry.live_path, "previous");
            snapshot_file(&entry.live_path, &snapshot)?;
            snapshots.push(snapshot.clone());
            journal.push(JournalEntry {
                live_path: entry.live_path.clone(),
                snapshot: Some(snapshot),
            });
        } else {
            journal.push(JournalEntry {
                live_path: entry.live_path.clone(),
                snapshot: None,
            });
        }
    }

    write_journal(config_dir, &journal)?;

    for entry in entries {
        match entry.action {
            UndeployAction::Restore => {
                let original = backup_path(config_dir, &entry.live_path, "original");
                restore_file(original, &entry.live_path)?;
            }
            UndeployAction::Remove => remove_file_if_exists(&entry.live_path)?,
            UndeployAction::Missing => {}
        }
    }

    remove_file_if_exists(deployed_file(config_dir))?;
    let _ = remove_file_if_exists(journal_file(config_dir));

    Ok(snapshots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::apply_theme;
    use theme_engine::LoadedTheme;

    fn scratch(name: &str) -> (PathBuf, PathBuf) {
        let base =
            std::env::temp_dir().join(format!("my-linux-deploy-{}-{}", name, std::process::id()));
        let root = base.join("root");
        let config = base.join("config");

        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&root).unwrap();

        (root, config)
    }

    fn theme(name: &str) -> LoadedTheme {
        crate::load_desktop_theme("../../themes", name).expect("theme should load")
    }

    #[test]
    fn maps_live_paths_without_touching_firefox_profiles() {
        assert_eq!(
            live_relative_path("waybar.css").as_deref(),
            Some("waybar/style.css")
        );
        assert_eq!(
            live_relative_path("hyprland/my-linux.conf").as_deref(),
            Some("hypr/my-linux.conf")
        );
        assert_eq!(
            live_relative_path("nvim/lua/my-linux.lua").as_deref(),
            Some("nvim/lua/my-linux.lua")
        );
        assert_eq!(live_relative_path("firefox/userChrome.css"), None);
        assert_eq!(live_relative_path("../escape"), None);
    }

    #[test]
    fn deploys_with_backups_and_undeploys() {
        let (root, config) = scratch("roundtrip");

        fs::create_dir_all(config.join("waybar")).unwrap();
        fs::write(config.join("waybar/style.css"), "original waybar").unwrap();

        apply_theme(&root, &theme("cyberpunk")).unwrap();

        let report = deploy(&root, &config, false, false).unwrap();

        assert!(report.changed() > 0);
        assert!(report.conflicts.is_empty());
        assert!(
            report
                .skipped
                .iter()
                .any(|skipped| skipped.relative_path == "firefox/userChrome.css")
        );
        assert!(
            fs::read_to_string(config.join("waybar/style.css"))
                .unwrap()
                .contains("@define-color")
        );

        let original = backup_path(&config, &config.join("waybar/style.css"), "original");
        assert_eq!(fs::read_to_string(&original).unwrap(), "original waybar");

        let status = deploy_status(&config).unwrap();
        assert!(status.iter().any(|entry| entry.exists && !entry.modified));

        let report = undeploy(&root, &config, false, false).unwrap();
        assert!(
            report
                .entries
                .iter()
                .any(|entry| { entry.action == UndeployAction::Restore })
        );
        assert_eq!(
            fs::read_to_string(config.join("waybar/style.css")).unwrap(),
            "original waybar"
        );
        assert!(!config.join("dunst/dunstrc").exists());

        let _ = fs::remove_dir_all(config.parent().unwrap());
    }

    #[test]
    fn refuses_modified_live_files_unless_forced() {
        let (root, config) = scratch("conflict");

        apply_theme(&root, &theme("cyberpunk")).unwrap();
        deploy(&root, &config, false, false).unwrap();

        let live = config.join("waybar/style.css");
        fs::write(&live, "/* edited by hand */").unwrap();

        let error = deploy(&root, &config, false, false).expect_err("modified live file");
        assert!(matches!(error, DeployError::Conflict(_)));

        deploy(&root, &config, true, false).expect("force replaces the live file");
        assert!(fs::read_to_string(&live).unwrap().contains("@define-color"));

        let _ = fs::remove_dir_all(config.parent().unwrap());
    }

    #[test]
    fn dry_run_writes_nothing() {
        let (root, config) = scratch("dry-run");

        apply_theme(&root, &theme("cyberpunk")).unwrap();

        let report = deploy(&root, &config, false, true).unwrap();

        assert!(report.dry_run);
        assert!(!config.join("waybar/style.css").exists());

        let _ = fs::remove_dir_all(config.parent().unwrap());
    }

    #[test]
    fn recovers_an_interrupted_live_deployment() {
        let (root, config) = scratch("recover");
        let live = config.join("waybar/style.css");
        let snapshot = backup_path(&config, &live, "previous");

        fs::create_dir_all(live.parent().unwrap()).unwrap();
        fs::write(&live, "half written").unwrap();
        fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
        fs::write(&snapshot, "before deployment").unwrap();
        write_journal(
            &config,
            &[JournalEntry {
                live_path: live.clone(),
                snapshot: Some(snapshot.clone()),
            }],
        )
        .unwrap();

        let restored = recover_deploy(&root, &config).unwrap();

        assert_eq!(restored, vec![live.clone()]);
        assert_eq!(fs::read_to_string(&live).unwrap(), "before deployment");
        assert!(!journal_file(&config).exists());

        let _ = fs::remove_dir_all(config.parent().unwrap());
    }

    #[test]
    fn keeps_an_original_backup_when_switching_themes() {
        let (root, config) = scratch("themes");

        apply_theme(&root, &theme("cyberpunk")).unwrap();

        fs::create_dir_all(config.join("waybar")).unwrap();
        fs::write(config.join("waybar/style.css"), "the user's waybar").unwrap();

        deploy(&root, &config, false, false).unwrap();

        let original = backup_path(&config, &config.join("waybar/style.css"), "original");
        assert_eq!(fs::read_to_string(&original).unwrap(), "the user's waybar");

        apply_theme(&root, &theme("nature")).unwrap();
        deploy(&root, &config, false, false).unwrap();

        assert_eq!(fs::read_to_string(&original).unwrap(), "the user's waybar");
        assert!(
            fs::read_to_string(config.join("waybar/style.css"))
                .unwrap()
                .contains("@define-color")
        );

        let _ = fs::remove_dir_all(config.parent().unwrap());
    }
}
