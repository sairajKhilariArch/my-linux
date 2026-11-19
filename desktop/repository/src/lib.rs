//! A community repository client for themes and extensions.
//!
//! A repository is a TOML index plus the content it references. The index can
//! live anywhere: a directory in a git repository, a shared network path, or a
//! local folder. My Linux never invents content: every package is installed by
//! the same validated theme and extension code that a local file uses.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use config_manager::install_content;
use desktop_extensions::ExtensionRegistry;
use theme_engine::Registry;

use crate::index::{Package, PackageKind, RepositoryIndex};

pub mod index;

/// Schema version of an index understood by this build.
pub const SCHEMA_VERSION: u32 = 1;

/// File name of an index inside a repository directory.
pub const INDEX_FILE: &str = "index.toml";

#[derive(Debug)]
pub enum RepositoryError {
    Io(io::Error),
    Invalid(String),
    Unknown(String),
    Unavailable(String),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Invalid(message) => write!(formatter, "{message}"),
            Self::Unknown(id) => write!(formatter, "Unknown package: {id}"),
            Self::Unavailable(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for RepositoryError {}

impl From<io::Error> for RepositoryError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<config_manager::ConfigError> for RepositoryError {
    fn from(error: config_manager::ConfigError) -> Self {
        Self::Invalid(error.to_string())
    }
}

/// Directory where fetched repositories are cached.
///
/// `$MY_LINUX_REPOSITORY_CACHE`, otherwise `$XDG_CACHE_HOME/my-linux/repository`.
pub fn cache_dir() -> PathBuf {
    if let Some(directory) = std::env::var_os("MY_LINUX_REPOSITORY_CACHE") {
        return PathBuf::from(directory);
    }

    let cache_home = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(|| PathBuf::from(".cache"));

    cache_home.join("my-linux").join("repository")
}

/// Resolves an index location: a local path, a `file://` URL, or a git URL.
///
/// Remote indexes are cloned into `cache`. A local path is used in place, so a
/// checkout can be edited and reinstalled without a copy step.
pub fn fetch(location: &str, cache: impl AsRef<Path>) -> Result<PathBuf, RepositoryError> {
    let cache = cache.as_ref();
    let local = location.strip_prefix("file://").unwrap_or(location);
    let path = PathBuf::from(local);

    if path.exists() {
        return Ok(path);
    }

    let remote = location.starts_with("http://")
        || location.starts_with("https://")
        || location.starts_with("git@")
        || location.starts_with("ssh://")
        || location.ends_with(".git");

    if !remote {
        return Err(RepositoryError::Unavailable(format!(
            "No repository at {location}; pass a path, a file:// URL or a git URL"
        )));
    }

    let name = location
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .rsplit(['/', ':'])
        .find(|part| !part.is_empty())
        .unwrap_or("repository")
        .to_string();

    fs::create_dir_all(cache)?;

    let target = cache.join(&name);

    if target.join(".git").is_dir() {
        let status = Command::new("git")
            .arg("-C")
            .arg(&target)
            .args(["pull", "--ff-only", "--quiet"])
            .status()
            .map_err(|error| {
                RepositoryError::Unavailable(format!("git is not available: {error}"))
            })?;

        if !status.success() {
            return Err(RepositoryError::Unavailable(format!(
                "`git pull` failed in {}",
                target.display()
            )));
        }
    } else {
        if target.exists() {
            fs::remove_dir_all(&target)?;
        }

        let status = Command::new("git")
            .args(["clone", "--depth", "1", "--quiet", location])
            .arg(&target)
            .status()
            .map_err(|error| {
                RepositoryError::Unavailable(format!("git is not available: {error}"))
            })?;

        if !status.success() {
            return Err(RepositoryError::Unavailable(format!(
                "Could not clone {location}"
            )));
        }
    }

    Ok(target)
}

/// Loads the index at `location`, fetching it first when it is remote.
pub fn load(location: &str, cache: impl AsRef<Path>) -> Result<RepositoryIndex, RepositoryError> {
    let resolved = fetch(location, cache)?;

    if resolved.is_file() {
        return index::load(&resolved);
    }

    let candidate = resolved.join(INDEX_FILE);

    if candidate.is_file() {
        return index::load(candidate);
    }

    Err(RepositoryError::Unavailable(format!(
        "No {INDEX_FILE} in {}",
        resolved.display()
    )))
}

/// Installs `package` into `user_themes` (themes) or the extension registry.
///
/// `replace` allows updating an installation that already exists.
pub fn install(
    package: &Package,
    user_themes: impl AsRef<Path>,
    extensions: &ExtensionRegistry,
    replace: bool,
) -> Result<PathBuf, RepositoryError> {
    match package.kind {
        PackageKind::Theme => install_theme(package, user_themes.as_ref(), replace),
        PackageKind::Extension => install_extension(package, extensions, replace),
    }
}

/// Installs a theme package into the user theme directory.
pub fn install_theme(
    package: &Package,
    user_themes: &Path,
    replace: bool,
) -> Result<PathBuf, RepositoryError> {
    let theme = Registry::load_path(&package.source).map_err(|error| {
        RepositoryError::Invalid(format!("{}: {error}", package.source.display()))
    })?;

    let target = user_themes
        .join(&package.id)
        .join("colors")
        .join("palette.toml");

    if target.exists() && !replace {
        return Err(RepositoryError::Invalid(format!(
            "Theme `{}` is already installed; pass --force to replace it",
            package.id
        )));
    }

    let mut imported = theme;
    imported.metadata.id = package.id.clone();

    let rendered = theme_engine::render_theme_toml(&imported);

    install_content(rendered.as_bytes(), &target)?;

    Ok(target)
}

/// Installs an extension package into the user extension directory.
pub fn install_extension(
    package: &Package,
    extensions: &ExtensionRegistry,
    replace: bool,
) -> Result<PathBuf, RepositoryError> {
    if replace {
        let target = extensions.user_root().join(&package.id);

        if target.is_dir() {
            fs::remove_dir_all(&target)?;
        }
    }

    extensions
        .import(&package.source, Some(&package.id))
        .map_err(|error| RepositoryError::Invalid(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use theme_engine::Registry as ThemeRegistry;

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "my-linux-repository-{}-{}",
            name,
            std::process::id()
        ));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        directory
    }

    fn write_theme(directory: &Path, id: &str) {
        let colors = directory.join(id).join("colors");

        fs::create_dir_all(&colors).unwrap();
        fs::write(
            colors.join("palette.toml"),
            format!(
                "[theme]\nschema = 1\nname = \"{id}\"\nid = \"{id}\"\nversion = \"1.0.0\"\ndescription = \"Repository theme\"\n\n[colors]\nbackground = \"#101010\"\nsurface = \"#202020\"\nsurface_alt = \"#303030\"\nprimary = \"#FF0055\"\nsecondary = \"#00FFCC\"\ntext = \"#FFFFFF\"\ntext_muted = \"#AAAAAA\"\nborder = \"#444444\"\nsuccess = \"#00FF00\"\nwarning = \"#FFFF00\"\nerror = \"#FF0000\"\n\n[colors.states]\nactive = \"#FF0055\"\nhover = \"#00FFCC\"\nselected = \"#303030\"\n"
            ),
        )
        .unwrap();
    }

    fn write_extension(directory: &Path, id: &str) {
        let extension = directory.join(id);

        fs::create_dir_all(&extension).unwrap();
        fs::write(
            extension.join("extension.toml"),
            format!(
                "[extension]\nschema = 1\nid = \"{id}\"\nname = \"{id}\"\nversion = \"1.0.0\"\ndescription = \"Repository extension\"\nformat = \"text\"\ndestination = \"{id}/style.conf\"\ntemplate = \"template.txt\"\n"
            ),
        )
        .unwrap();
        fs::write(
            extension.join("template.txt"),
            "accent = {{colors.primary}}\n",
        )
        .unwrap();
    }

    const INDEX: &str = r#"
[repository]
schema = 1
name = "Test Repository"
description = "Packages used by the tests"

[[package]]
id = "aurora"
kind = "theme"
name = "Aurora"
version = "1.0.0"
description = "A repository theme"
source = "themes/aurora"

[[package]]
id = "aurora-widget"
kind = "extension"
name = "Aurora widget"
version = "1.0.0"
description = "A repository extension"
source = "extensions/aurora-widget"
"#;

    #[test]
    fn loads_an_index_and_resolves_package_sources() {
        let root = scratch("index");

        write_theme(&root.join("themes"), "aurora");
        write_extension(&root.join("extensions"), "aurora-widget");
        fs::write(root.join(INDEX_FILE), INDEX).unwrap();

        let index = load(root.to_str().unwrap(), root.join("cache")).unwrap();

        assert_eq!(index.name, "Test Repository");
        assert_eq!(index.packages.len(), 2);

        let theme = index.find("aurora").unwrap();

        assert_eq!(theme.kind, PackageKind::Theme);
        assert!(theme.source.join("colors/palette.toml").is_file());
        assert_eq!(index.search("widget").len(), 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installs_a_theme_and_an_extension_package() {
        let root = scratch("install");
        let themes = root.join("themes-source");
        let extensions_source = root.join("extensions-source");

        write_theme(&themes, "aurora");
        write_extension(&extensions_source, "aurora-widget");

        let packages = root.join("repo");

        fs::create_dir_all(&packages).unwrap();
        fs::write(
            packages.join(INDEX_FILE),
            INDEX
                .replace(
                    "source = \"themes/aurora\"",
                    &format!("source = \"{}\"", themes.join("aurora").display()),
                )
                .replace(
                    "source = \"extensions/aurora-widget\"",
                    &format!(
                        "source = \"{}\"",
                        extensions_source.join("aurora-widget").display()
                    ),
                ),
        )
        .unwrap();

        let index = load(packages.to_str().unwrap(), root.join("cache")).unwrap();
        let user_themes = root.join("user-themes");
        let extension_registry = ExtensionRegistry::new(
            root.join("builtin-extensions"),
            root.join("user-extensions"),
        );

        let theme = index.find("aurora").unwrap();

        let installed = install(theme, &user_themes, &extension_registry, false).unwrap();

        assert!(installed.ends_with("aurora/colors/palette.toml"));
        assert_eq!(
            ThemeRegistry::new(vec![user_themes.clone()])
                .load("aurora")
                .unwrap()
                .colors
                .primary
                .to_hex(),
            "#FF0055"
        );

        // A second install without --force is refused.
        assert!(install(theme, &user_themes, &extension_registry, false).is_err());
        assert!(install(theme, &user_themes, &extension_registry, true).is_ok());

        let extension = index.find("aurora-widget").unwrap();
        let installed = install(extension, &user_themes, &extension_registry, false).unwrap();

        assert!(installed.join("extension.toml").is_file());
        assert!(extension_registry.load("aurora-widget").is_ok());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fetches_a_local_and_a_file_url() {
        let root = scratch("fetch");

        fs::write(root.join(INDEX_FILE), INDEX).unwrap();

        assert_eq!(
            fetch(root.to_str().unwrap(), root.join("cache")).unwrap(),
            root
        );

        let url = format!("file://{}", root.display());

        assert_eq!(fetch(&url, root.join("cache")).unwrap(), root);

        assert!(load(&url, root.join("cache")).is_ok());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_a_missing_location() {
        let root = scratch("missing");

        assert!(fetch("/nonexistent/my-linux-repository", root.join("cache")).is_err());

        fs::remove_dir_all(root).unwrap();
    }
}
