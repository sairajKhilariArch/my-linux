//! The installation manifest: what was installed, from which version, when.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::operations::InstallError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Manifest {
    pub version: String,
    pub installed_at: u64,
    pub prefix: String,
    pub platform: String,
    #[serde(default)]
    pub files: Vec<ManifestFile>,
    #[serde(default)]
    pub preserved: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestFile {
    /// Path relative to the installation prefix.
    pub path: String,
    pub digest: String,
    /// `binary`, `theme`, `profile`, `desktop` or `icon`.
    pub kind: String,
}

impl Manifest {
    pub fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0)
    }

    pub fn render(&self) -> Result<String, InstallError> {
        toml::to_string(self).map_err(InstallError::Serialize)
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), InstallError> {
        let rendered = self.render()?;

        InstallError::write_text(path.as_ref(), &rendered)
    }

    pub fn digest_of(&self, relative_path: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|file| file.path == relative_path)
            .map(|file| file.digest.as_str())
    }
}

/// Reads a manifest; `None` when the file does not exist.
pub fn read(path: impl AsRef<Path>) -> Result<Option<Manifest>, InstallError> {
    match fs::read_to_string(path.as_ref()) {
        Ok(contents) => toml::from_str(&contents)
            .map(Some)
            .map_err(InstallError::Toml),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
