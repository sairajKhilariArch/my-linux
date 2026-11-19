//! Filesystem layout of an installation.

use std::path::{Path, PathBuf};

use crate::{DESKTOP_FILE_NAME, operations::InstallError};

/// An installation prefix such as `~/.local`.
#[derive(Debug, Clone)]
pub struct InstallLayout {
    pub prefix: PathBuf,
}

impl InstallLayout {
    pub fn new(prefix: impl Into<PathBuf>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.prefix.join("bin")
    }

    pub fn bin(&self, name: &str) -> PathBuf {
        self.bin_dir().join(name)
    }

    pub fn share_dir(&self) -> PathBuf {
        self.prefix.join("share").join("my-linux")
    }

    pub fn themes_dir(&self) -> PathBuf {
        self.share_dir().join("themes")
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.share_dir().join("profiles")
    }

    pub fn extensions_dir(&self) -> PathBuf {
        self.share_dir().join("extensions")
    }

    pub fn applications_dir(&self) -> PathBuf {
        self.prefix.join("share").join("applications")
    }

    pub fn desktop_file(&self) -> PathBuf {
        self.applications_dir().join(DESKTOP_FILE_NAME)
    }

    pub fn icons_dir(&self) -> PathBuf {
        self.prefix
            .join("share")
            .join("icons")
            .join("hicolor")
            .join("scalable")
            .join("apps")
    }

    pub fn icon_file(&self) -> PathBuf {
        self.icons_dir().join("my-linux.svg")
    }

    pub fn manifest_file(&self) -> PathBuf {
        self.share_dir().join("install-manifest.toml")
    }

    pub fn directories(&self) -> Vec<PathBuf> {
        vec![
            self.bin_dir(),
            self.share_dir(),
            self.themes_dir(),
            self.profiles_dir(),
            self.extensions_dir(),
            self.applications_dir(),
            self.icons_dir(),
        ]
    }

    /// Path relative to the prefix, always with `/` separators.
    pub fn relative(&self, path: &Path) -> String {
        match path.strip_prefix(&self.prefix) {
            Ok(relative) => relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<String>>()
                .join("/"),
            Err(_) => path.to_string_lossy().to_string(),
        }
    }

    /// The recorded state directory, outside the prefix.
    pub fn state_dir(&self) -> PathBuf {
        crate::layout::state_dir()
    }

    /// True when `path` is inside the prefix.
    pub fn owns(&self, path: &Path) -> bool {
        path.starts_with(&self.prefix)
    }

    /// Makes sure the prefix looks like a My Linux installation.
    pub fn require_manifest(&self) -> Result<crate::Manifest, InstallError> {
        crate::manifest::read(self.manifest_file())?.ok_or_else(|| {
            InstallError::Message(format!(
                "My Linux is not installed under {}; run `my-linux-theme install` first",
                self.prefix.display()
            ))
        })
    }
}

/// Default prefix: `$MY_LINUX_PREFIX`, else the parent of the XDG data home.
pub fn default_prefix() -> PathBuf {
    if let Some(prefix) = std::env::var_os("MY_LINUX_PREFIX") {
        return PathBuf::from(prefix);
    }

    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"));

    data_home
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(".local"))
}

/// My Linux state directory (`$XDG_STATE_HOME/my-linux`).
pub fn state_dir() -> PathBuf {
    if let Some(state) = std::env::var_os("MY_LINUX_ROOT") {
        return PathBuf::from(state);
    }

    let state_home = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from(".local/state"));

    state_home.join("my-linux")
}
