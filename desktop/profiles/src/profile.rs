//! Profile schema, validation and TOML serialisation.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Schema version understood by this build.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub enum ProfileError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    Serialize(toml::ser::Error),
    Invalid(String),
    UnknownIntegration(String),
    Unknown(String),
    Builtin(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Toml(error) => write!(f, "Invalid profile TOML: {error}"),
            Self::Serialize(error) => write!(f, "Could not write the profile: {error}"),
            Self::Invalid(message) => write!(f, "Invalid profile: {message}"),
            Self::UnknownIntegration(id) => write!(f, "Unknown integration `{id}`"),
            Self::Unknown(id) => write!(f, "Unknown profile: {id}"),
            Self::Builtin(id) => write!(f, "Built-in profiles cannot be modified: {id}"),
        }
    }
}

impl std::error::Error for ProfileError {}

impl From<std::io::Error> for ProfileError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<toml::de::Error> for ProfileError {
    fn from(error: toml::de::Error) -> Self {
        Self::Toml(error)
    }
}

impl From<toml::ser::Error> for ProfileError {
    fn from(error: toml::ser::Error) -> Self {
        Self::Serialize(error)
    }
}

/// Identity and references of a desktop profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileMetadata {
    pub schema: u32,
    pub id: String,
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub wallpaper: Option<String>,
    /// Integration ids to install; empty means every integration.
    #[serde(default)]
    pub integrations: Vec<String>,
}

impl Default for ProfileMetadata {
    fn default() -> Self {
        Self {
            schema: SCHEMA_VERSION,
            id: String::new(),
            name: String::new(),
            version: default_version(),
            description: String::new(),
            author: None,
            theme: String::new(),
            wallpaper: None,
            integrations: Vec::new(),
        }
    }
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// Font, icon and cursor settings carried by a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileAssets {
    pub font: Option<String>,
    pub font_size: u32,
    pub icon_theme: Option<String>,
    pub cursor_theme: Option<String>,
    pub cursor_size: u32,
}

impl Default for ProfileAssets {
    fn default() -> Self {
        Self {
            font: None,
            font_size: 11,
            icon_theme: None,
            cursor_theme: None,
            cursor_size: 24,
        }
    }
}

/// Window-manager layout settings carried by a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileLayout {
    pub gaps_in: u32,
    pub gaps_out: u32,
    pub border_size: u32,
    pub rounding: u32,
    pub active_opacity: f64,
    pub inactive_opacity: f64,
    pub blur: bool,
}

impl Default for ProfileLayout {
    fn default() -> Self {
        Self {
            gaps_in: 5,
            gaps_out: 10,
            border_size: 2,
            rounding: 10,
            active_opacity: 1.0,
            inactive_opacity: 0.95,
            blur: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileFile {
    profile: ProfileMetadata,
    #[serde(default)]
    assets: ProfileAssets,
    #[serde(default)]
    layout: ProfileLayout,
}

/// A complete desktop configuration loaded from disk.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub metadata: ProfileMetadata,
    pub assets: ProfileAssets,
    pub layout: ProfileLayout,
    /// File the profile was loaded from; empty for a profile created in memory.
    pub path: PathBuf,
}

impl Profile {
    /// A new custom profile referencing an existing theme.
    pub fn new(id: &str, name: &str, theme: &str) -> Result<Self, ProfileError> {
        let mut profile = Self {
            metadata: ProfileMetadata {
                schema: SCHEMA_VERSION,
                id: id.to_string(),
                name: name.to_string(),
                version: default_version(),
                description: format!("Custom desktop profile {name}"),
                author: Some("My Linux".to_string()),
                theme: theme.to_string(),
                wallpaper: None,
                integrations: Vec::new(),
            },
            assets: ProfileAssets::default(),
            layout: ProfileLayout::default(),
            path: PathBuf::new(),
        };

        profile.normalize();
        profile.validate()?;

        Ok(profile)
    }

    /// Loads a profile from a TOML file.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)?;

        Self::parse(&contents, path)
    }

    /// Parses profile TOML. `source` is only used for the recorded path.
    pub fn parse(contents: &str, source: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let file: ProfileFile = toml::from_str(contents)?;

        let mut profile = Self {
            metadata: file.profile,
            assets: file.assets,
            layout: file.layout,
            path: source.as_ref().to_path_buf(),
        };

        profile.normalize();
        profile.validate()?;

        Ok(profile)
    }

    /// Renders the profile as TOML that can be imported back.
    pub fn to_toml(&self) -> Result<String, ProfileError> {
        let file = ProfileFile {
            profile: self.metadata.clone(),
            assets: self.assets.clone(),
            layout: self.layout.clone(),
        };

        let body = toml::to_string_pretty(&file)?;

        Ok(format!(
            "# My Linux desktop profile.\n# Apply it with: my-linux-theme profile apply {}\n\n{body}",
            self.metadata.id
        ))
    }

    /// Trims, lowercases ids and removes empty values.
    pub fn normalize(&mut self) {
        self.metadata.id = self.metadata.id.trim().to_lowercase();
        self.metadata.name = self.metadata.name.trim().to_string();
        self.metadata.version = self.metadata.version.trim().to_string();
        self.metadata.description = self.metadata.description.trim().to_string();
        self.metadata.theme = self.metadata.theme.trim().to_lowercase();
        self.metadata.wallpaper = self
            .metadata
            .wallpaper
            .take()
            .map(|wallpaper| wallpaper.trim().to_string())
            .filter(|wallpaper| !wallpaper.is_empty());

        let mut integrations: Vec<String> = self
            .metadata
            .integrations
            .iter()
            .map(|integration| integration.trim().to_lowercase())
            .filter(|integration| !integration.is_empty())
            .collect();

        integrations.sort();
        integrations.dedup();

        self.metadata.integrations = integrations;

        self.assets.font = normalize_option(self.assets.font.take());
        self.assets.icon_theme = normalize_option(self.assets.icon_theme.take());
        self.assets.cursor_theme = normalize_option(self.assets.cursor_theme.take());
    }

    /// Rejects profiles that could not be applied safely.
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.metadata.schema != SCHEMA_VERSION {
            return Err(ProfileError::Invalid(format!(
                "unsupported schema {}; this build supports {SCHEMA_VERSION}",
                self.metadata.schema
            )));
        }

        if !valid_id(&self.metadata.id) {
            return Err(ProfileError::Invalid(format!(
                "invalid profile id `{}`: use lowercase letters, digits, '-' or '_'",
                self.metadata.id
            )));
        }

        if self.metadata.name.trim().is_empty() {
            return Err(ProfileError::Invalid(
                "the profile name must not be empty".to_string(),
            ));
        }

        if !valid_id(&self.metadata.theme) {
            return Err(ProfileError::Invalid(format!(
                "invalid theme reference `{}`",
                self.metadata.theme
            )));
        }

        if !valid_version(&self.metadata.version) {
            return Err(ProfileError::Invalid(format!(
                "invalid version `{}`: expected MAJOR.MINOR.PATCH",
                self.metadata.version
            )));
        }

        for integration in &self.metadata.integrations {
            if desktop_integrations::find(integration).is_none() {
                return Err(ProfileError::UnknownIntegration(integration.clone()));
            }
        }

        if self.assets.font_size == 0 {
            return Err(ProfileError::Invalid(
                "font_size must be greater than zero".to_string(),
            ));
        }

        if self.assets.cursor_size == 0 {
            return Err(ProfileError::Invalid(
                "cursor_size must be greater than zero".to_string(),
            ));
        }

        for (label, opacity) in [
            ("active_opacity", self.layout.active_opacity),
            ("inactive_opacity", self.layout.inactive_opacity),
        ] {
            if !(0.0..=1.0).contains(&opacity) {
                return Err(ProfileError::Invalid(format!(
                    "{label} must be between 0.0 and 1.0"
                )));
            }
        }

        Ok(())
    }
}

fn normalize_option(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
        })
}

fn valid_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();

    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = r#"
[profile]
schema = 1
id = "dev"
name = "Developer"
version = "1.0.0"
description = "A profile"
theme = "cyberpunk"
wallpaper = "/wallpapers/neon.png"
integrations = ["waybar", "kitty", "waybar"]

[assets]
font = "JetBrainsMono Nerd Font"
font_size = 12
icon_theme = "Papirus-Dark"
cursor_theme = "Bibata"
cursor_size = 28

[layout]
gaps_in = 4
gaps_out = 8
border_size = 2
rounding = 12
active_opacity = 1.0
inactive_opacity = 0.9
blur = false
"#;

    #[test]
    fn parses_and_normalises_a_profile() {
        let profile = Profile::parse(PROFILE, "/profiles/dev/profile.toml").unwrap();

        assert_eq!(profile.metadata.id, "dev");
        assert_eq!(profile.metadata.theme, "cyberpunk");
        assert_eq!(
            profile.metadata.integrations,
            vec!["kitty".to_string(), "waybar".to_string()]
        );
        assert_eq!(profile.assets.font_size, 12);
        assert_eq!(profile.assets.cursor_size, 28);
        assert_eq!(profile.layout.rounding, 12);
        assert!(!profile.layout.blur);
        assert_eq!(profile.path, PathBuf::from("/profiles/dev/profile.toml"));
    }

    #[test]
    fn round_trips_through_toml() {
        let profile = Profile::parse(PROFILE, "/profiles/dev/profile.toml").unwrap();
        let rendered = profile.to_toml().unwrap();

        assert!(rendered.contains("[profile]"));
        assert!(rendered.contains("[assets]"));
        assert!(rendered.contains("[layout]"));

        let reimported = Profile::parse(&rendered, "/profiles/dev/profile.toml").unwrap();

        assert_eq!(reimported.metadata, profile.metadata);
        assert_eq!(reimported.assets, profile.assets);
        assert_eq!(reimported.layout, profile.layout);
    }

    #[test]
    fn fills_missing_settings_with_defaults() {
        let profile = Profile::parse(
            r#"
[profile]
schema = 1
id = "plain"
name = "Plain"
theme = "minimal"
"#,
            "plain.toml",
        )
        .unwrap();

        assert_eq!(profile.assets.font_size, 11);
        assert_eq!(profile.assets.cursor_size, 24);
        assert_eq!(profile.layout.gaps_in, 5);
        assert!(profile.layout.blur);
        assert!(profile.metadata.integrations.is_empty());
    }

    #[test]
    fn rejects_invalid_profiles() {
        let base = PROFILE.to_string();

        assert!(
            Profile::parse(&base.replace("schema = 1", "schema = 9"), "dev.toml").is_err(),
            "schema version is checked"
        );
        assert!(
            Profile::parse(
                &base.replace("id = \"dev\"", "id = \"Dev Profile\""),
                "dev.toml"
            )
            .is_err(),
            "ids are validated"
        );
        assert!(
            Profile::parse(
                &base.replace("theme = \"cyberpunk\"", "theme = \"\""),
                "dev.toml"
            )
            .is_err(),
            "a theme reference is required"
        );
        assert!(
            Profile::parse(
                &base.replace("\"waybar\", \"kitty\", \"waybar\"", "\"waybar\", \"nope\""),
                "dev.toml"
            )
            .is_err(),
            "unknown integrations are rejected"
        );
        assert!(
            Profile::parse(
                &base.replace("active_opacity = 1.0", "active_opacity = 2.0"),
                "dev.toml"
            )
            .is_err(),
            "opacities are validated"
        );
    }

    #[test]
    fn creates_custom_profiles_with_defaults() {
        let profile = Profile::new("mine", "Mine", "nature").unwrap();

        assert_eq!(profile.metadata.schema, SCHEMA_VERSION);
        assert_eq!(profile.metadata.theme, "nature");
        assert!(profile.metadata.integrations.is_empty());
        assert!(profile.to_toml().unwrap().contains("id = \"mine\""));
    }
}
