//! Profile discovery, saving and removal.
//!
//! User profiles live in `$MY_LINUX_USER_PROFILES` (default
//! `$XDG_DATA_HOME/my-linux/profiles`) and shadow built-in profiles with the
//! same id.

use std::fs;
use std::path::{Path, PathBuf};

use crate::profile::{Profile, ProfileError};

/// Where a profile was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSource {
    User,
    Builtin,
}

impl ProfileSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Builtin => "builtin",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub theme: String,
    pub wallpaper: Option<String>,
    pub integrations: usize,
    pub source: ProfileSource,
    pub path: PathBuf,
}

/// A user profile directory and the built-in profile directory.
#[derive(Debug, Clone)]
pub struct ProfileRegistry {
    user: PathBuf,
    builtin: PathBuf,
}

impl ProfileRegistry {
    pub fn new(builtin: impl Into<PathBuf>, user: impl Into<PathBuf>) -> Self {
        Self {
            user: user.into(),
            builtin: builtin.into(),
        }
    }

    pub fn user_root(&self) -> &Path {
        &self.user
    }

    pub fn builtin_root(&self) -> &Path {
        &self.builtin
    }

    /// File a profile was loaded from; user profiles win.
    pub fn find(&self, id: &str) -> Option<PathBuf> {
        let id = id.trim().to_lowercase();

        if id.is_empty() {
            return None;
        }

        for root in [&self.user, &self.builtin] {
            let directory = root.join(&id).join("profile.toml");

            if directory.is_file() {
                return Some(directory);
            }

            let file = root.join(format!("{id}.toml"));

            if file.is_file() {
                return Some(file);
            }
        }

        None
    }

    /// Loads a profile by id.
    pub fn load(&self, id: &str) -> Result<Profile, ProfileError> {
        let file = self
            .find(id)
            .ok_or_else(|| ProfileError::Unknown(id.to_string()))?;

        Profile::from_path(file)
    }

    /// Every profile, user profiles first and shadowing built-ins.
    pub fn list(&self) -> Vec<ProfileSummary> {
        let mut summaries: Vec<ProfileSummary> = Vec::new();

        for (root, source) in [
            (&self.user, ProfileSource::User),
            (&self.builtin, ProfileSource::Builtin),
        ] {
            let Ok(entries) = fs::read_dir(root) else {
                continue;
            };

            let mut paths: Vec<PathBuf> = entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .collect();

            paths.sort();

            for path in paths {
                let Some(id) = id_of(&path) else {
                    continue;
                };

                if summaries.iter().any(|summary| summary.id == id) {
                    continue;
                }

                let file = if path.is_dir() {
                    path.join("profile.toml")
                } else {
                    path.clone()
                };

                let Ok(profile) = Profile::from_path(&file) else {
                    continue;
                };

                summaries.push(ProfileSummary {
                    id: profile.metadata.id.clone(),
                    name: profile.metadata.name.clone(),
                    theme: profile.metadata.theme.clone(),
                    wallpaper: profile.metadata.wallpaper.clone(),
                    integrations: profile.metadata.integrations.len(),
                    source,
                    path: file,
                });
            }
        }

        summaries.sort_by(|left, right| left.id.cmp(&right.id));

        summaries
    }

    /// Writes a user profile and returns the file it was written to.
    pub fn save(&self, profile: &Profile) -> Result<PathBuf, ProfileError> {
        profile.validate()?;

        let file = self.user.join(&profile.metadata.id).join("profile.toml");

        let parent = file
            .parent()
            .ok_or_else(|| ProfileError::Invalid("Invalid profile path".to_string()))?;

        fs::create_dir_all(parent)?;
        fs::write(&file, profile.to_toml()?)?;

        Ok(file)
    }

    /// Removes a user profile. Built-in profiles are never removed.
    pub fn remove(&self, id: &str) -> Result<PathBuf, ProfileError> {
        let id = id.trim().to_lowercase();

        let file = self.user.join(format!("{id}.toml"));

        if file.is_file() {
            fs::remove_file(&file)?;
            return Ok(file);
        }

        let directory = self.user.join(&id);

        if directory.join("profile.toml").is_file() {
            fs::remove_dir_all(&directory)?;
            return Ok(directory);
        }

        if self.builtin.join(&id).join("profile.toml").is_file()
            || self.builtin.join(format!("{id}.toml")).is_file()
        {
            return Err(ProfileError::Builtin(id));
        }

        Err(ProfileError::Unknown(id))
    }
}

fn id_of(path: &Path) -> Option<String> {
    if path.is_dir() {
        return path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_lowercase());
    }

    path.file_stem()
        .and_then(|name| name.to_str())
        .map(|name| name.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("my-linux-profiles-{}-{}", name, std::process::id()));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        directory
    }

    const BASE: &str = r#"
[profile]
schema = 1
id = "dev"
name = "Developer"
version = "1.0.0"
description = "A profile"
theme = "cyberpunk"
integrations = ["waybar"]
"#;

    #[test]
    fn loads_builtin_presets() {
        let registry = ProfileRegistry::new("../../profiles", scratch("user"));

        let summaries = registry.list();

        assert_eq!(summaries.len(), 4, "four built-in presets are expected");
        assert!(
            summaries
                .iter()
                .all(|summary| summary.source == ProfileSource::Builtin)
        );
        assert!(summaries.iter().any(|summary| summary.id == "gaming"));

        for summary in summaries {
            let profile = registry
                .load(&summary.id)
                .expect("every preset should load");

            assert_eq!(profile.metadata.id, summary.id);
            assert!(!profile.metadata.theme.is_empty());
        }
    }

    #[test]
    fn saves_lists_and_removes_a_user_profile() {
        let directory = scratch("save");
        let registry = ProfileRegistry::new(directory.join("builtin"), directory.join("user"));
        let profile = Profile::parse(BASE, "dev.toml").unwrap();

        let file = registry.save(&profile).unwrap();

        assert!(file.ends_with("user/dev/profile.toml"));
        assert_eq!(registry.load("dev").unwrap().metadata.name, "Developer");
        assert_eq!(registry.list()[0].source, ProfileSource::User);

        registry.remove("dev").unwrap();
        assert!(registry.load("dev").is_err());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn user_profiles_shadow_builtin_profiles() {
        let directory = scratch("shadow");
        let builtin = directory.join("builtin");

        fs::create_dir_all(builtin.join("dev")).unwrap();
        fs::write(builtin.join("dev").join("profile.toml"), BASE).unwrap();

        let registry = ProfileRegistry::new(&builtin, directory.join("user"));
        let mut profile = Profile::parse(BASE, "dev.toml").unwrap();

        profile.metadata.name = "User Developer".to_string();
        registry.save(&profile).unwrap();

        assert_eq!(
            registry.load("dev").unwrap().metadata.name,
            "User Developer"
        );
        assert_eq!(registry.list().len(), 1);
        assert_eq!(registry.list()[0].source, ProfileSource::User);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn refuses_to_remove_builtin_profiles() {
        let directory = scratch("builtin");
        let registry = ProfileRegistry::new("../../profiles", directory.join("user"));

        match registry.remove("gaming") {
            Err(ProfileError::Builtin(id)) => assert_eq!(id, "gaming"),
            other => panic!("built-in profiles must be protected, got {other:?}"),
        }
    }
}
