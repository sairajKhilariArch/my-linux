//! Theme loading, validation and discovery.
//!
//! Themes are read from one or more roots. The first root wins, so a user
//! theme can override a built-in theme with the same id.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::Color;

/// Schema version understood by this build.
pub const SCHEMA_VERSION: u32 = 1;

const COLOR_KEYS: [&str; 11] = [
    "background",
    "surface",
    "surface_alt",
    "primary",
    "secondary",
    "text",
    "text_muted",
    "border",
    "success",
    "warning",
    "error",
];

#[derive(Debug, Clone, Deserialize)]
struct RawMetadata {
    name: String,
    id: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    schema: Option<u32>,
    #[serde(default)]
    extends: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawThemeFile {
    theme: RawMetadata,
    #[serde(default)]
    colors: toml::Table,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeMetadata {
    pub name: String,
    pub id: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub schema: u32,
    pub extends: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub background: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub primary: Color,
    pub secondary: Color,
    pub text: Color,
    pub text_muted: Color,
    pub border: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub active: Color,
    pub hover: Color,
    pub selected: Color,
}

#[derive(Debug, Clone)]
pub struct LoadedTheme {
    pub metadata: ThemeMetadata,
    pub colors: ThemeColors,
    /// Directory or file the theme was loaded from.
    pub path: PathBuf,
}

#[derive(Debug)]
pub enum ThemeLoadError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    InvalidColor(String),
    UnknownTheme(String),
    InvalidMetadata(String),
    UnsupportedSchema(u32),
    MissingColor(String),
    InheritanceCycle(String),
}

impl std::fmt::Display for ThemeLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Could not read theme file: {error}"),
            Self::Toml(error) => write!(f, "Invalid TOML: {error}"),
            Self::InvalidColor(color) => write!(f, "Invalid hexadecimal color: {color}"),
            Self::UnknownTheme(name) => write!(f, "Unknown theme: {name}"),
            Self::InvalidMetadata(message) => write!(f, "Invalid theme metadata: {message}"),
            Self::UnsupportedSchema(version) => write!(
                f,
                "Unsupported theme schema version {version}; this build supports {SCHEMA_VERSION}"
            ),
            Self::MissingColor(key) => write!(f, "Theme is missing the colour `{key}`"),
            Self::InheritanceCycle(name) => {
                write!(f, "Theme inheritance cycle detected at `{name}`")
            }
        }
    }
}

impl std::error::Error for ThemeLoadError {}

impl From<std::io::Error> for ThemeLoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<toml::de::Error> for ThemeLoadError {
    fn from(error: toml::de::Error) -> Self {
        Self::Toml(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSource {
    User,
    Builtin,
}

impl ThemeSource {
    pub const fn label(self) -> &'static str {
        match self {
            ThemeSource::User => "user",
            ThemeSource::Builtin => "builtin",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThemeSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub path: PathBuf,
    pub source: ThemeSource,
}

#[derive(Debug, Clone)]
pub struct BrokenTheme {
    pub path: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone, Default)]
pub struct Discovery {
    pub themes: Vec<ThemeSummary>,
    pub broken: Vec<BrokenTheme>,
}

impl Discovery {
    pub fn errors(&self) -> Vec<String> {
        self.broken
            .iter()
            .map(|broken| format!("{}: {}", broken.path.display(), broken.error))
            .collect()
    }
}

/// A theme root and where themes found in it come from.
#[derive(Debug, Clone)]
struct Root {
    path: PathBuf,
    source: ThemeSource,
}

/// Where a theme id was found.
#[derive(Debug, Clone)]
pub struct ThemeLocation {
    pub id: String,
    pub path: PathBuf,
    pub source: ThemeSource,
}

#[derive(Debug, Clone)]
struct ResolvedTheme {
    metadata: ThemeMetadata,
    values: PaletteValues,
}

/// A registry of theme roots searched in order.
#[derive(Debug, Clone)]
pub struct Registry {
    roots: Vec<Root>,
}

impl Registry {
    /// Builds a registry where every root contains built-in themes.
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots: roots
                .into_iter()
                .map(|path| Root {
                    path,
                    source: ThemeSource::Builtin,
                })
                .collect(),
        }
    }

    /// Builds a registry where user themes take precedence over built-ins.
    pub fn with_user(builtin: impl Into<PathBuf>, user: impl Into<PathBuf>) -> Self {
        Self {
            roots: vec![
                Root {
                    path: user.into(),
                    source: ThemeSource::User,
                },
                Root {
                    path: builtin.into(),
                    source: ThemeSource::Builtin,
                },
            ],
        }
    }

    pub fn root_paths(&self) -> Vec<PathBuf> {
        self.roots.iter().map(|root| root.path.clone()).collect()
    }

    /// Finds a theme by id, preferring earlier roots.
    pub fn find(&self, id: &str) -> Option<ThemeLocation> {
        let id = id.trim().to_lowercase();

        if id.is_empty() {
            return None;
        }

        for root in &self.roots {
            let directory = root.path.join(&id);

            if directory.is_dir() && theme_file(&directory).is_ok() {
                return Some(ThemeLocation {
                    id,
                    path: directory,
                    source: root.source,
                });
            }

            let file = root.path.join(format!("{id}.toml"));

            if file.is_file() {
                return Some(ThemeLocation {
                    id,
                    path: file,
                    source: root.source,
                });
            }
        }

        None
    }

    /// Loads a theme by id, resolving inheritance.
    pub fn load(&self, id: &str) -> Result<LoadedTheme, ThemeLoadError> {
        let entry = self
            .find(id)
            .ok_or_else(|| ThemeLoadError::UnknownTheme(id.to_string()))?;

        let resolved = self.load_entry(&entry, &mut Vec::new())?;
        let colors = resolved.values.into_colors()?;

        Ok(LoadedTheme {
            metadata: resolved.metadata,
            colors,
            path: entry.path,
        })
    }

    /// Lists every theme and every broken theme file found in the roots.
    pub fn list(&self) -> Discovery {
        let mut discovery = Discovery::default();
        let mut listed: Vec<String> = Vec::new();

        for root in &self.roots {
            let Ok(entries) = fs::read_dir(&root.path) else {
                continue;
            };

            let mut paths: Vec<PathBuf> = entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .collect();

            paths.sort();

            for path in paths {
                let Some(id) = theme_id_of(&path) else {
                    continue;
                };

                if !is_candidate(&path) {
                    continue;
                }

                if listed.contains(&id) {
                    continue;
                }

                match self.load(&id) {
                    Ok(theme) => {
                        listed.push(id);

                        discovery.themes.push(ThemeSummary {
                            id: theme.metadata.id.clone(),
                            name: theme.metadata.name.clone(),
                            version: theme.metadata.version.clone(),
                            description: theme.metadata.description.clone(),
                            path,
                            source: root.source,
                        });
                    }
                    Err(error) => discovery.broken.push(BrokenTheme {
                        path,
                        error: error.to_string(),
                    }),
                }
            }
        }

        discovery
    }

    /// Loads a theme directly from a directory or a single `.toml` file.
    pub fn load_path(path: impl AsRef<Path>) -> Result<LoadedTheme, ThemeLoadError> {
        let path = path.as_ref();
        let parent = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let id = theme_id_of(path)
            .ok_or_else(|| ThemeLoadError::UnknownTheme(path.display().to_string()))?;

        Registry::new(vec![parent]).load(&id)
    }

    fn load_entry(
        &self,
        entry: &ThemeLocation,
        seen: &mut Vec<String>,
    ) -> Result<ResolvedTheme, ThemeLoadError> {
        if seen.contains(&entry.id) {
            return Err(ThemeLoadError::InheritanceCycle(entry.id.clone()));
        }

        seen.push(entry.id.clone());

        let file = theme_file(&entry.path)?;
        let contents = fs::read_to_string(file)?;
        let raw: RawThemeFile = toml::from_str(&contents)?;

        let expected_id = entry.path.is_dir().then_some(entry.id.as_str());
        let metadata = validate_metadata(raw.theme, expected_id)?;

        let mut values = PaletteValues::default();

        if let Some(parent) = &metadata.extends {
            let parent_entry = self
                .find(parent)
                .ok_or_else(|| ThemeLoadError::UnknownTheme(parent.clone()))?;

            values = self.load_entry(&parent_entry, seen)?.values;
        }

        values.merge(&raw.colors);

        Ok(ResolvedTheme { metadata, values })
    }
}

#[derive(Debug, Clone, Default)]
struct PaletteValues {
    colors: BTreeMap<String, String>,
    states: BTreeMap<String, String>,
}

impl PaletteValues {
    fn merge(&mut self, table: &toml::Table) {
        for (key, value) in table {
            if key == "states" {
                if let toml::Value::Table(states) = value {
                    for (state, value) in states {
                        if let toml::Value::String(text) = value {
                            self.states.insert(state.clone(), text.clone());
                        }
                    }
                }

                continue;
            }

            if COLOR_KEYS.contains(&key.as_str())
                && let toml::Value::String(text) = value
            {
                self.colors.insert(key.clone(), text.clone());
            }
        }
    }

    fn into_colors(self) -> Result<ThemeColors, ThemeLoadError> {
        let color = |key: &str| -> Result<Color, ThemeLoadError> {
            let value = self
                .colors
                .get(key)
                .ok_or_else(|| ThemeLoadError::MissingColor(key.to_string()))?;

            parse_color(value)
        };

        let state = |key: &str| -> Result<Color, ThemeLoadError> {
            let value = self
                .states
                .get(key)
                .ok_or_else(|| ThemeLoadError::MissingColor(format!("states.{key}")))?;

            parse_color(value)
        };

        Ok(ThemeColors {
            background: color("background")?,
            surface: color("surface")?,
            surface_alt: color("surface_alt")?,
            primary: color("primary")?,
            secondary: color("secondary")?,
            text: color("text")?,
            text_muted: color("text_muted")?,
            border: color("border")?,
            success: color("success")?,
            warning: color("warning")?,
            error: color("error")?,
            active: state("active")?,
            hover: state("hover")?,
            selected: state("selected")?,
        })
    }
}

fn is_candidate(path: &Path) -> bool {
    if path.is_dir() {
        return theme_file(path).is_ok();
    }

    path.extension()
        .map(|extension| extension.eq_ignore_ascii_case("toml"))
        .unwrap_or(false)
}

fn theme_id_of(path: &Path) -> Option<String> {
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

/// The TOML file that defines a theme directory or theme file.
fn theme_file(path: &Path) -> Result<PathBuf, ThemeLoadError> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }

    let palette = path.join("colors").join("palette.toml");

    if palette.is_file() {
        return Ok(palette);
    }

    let legacy = path.join("theme.toml");

    if legacy.is_file() {
        return Ok(legacy);
    }

    Err(ThemeLoadError::UnknownTheme(path.display().to_string()))
}

pub fn parse_color(value: &str) -> Result<Color, ThemeLoadError> {
    Color::from_hex(value).ok_or_else(|| ThemeLoadError::InvalidColor(value.to_string()))
}

/// Checks that a theme id is usable as a directory and command argument.
pub fn validate_id(id: &str) -> Result<(), ThemeLoadError> {
    let valid = !id.is_empty()
        && id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
        });

    if valid {
        Ok(())
    } else {
        Err(ThemeLoadError::InvalidMetadata(format!(
            "invalid theme id `{id}`: use lowercase letters, digits, '-' or '_'"
        )))
    }
}

fn validate_version(version: &str) -> Result<(), ThemeLoadError> {
    let parts: Vec<&str> = version.split('.').collect();

    let valid = parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()));

    if valid {
        Ok(())
    } else {
        Err(ThemeLoadError::InvalidMetadata(format!(
            "invalid version `{version}`: expected MAJOR.MINOR.PATCH"
        )))
    }
}

fn validate_metadata(
    raw: RawMetadata,
    expected_id: Option<&str>,
) -> Result<ThemeMetadata, ThemeLoadError> {
    let schema = raw.schema.unwrap_or(SCHEMA_VERSION);

    if schema != SCHEMA_VERSION {
        return Err(ThemeLoadError::UnsupportedSchema(schema));
    }

    validate_id(&raw.id)?;

    if let Some(expected) = expected_id
        && expected != raw.id
    {
        return Err(ThemeLoadError::InvalidMetadata(format!(
            "theme id `{}` does not match its directory `{expected}`",
            raw.id
        )));
    }

    if raw.name.trim().is_empty() {
        return Err(ThemeLoadError::InvalidMetadata(
            "theme name must not be empty".to_string(),
        ));
    }

    if raw.description.trim().is_empty() {
        return Err(ThemeLoadError::InvalidMetadata(
            "theme description must not be empty".to_string(),
        ));
    }

    let version = if raw.version.trim().is_empty() {
        "0.1.0".to_string()
    } else {
        raw.version.trim().to_string()
    };

    validate_version(&version)?;

    if let Some(parent) = &raw.extends {
        validate_id(parent)?;
    }

    Ok(ThemeMetadata {
        name: raw.name.trim().to_string(),
        id: raw.id,
        version,
        description: raw.description.trim().to_string(),
        author: raw.author,
        schema,
        extends: raw.extends,
    })
}

/// Loads a theme from a directory or a single `.toml` file.
pub fn load_theme(path: impl AsRef<Path>) -> Result<LoadedTheme, ThemeLoadError> {
    Registry::load_path(path)
}

/// Loads a theme by id from a single directory of themes.
pub fn load_theme_by_id(
    themes_directory: impl AsRef<Path>,
    id: &str,
) -> Result<LoadedTheme, ThemeLoadError> {
    Registry::new(vec![themes_directory.as_ref().to_path_buf()]).load(id)
}

/// Loads a theme by name from a single directory of themes.
pub fn load_theme_by_name(
    themes_directory: impl AsRef<Path>,
    name: &str,
) -> Result<LoadedTheme, ThemeLoadError> {
    load_theme_by_id(themes_directory, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn themes() -> Registry {
        Registry::new(vec!["../../themes".into()])
    }

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("my-linux-loader-{}-{}", name, std::process::id()));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        directory
    }

    fn write_theme(root: &Path, id: &str, contents: &str) {
        let directory = root.join(id).join("colors");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("palette.toml"), contents).unwrap();
    }

    const BASE_THEME: &str = r##"
[theme]
schema = 1
name = "Base"
id = "base"
version = "1.0.0"
description = "Base theme"

[colors]
background = "#000000"
surface = "#111111"
surface_alt = "#222222"
primary = "#FF0000"
secondary = "#00FF00"
text = "#FFFFFF"
text_muted = "#AAAAAA"
border = "#333333"
success = "#00FF00"
warning = "#FFFF00"
error = "#FF0000"

[colors.states]
active = "#FF0000"
hover = "#00FF00"
selected = "#222222"
"##;

    #[test]
    fn loads_futuristic_theme() {
        let theme = themes().load("futuristic").expect("Futuristic should load");

        assert_eq!(theme.metadata.id, "futuristic");
        assert_eq!(theme.colors.background.to_hex(), "#0B1020");
        assert_eq!(theme.colors.primary.to_hex(), "#A78BFA");
        assert_eq!(theme.colors.hover.to_hex(), "#22D3EE");
    }

    #[test]
    fn loads_all_builtin_themes() {
        for name in ["futuristic", "minimal", "cyberpunk", "nature"] {
            let theme = themes().load(name).expect("theme should load");

            assert_eq!(theme.metadata.id, name);
            assert_eq!(theme.metadata.schema, SCHEMA_VERSION);
        }
    }

    #[test]
    fn discovers_builtin_themes() {
        let discovery = themes().list();

        assert_eq!(discovery.themes.len(), 4);
        assert!(discovery.broken.is_empty());
        assert!(
            discovery
                .themes
                .iter()
                .all(|theme| theme.source == ThemeSource::Builtin)
        );
    }

    #[test]
    fn rejects_unknown_themes() {
        let result = themes().load("unknown");

        assert!(matches!(result, Err(ThemeLoadError::UnknownTheme(_))));
    }

    #[test]
    fn discards_invalid_colors() {
        assert!(parse_color("#GG0000").is_err());
        assert!(parse_color("#123").is_err());
        assert!(parse_color("#00D2FF").is_ok());
    }

    #[test]
    fn validates_theme_ids() {
        assert!(validate_id("my-theme").is_ok());
        assert!(validate_id("theme_2").is_ok());
        assert!(validate_id("MyTheme").is_err());
        assert!(validate_id("").is_err());
        assert!(validate_id("../escape").is_err());
    }

    #[test]
    fn loads_a_user_theme_from_an_arbitrary_directory() {
        let root = scratch("user");

        write_theme(&root, "base", BASE_THEME);

        let theme = Registry::new(vec![root.clone()])
            .load("base")
            .expect("user theme should load");

        assert_eq!(theme.metadata.name, "Base");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn user_themes_override_builtin_themes() {
        let builtin = scratch("builtin");
        let user = scratch("override");

        write_theme(
            &builtin,
            "shared",
            &BASE_THEME.replace("id = \"base\"", "id = \"shared\""),
        );
        write_theme(
            &user,
            "shared",
            &BASE_THEME
                .replace("id = \"base\"", "id = \"shared\"")
                .replace("#FF0000", "#0000FF")
                .replace("Base", "User Base"),
        );

        let registry = Registry::with_user(builtin.clone(), user.clone());
        let theme = registry.load("shared").expect("theme should load");

        assert_eq!(theme.metadata.name, "User Base");
        assert_eq!(theme.colors.primary.to_hex(), "#0000FF");

        let discovery = registry.list();

        assert_eq!(discovery.themes.len(), 1);
        assert_eq!(discovery.themes[0].source, ThemeSource::User);

        fs::remove_dir_all(builtin).unwrap();
        fs::remove_dir_all(user).unwrap();
    }

    #[test]
    fn inherits_colours_from_a_parent_theme() {
        let root = scratch("inherit");

        write_theme(&root, "base", BASE_THEME);
        write_theme(
            &root,
            "child",
            r##"
[theme]
name = "Child"
id = "child"
version = "1.0.0"
description = "Child theme"
extends = "base"

[colors]
primary = "#123456"
"##,
        );

        let theme = Registry::new(vec![root.clone()])
            .load("child")
            .expect("child theme should load");

        assert_eq!(theme.colors.primary.to_hex(), "#123456");
        assert_eq!(theme.colors.background.to_hex(), "#000000");
        assert_eq!(theme.colors.selected.to_hex(), "#222222");
        assert_eq!(theme.metadata.extends.as_deref(), Some("base"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_inheritance_cycles() {
        let root = scratch("cycle");

        write_theme(
            &root,
            "first",
            &BASE_THEME
                .replace("name = \"Base\"", "name = \"First\"")
                .replace("id = \"base\"", "id = \"first\"")
                .replace(
                    "description = \"Base theme\"",
                    "description = \"First\"\nextends = \"second\"",
                ),
        );
        write_theme(
            &root,
            "second",
            &BASE_THEME
                .replace("name = \"Base\"", "name = \"Second\"")
                .replace("id = \"base\"", "id = \"second\"")
                .replace(
                    "description = \"Base theme\"",
                    "description = \"Second\"\nextends = \"first\"",
                ),
        );

        let result = Registry::new(vec![root.clone()]).load("first");

        assert!(matches!(result, Err(ThemeLoadError::InheritanceCycle(_))));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_metadata() {
        let root = scratch("metadata");

        write_theme(&root, "bad-version", &BASE_THEME.replace("1.0.0", "1.0"));

        let result = Registry::new(vec![root.clone()]).load("bad-version");

        assert!(matches!(result, Err(ThemeLoadError::InvalidMetadata(_))));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unsupported_schema_versions() {
        let root = scratch("schema");

        write_theme(
            &root,
            "base",
            &BASE_THEME.replace("schema = 1", "schema = 99"),
        );

        let result = Registry::new(vec![root.clone()]).load("base");

        assert!(matches!(result, Err(ThemeLoadError::UnsupportedSchema(99))));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_missing_colours() {
        let root = scratch("missing-color");

        write_theme(
            &root,
            "base",
            &BASE_THEME.replace("primary = \"#FF0000\"", ""),
        );

        let result = Registry::new(vec![root.clone()]).load("base");

        assert!(matches!(result, Err(ThemeLoadError::MissingColor(_))));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_invalid_themes_in_discovery() {
        let root = scratch("broken");

        write_theme(&root, "base", BASE_THEME);
        write_theme(&root, "broken", "this is not toml");

        let discovery = Registry::new(vec![root.clone()]).list();

        assert_eq!(discovery.themes.len(), 1);
        assert_eq!(discovery.broken.len(), 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn id_must_match_directory() {
        let root = scratch("id-mismatch");

        write_theme(&root, "directory", BASE_THEME);

        let result = Registry::new(vec![root.clone()]).load("directory");

        assert!(matches!(result, Err(ThemeLoadError::InvalidMetadata(_))));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn loads_single_file_themes() {
        let root = scratch("file-theme");

        fs::write(root.join("base.toml"), BASE_THEME).unwrap();

        let theme = Registry::new(vec![root.clone()])
            .load("base")
            .expect("file theme should load");

        assert_eq!(theme.metadata.name, "Base");

        fs::remove_dir_all(root).unwrap();
    }
}
