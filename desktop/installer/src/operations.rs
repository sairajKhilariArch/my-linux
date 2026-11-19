//! Install, upgrade, verify and uninstall operations.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use desktop_core::config_manager::digest_file;

use crate::environment::HostEnvironment;
use crate::layout::InstallLayout;
use crate::manifest::{Manifest, ManifestFile};

/// The icon is embedded so a `--source` tree does not need to ship it.
pub const ICON_SVG: &str = include_str!("../../../packaging/my-linux.svg");

#[derive(Debug)]
pub enum InstallError {
    Io(io::Error),
    Toml(toml::de::Error),
    Serialize(toml::ser::Error),
    Message(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Toml(error) => write!(formatter, "Invalid installation manifest: {error}"),
            Self::Serialize(error) => write!(formatter, "Cannot render the manifest: {error}"),
            Self::Message(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<io::Error> for InstallError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<toml::de::Error> for InstallError {
    fn from(error: toml::de::Error) -> Self {
        Self::Toml(error)
    }
}

impl InstallError {
    pub(crate) fn write_text(path: &Path, contents: &str) -> Result<(), InstallError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temporary = path.with_extension("new");

        fs::write(&temporary, contents)?;
        fs::rename(&temporary, path)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub prefix: PathBuf,
    pub binaries: Vec<(String, PathBuf)>,
    pub themes: PathBuf,
    pub profiles: PathBuf,
    /// Built-in extensions; optional so an older release tree still installs.
    pub extensions: Option<PathBuf>,
    pub version: String,
    pub force: bool,
    pub dry_run: bool,
}

impl InstallOptions {
    /// Installs the binaries next to the running executable.
    pub fn from_current_installation(prefix: impl AsRef<Path>) -> Result<Self, InstallError> {
        let executable = std::env::current_exe()?;
        let bin_dir = executable
            .parent()
            .ok_or_else(|| {
                InstallError::Message("Cannot locate the running executable".to_string())
            })?
            .to_path_buf();

        let themes = first_existing(&[
            bin_dir.join("../share/my-linux/themes"),
            bin_dir.join("../../themes"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes"),
        ])
        .ok_or_else(|| {
            InstallError::Message(format!(
                "Cannot find the themes directory next to {}; pass --source <release directory>",
                executable.display()
            ))
        })?;
        let profiles = first_existing(&[
            bin_dir.join("../share/my-linux/profiles"),
            bin_dir.join("../../profiles"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles"),
        ])
        .ok_or_else(|| {
            InstallError::Message(format!(
                "Cannot find the profiles directory next to {}; pass --source <release directory>",
                executable.display()
            ))
        })?;
        let extensions = first_existing(&[
            bin_dir.join("../share/my-linux/extensions"),
            bin_dir.join("../../extensions"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extensions"),
        ]);

        Self::with_tree(prefix, &bin_dir, themes, profiles, extensions)
    }

    /// Installs from a release tree: `bin/` plus `share/my-linux/{themes,profiles}`
    /// (or `themes/` and `profiles/` directly under the source).
    pub fn from_source(
        source: impl AsRef<Path>,
        prefix: impl AsRef<Path>,
    ) -> Result<Self, InstallError> {
        let source = source.as_ref();
        let bin_dir = [source.join("bin"), source.to_path_buf()]
            .into_iter()
            .find(|directory| directory.join("my-linux-theme").is_file())
            .ok_or_else(|| {
                InstallError::Message(format!(
                    "No my-linux-theme binary in {}; build a release first",
                    source.display()
                ))
            })?;
        let share = source.join("share").join("my-linux");
        let themes =
            first_existing(&[share.join("themes"), source.join("themes")]).ok_or_else(|| {
                InstallError::Message(format!(
                    "No themes directory in {}; expected share/my-linux/themes",
                    source.display()
                ))
            })?;
        let profiles = first_existing(&[share.join("profiles"), source.join("profiles")])
            .ok_or_else(|| {
                InstallError::Message(format!(
                    "No profiles directory in {}; expected share/my-linux/profiles",
                    source.display()
                ))
            })?;
        let extensions = first_existing(&[share.join("extensions"), source.join("extensions")]);

        Self::with_tree(prefix, &bin_dir, themes, profiles, extensions)
    }

    fn with_tree(
        prefix: impl AsRef<Path>,
        bin_dir: &Path,
        themes: PathBuf,
        profiles: PathBuf,
        extensions: Option<PathBuf>,
    ) -> Result<Self, InstallError> {
        let mut binaries = Vec::new();

        for name in ["my-linux-theme", "my-linux-gui"] {
            let path = bin_dir.join(name);

            if !path.is_file() {
                return Err(InstallError::Message(format!(
                    "Missing {name} in {}; build both binaries first",
                    bin_dir.display()
                )));
            }

            binaries.push((name.to_string(), path));
        }

        Ok(Self {
            prefix: prefix.as_ref().to_path_buf(),
            binaries,
            themes,
            profiles,
            extensions,
            version: crate::VERSION.to_string(),
            force: false,
            dry_run: false,
        })
    }
}

fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|candidate| candidate.is_dir())
        .cloned()
}

#[derive(Debug, Clone)]
pub struct InstallReport {
    pub prefix: PathBuf,
    pub version: String,
    pub previous_version: Option<String>,
    pub files: Vec<String>,
    pub preserved: Vec<String>,
    pub dry_run: bool,
    pub environment: HostEnvironment,
}

/// Installs (or upgrades) My Linux into the configured prefix.
pub fn install(options: &InstallOptions) -> Result<InstallReport, InstallError> {
    let layout = InstallLayout::new(&options.prefix);
    let previous = crate::manifest::read(layout.manifest_file())?;
    let environment = crate::environment::detect_environment();

    if !options.dry_run {
        for directory in layout.directories() {
            fs::create_dir_all(&directory)?;
        }
    }

    let mut manifest = Manifest {
        version: options.version.clone(),
        installed_at: Manifest::now(),
        prefix: layout.prefix.display().to_string(),
        platform: format!("{}/{}", environment.os_id, environment.architecture),
        files: Vec::new(),
        preserved: Vec::new(),
    };

    for (name, source) in &options.binaries {
        let destination = layout.bin(name);

        if options.dry_run {
            manifest
                .files
                .push(planned(&layout, &destination, "binary"));
            continue;
        }

        copy_executable(source, &destination)?;
        manifest
            .files
            .push(record(&layout, &destination, "binary")?);
    }

    {
        let mut context = TreeContext {
            layout: &layout,
            previous: previous.as_ref(),
            options,
            manifest: &mut manifest,
        };

        install_tree(
            &mut context,
            &options.themes,
            &layout.themes_dir(),
            "colors/palette.toml",
            "theme",
        )?;
        install_tree(
            &mut context,
            &options.profiles,
            &layout.profiles_dir(),
            "profile.toml",
            "profile",
        )?;

        if let Some(extensions) = &options.extensions {
            install_tree(
                &mut context,
                extensions,
                &layout.extensions_dir(),
                "extension.toml",
                "extension",
            )?;
        }
    }

    let icon = layout.icon_file();

    if options.dry_run {
        manifest.files.push(planned(&layout, &icon, "icon"));
    } else {
        InstallError::write_text(&icon, ICON_SVG)?;
        manifest.files.push(record(&layout, &icon, "icon")?);
    }

    let desktop_file = layout.desktop_file();
    let entry = desktop_entry(&layout.bin("my-linux-gui"));

    if options.dry_run {
        manifest
            .files
            .push(planned(&layout, &desktop_file, "desktop"));
    } else {
        InstallError::write_text(&desktop_file, &entry)?;
        manifest
            .files
            .push(record(&layout, &desktop_file, "desktop")?);
    }

    if !options.dry_run {
        manifest.write(layout.manifest_file())?;

        let report = verify(&layout)?;
        let problems = report.problems();

        if !problems.is_empty() {
            let details = problems
                .iter()
                .map(|check| format!("{} ({})", check.label, check.detail))
                .collect::<Vec<String>>()
                .join(", ");

            return Err(InstallError::Message(format!(
                "Installation verification failed: {details}"
            )));
        }
    }

    Ok(InstallReport {
        prefix: layout.prefix.clone(),
        version: options.version.clone(),
        previous_version: previous.as_ref().map(|manifest| manifest.version.clone()),
        files: manifest
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect(),
        preserved: manifest.preserved.clone(),
        dry_run: options.dry_run,
        environment,
    })
}

struct TreeContext<'a> {
    layout: &'a InstallLayout,
    previous: Option<&'a Manifest>,
    options: &'a InstallOptions,
    manifest: &'a mut Manifest,
}

fn install_tree(
    context: &mut TreeContext<'_>,
    source: &Path,
    target: &Path,
    signature: &str,
    kind: &str,
) -> Result<(), InstallError> {
    let layout = context.layout;
    let options = context.options;
    let previous = context.previous;
    let manifest = &mut *context.manifest;

    if !source.is_dir() {
        return Err(InstallError::Message(format!(
            "Missing {kind} source directory {}",
            source.display()
        )));
    }

    let mut entries: Vec<PathBuf> = fs::read_dir(source)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();

    entries.sort();

    for entry in entries {
        let name = entry
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let destination = target.join(&name);

        if destination.exists() {
            let signature_path = destination.join(signature);
            let relative = layout.relative(&signature_path);
            let installed = previous
                .and_then(|manifest| manifest.digest_of(&relative))
                .map(str::to_string);
            let current = if signature_path.is_file() {
                Some(
                    digest_file(&signature_path)
                        .map_err(|error| InstallError::Message(error.to_string()))?,
                )
            } else {
                None
            };
            let unmodified = matches!(
                (installed.as_deref(), current.as_deref()),
                (Some(installed), Some(current)) if installed == current
            );

            if !options.force && !unmodified {
                manifest.preserved.push(layout.relative(&destination));
                continue;
            }

            if !options.dry_run {
                fs::remove_dir_all(&destination)?;
            }
        }

        if options.dry_run {
            manifest.files.push(planned(layout, &destination, kind));
            continue;
        }

        copy_dir(&entry, &destination)?;

        for file in walk(&destination) {
            manifest.files.push(record(layout, &file, kind)?);
        }
    }

    Ok(())
}

fn planned(layout: &InstallLayout, path: &Path, kind: &str) -> ManifestFile {
    ManifestFile {
        path: layout.relative(path),
        digest: String::new(),
        kind: kind.to_string(),
    }
}

fn record(layout: &InstallLayout, path: &Path, kind: &str) -> Result<ManifestFile, InstallError> {
    let digest = digest_file(path).map_err(|error| InstallError::Message(error.to_string()))?;

    Ok(ManifestFile {
        path: layout.relative(path),
        digest,
        kind: kind.to_string(),
    })
}

fn copy_executable(source: &Path, destination: &Path) -> Result<(), InstallError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let temporary = destination.with_extension("new");

    fs::copy(source, &temporary)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755))?;
    }

    fs::rename(&temporary, destination)?;

    Ok(())
}

fn copy_dir(source: &Path, destination: &Path) -> Result<(), InstallError> {
    fs::create_dir_all(destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let target = destination.join(entry.file_name());

        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }

    Ok(())
}

fn walk(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![directory.to_path_buf()];

    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }

    files.sort();

    files
}

/// The desktop entry written into `share/applications`.
pub fn desktop_entry(exec: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=My Linux\n\
         GenericName=Desktop Customization\n\
         Comment=Themes, profiles, wallpapers and desktop configuration\n\
         Exec={}\n\
         Icon=my-linux\n\
         Terminal=false\n\
         Categories=Settings;DesktopSettings;\n\
         Keywords=theme;hyprland;desktop;wallpaper;profile;\n\
         StartupNotify=true\n",
        exec.display()
    )
}

/// Upgrades an existing installation, preserving modified built-in content.
pub fn upgrade(options: &InstallOptions) -> Result<InstallReport, InstallError> {
    InstallLayout::new(&options.prefix).require_manifest()?;

    install(options)
}

#[derive(Debug, Clone)]
pub struct UninstallReport {
    pub prefix: PathBuf,
    pub version: String,
    pub removed: Vec<String>,
    pub missing: Vec<String>,
    pub dry_run: bool,
}

/// Removes every installed program file. State, backups, user themes and user
/// profiles are never touched.
pub fn uninstall(layout: &InstallLayout, dry_run: bool) -> Result<UninstallReport, InstallError> {
    let manifest = layout.require_manifest()?;
    let mut report = UninstallReport {
        prefix: layout.prefix.clone(),
        version: manifest.version.clone(),
        removed: Vec::new(),
        missing: Vec::new(),
        dry_run,
    };

    for file in &manifest.files {
        if !safe_relative(&file.path) {
            return Err(InstallError::Message(format!(
                "Unsafe path in the installation manifest: {}",
                file.path
            )));
        }

        if dry_run {
            report.removed.push(file.path.clone());
            continue;
        }

        let path = layout.prefix.join(&file.path);

        match fs::remove_file(&path) {
            Ok(()) => report.removed.push(file.path.clone()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                report.missing.push(file.path.clone());
            }
            Err(error) => return Err(error.into()),
        }
    }

    if !dry_run {
        let manifest_path = layout.manifest_file();

        match fs::remove_file(&manifest_path) {
            Ok(()) => report.removed.push(layout.relative(&manifest_path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        for directory in [
            layout.themes_dir(),
            layout.profiles_dir(),
            layout.extensions_dir(),
            layout.share_dir(),
            layout.icons_dir(),
            layout.applications_dir(),
        ] {
            remove_empty_dirs(&directory);
        }
    }

    Ok(report)
}

fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path
            .split('/')
            .any(|component| component == ".." || component.is_empty())
}

fn remove_empty_dirs(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            remove_empty_dirs(&path);
        }
    }

    if fs::read_dir(directory)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
    {
        let _ = fs::remove_dir(directory);
    }
}

#[derive(Debug, Clone)]
pub struct VerifyCheck {
    pub label: String,
    pub path: Option<PathBuf>,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct VerifyReport {
    pub checks: Vec<VerifyCheck>,
    pub version: String,
    pub current_version: String,
}

impl VerifyReport {
    pub fn ok(&self) -> bool {
        self.checks.iter().all(|check| check.ok)
    }

    pub fn problems(&self) -> Vec<&VerifyCheck> {
        self.checks.iter().filter(|check| !check.ok).collect()
    }
}

/// Checks every manifest entry against the files on disk.
pub fn verify(layout: &InstallLayout) -> Result<VerifyReport, InstallError> {
    let manifest = layout.require_manifest()?;
    let mut checks = Vec::new();

    for file in &manifest.files {
        let path = layout.prefix.join(&file.path);
        let exists = path.is_file();
        let digest_ok = exists
            && digest_file(&path)
                .map(|digest| digest == file.digest)
                .unwrap_or(false);

        checks.push(VerifyCheck {
            label: format!("{} ({})", file.path, file.kind),
            path: Some(path),
            ok: exists && digest_ok,
            detail: if !exists {
                "missing".to_string()
            } else if !digest_ok {
                "digest mismatch".to_string()
            } else {
                "ok".to_string()
            },
        });
    }

    let desktop = layout.desktop_file();

    checks.push(VerifyCheck {
        label: "Desktop entry".to_string(),
        path: Some(desktop.clone()),
        ok: desktop.is_file(),
        detail: if desktop.is_file() {
            "ok".to_string()
        } else {
            "missing".to_string()
        },
    });

    let up_to_date = manifest.version == crate::VERSION;

    checks.push(VerifyCheck {
        label: "Version".to_string(),
        path: None,
        ok: up_to_date,
        detail: format!("installed {}, running {}", manifest.version, crate::VERSION),
    });

    Ok(VerifyReport {
        checks,
        version: manifest.version,
        current_version: crate::VERSION.to_string(),
    })
}

#[derive(Debug, Clone)]
pub struct InstallStatus {
    pub manifest: Manifest,
    pub verify: VerifyReport,
}

/// Status of an existing installation, if any.
pub fn install_status(layout: &InstallLayout) -> Result<Option<InstallStatus>, InstallError> {
    if crate::manifest::read(layout.manifest_file())?.is_none() {
        return Ok(None);
    }

    let manifest = layout.require_manifest()?;
    let verify = verify(layout)?;

    Ok(Some(InstallStatus { manifest, verify }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "my-linux-installer-{}-{}",
            name,
            std::process::id()
        ));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        directory
    }

    fn source_tree(root: &Path, color: &str, binary: &str) -> PathBuf {
        let source = root.join("source");

        fs::create_dir_all(source.join("bin")).unwrap();
        fs::write(source.join("bin/my-linux-theme"), binary).unwrap();
        fs::write(source.join("bin/my-linux-gui"), binary).unwrap();
        fs::create_dir_all(source.join("themes/test-theme/colors")).unwrap();
        fs::write(
            source.join("themes/test-theme/colors/palette.toml"),
            format!(
                "[theme]\nschema = 1\nid = \"test-theme\"\nname = \"Test\"\n\n[colors]\nbackground = \"{color}\"\n"
            ),
        )
        .unwrap();
        fs::create_dir_all(source.join("profiles/test-profile")).unwrap();
        fs::write(
            source.join("profiles/test-profile/profile.toml"),
            "[profile]\nschema = 1\nid = \"test-profile\"\nname = \"Test\"\ntheme = \"test-theme\"\n",
        )
        .unwrap();
        fs::create_dir_all(source.join("extensions/test-extension")).unwrap();
        fs::write(
            source.join("extensions/test-extension/extension.toml"),
            "[extension]\nschema = 1\nid = \"test-extension\"\nname = \"Test\"\nversion = \"1.0.0\"\ndescription = \"Test\"\nformat = \"text\"\ndestination = \"test/one.conf\"\ntemplate = \"template.txt\"\n",
        )
        .unwrap();
        fs::write(
            source.join("extensions/test-extension/template.txt"),
            "color = {{colors.background}}\n",
        )
        .unwrap();

        source
    }

    fn plain_source(root: &Path) -> PathBuf {
        let source = root.join("plain");
        fs::create_dir_all(source.join("bin")).unwrap();
        fs::write(source.join("bin/my-linux-theme"), "theme").unwrap();
        fs::write(source.join("bin/my-linux-gui"), "gui").unwrap();
        fs::create_dir_all(source.join("themes")).unwrap();
        fs::create_dir_all(source.join("profiles")).unwrap();
        source
    }

    #[test]
    fn installs_program_files_and_manifest() {
        let root = scratch("install");
        let source = source_tree(&root, "#101010", "binary");
        let prefix = root.join("prefix");
        let options =
            InstallOptions::from_source(&source, &prefix).expect("source tree should resolve");

        let report = install(&options).unwrap();

        assert_eq!(report.version, crate::VERSION);
        assert!(report.previous_version.is_none());
        assert!(prefix.join("bin/my-linux-theme").is_file());
        assert!(prefix.join("bin/my-linux-gui").is_file());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(prefix.join("bin/my-linux-gui"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;

            assert_eq!(mode, 0o755);
        }

        assert!(
            prefix
                .join("share/my-linux/themes/test-theme/colors/palette.toml")
                .is_file()
        );
        assert!(
            prefix
                .join("share/my-linux/profiles/test-profile/profile.toml")
                .is_file()
        );
        assert!(
            prefix
                .join("share/my-linux/extensions/test-extension/extension.toml")
                .is_file()
        );
        assert!(
            prefix
                .join("share/my-linux/extensions/test-extension/template.txt")
                .is_file()
        );
        assert!(
            prefix
                .join("share/icons/hicolor/scalable/apps/my-linux.svg")
                .is_file()
        );

        let entry = fs::read_to_string(prefix.join("share/applications/my-linux.desktop")).unwrap();

        assert!(entry.contains("Name=My Linux"));
        assert!(entry.contains(prefix.join("bin/my-linux-gui").to_str().unwrap()));

        let layout = InstallLayout::new(&prefix);
        let manifest = crate::manifest::read(layout.manifest_file())
            .unwrap()
            .unwrap();

        assert_eq!(manifest.version, crate::VERSION);
        assert!(manifest.files.iter().any(|file| file.kind == "theme"));
        assert!(manifest.files.iter().any(|file| file.kind == "extension"));
        assert!(verify(&layout).unwrap().ok());
        assert_eq!(
            install_status(&layout).unwrap().unwrap().manifest.version,
            crate::VERSION
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn upgrade_replaces_binaries_and_preserves_modified_themes() {
        let root = scratch("upgrade");
        let prefix = root.join("prefix");
        let first = source_tree(&root.join("v1"), "#111111", "binary-v1");

        install(&InstallOptions::from_source(&first, &prefix).unwrap()).unwrap();

        let installed_theme = prefix.join("share/my-linux/themes/test-theme/colors/palette.toml");
        let installed = fs::read_to_string(&installed_theme).unwrap();

        assert!(installed.contains("#111111"));

        fs::write(&installed_theme, "edited by the user\n").unwrap();

        let second = source_tree(&root.join("v2"), "#222222", "binary-v2");
        let report = upgrade(&InstallOptions::from_source(&second, &prefix).unwrap()).unwrap();

        assert_eq!(
            fs::read_to_string(prefix.join("bin/my-linux-theme")).unwrap(),
            "binary-v2"
        );
        assert_eq!(
            fs::read_to_string(&installed_theme).unwrap(),
            "edited by the user\n"
        );
        assert!(
            report
                .preserved
                .iter()
                .any(|path| path.contains("test-theme"))
        );
        assert_eq!(report.previous_version.as_deref(), Some(crate::VERSION));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn uninstall_removes_program_files_and_keeps_state() {
        let root = scratch("uninstall");
        let prefix = root.join("prefix");
        let source = source_tree(&root, "#333333", "binary");

        install(&InstallOptions::from_source(&source, &prefix).unwrap()).unwrap();

        let state = root.join("state");
        fs::create_dir_all(&state).unwrap();
        fs::write(state.join("events.jsonl"), "keep me\n").unwrap();

        let layout = InstallLayout::new(&prefix);
        let report = uninstall(&layout, false).unwrap();

        assert!(!prefix.join("bin/my-linux-theme").exists());
        assert!(!layout.desktop_file().exists());
        assert!(!layout.manifest_file().exists());
        assert!(state.join("events.jsonl").is_file());
        assert!(report.removed.iter().any(|path| path.contains("bin/")));
        assert!(uninstall(&layout, false).is_err());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verify_detects_a_changed_binary() {
        let root = scratch("verify");
        let prefix = root.join("prefix");
        let source = source_tree(&root, "#444444", "binary");

        install(&InstallOptions::from_source(&source, &prefix).unwrap()).unwrap();

        fs::write(prefix.join("bin/my-linux-theme"), "tampered").unwrap();

        let report = verify(&InstallLayout::new(&prefix)).unwrap();

        assert!(!report.ok());
        assert!(
            report
                .problems()
                .iter()
                .any(|check| check.label.contains("my-linux-theme"))
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dry_run_writes_nothing() {
        let root = scratch("dry-run");
        let prefix = root.join("prefix");
        let source = source_tree(&root, "#555555", "binary");
        let mut options = InstallOptions::from_source(&source, &prefix).unwrap();

        options.dry_run = true;

        let report = install(&options).unwrap();

        assert!(!prefix.exists());
        assert!(!report.files.is_empty());
        assert!(report.dry_run);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_to_upgrade_what_is_not_installed() {
        let root = scratch("not-installed");
        let source = plain_source(&root);
        let options = InstallOptions::from_source(&source, root.join("prefix")).unwrap();

        let error = upgrade(&options).unwrap_err();

        assert!(error.to_string().contains("not installed"));
    }

    #[test]
    fn rejects_a_source_without_binaries() {
        let root = scratch("empty-source");
        let empty = root.join("empty");
        fs::create_dir_all(&empty).unwrap();

        assert!(InstallOptions::from_source(&empty, root.join("prefix")).is_err());

        fs::remove_dir_all(root).unwrap();
    }
}
