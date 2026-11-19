//! Repository index parsing and validation.
//!
//! ```toml
//! [repository]
//! schema = 1
//! name = "My Linux Community"
//!
//! [[package]]
//! id = "aurora"
//! kind = "theme"          # theme | extension
//! name = "Aurora"
//! version = "1.0.0"
//! description = "A cool blue theme"
//! source = "themes/aurora" # relative to the index, or absolute
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::RepositoryError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    Theme,
    Extension,
}

impl PackageKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Extension => "extension",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "theme" => Some(Self::Theme),
            "extension" => Some(Self::Extension),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub id: String,
    pub kind: PackageKind,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    /// Absolute path of the package content, resolved against the index.
    pub source: PathBuf,
    /// The source exactly as written in the index.
    pub raw_source: String,
}

#[derive(Debug, Clone)]
pub struct RepositoryIndex {
    pub name: String,
    pub description: String,
    pub schema: u32,
    pub path: PathBuf,
    pub packages: Vec<Package>,
}

impl RepositoryIndex {
    pub fn find(&self, id: &str) -> Option<&Package> {
        let id = id.trim().to_lowercase();

        self.packages.iter().find(|package| package.id == id)
    }

    /// Packages whose id, name or description contains `query`.
    pub fn search(&self, query: &str) -> Vec<&Package> {
        let query = query.trim().to_lowercase();

        if query.is_empty() {
            return self.packages.iter().collect();
        }

        self.packages
            .iter()
            .filter(|package| {
                package.id.contains(&query)
                    || package.name.to_lowercase().contains(&query)
                    || package.description.to_lowercase().contains(&query)
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct RawIndex {
    repository: RawRepository,
    #[serde(default)]
    package: Vec<RawPackage>,
}

#[derive(Debug, Deserialize)]
struct RawRepository {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    schema: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawPackage {
    id: String,
    kind: String,
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: Option<String>,
    source: String,
}

/// Loads and validates an index file.
pub fn load(path: impl AsRef<Path>) -> Result<RepositoryIndex, RepositoryError> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path)?;
    let directory = path.parent().unwrap_or(Path::new("."));

    parse(&contents, directory, path)
}

/// Parses index `contents`; package sources resolve against `directory`.
pub fn parse(
    contents: &str,
    directory: &Path,
    path: impl Into<PathBuf>,
) -> Result<RepositoryIndex, RepositoryError> {
    let raw: RawIndex = toml::from_str(contents)
        .map_err(|error| RepositoryError::Invalid(format!("Invalid repository index: {error}")))?;

    let schema = raw.repository.schema.unwrap_or(crate::SCHEMA_VERSION);

    if schema != crate::SCHEMA_VERSION {
        return Err(RepositoryError::Invalid(format!(
            "Unsupported repository schema {schema}; this build supports {}",
            crate::SCHEMA_VERSION
        )));
    }

    if raw.repository.name.trim().is_empty() {
        return Err(RepositoryError::Invalid(
            "repository.name must not be empty".to_string(),
        ));
    }

    let mut packages: Vec<Package> = Vec::new();

    for raw in raw.package {
        let id = raw.id.trim().to_lowercase();

        theme_engine::validate_id(&id)
            .map_err(|error| RepositoryError::Invalid(error.to_string()))?;

        if packages.iter().any(|package| package.id == id) {
            return Err(RepositoryError::Invalid(format!(
                "duplicate package id `{id}`"
            )));
        }

        if raw.name.trim().is_empty() {
            return Err(RepositoryError::Invalid(format!(
                "package `{id}` has an empty name"
            )));
        }

        if raw.description.trim().is_empty() {
            return Err(RepositoryError::Invalid(format!(
                "package `{id}` has an empty description"
            )));
        }

        let kind = PackageKind::parse(&raw.kind).ok_or_else(|| {
            RepositoryError::Invalid(format!(
                "package `{id}` has unknown kind `{}`: use theme or extension",
                raw.kind
            ))
        })?;
        let source_text = raw.source.trim();

        if source_text.is_empty() {
            return Err(RepositoryError::Invalid(format!(
                "package `{id}` has an empty source"
            )));
        }

        let relative = source_text.strip_prefix("file://").unwrap_or(source_text);
        let source = {
            let candidate = PathBuf::from(relative);

            if candidate.is_absolute() {
                candidate
            } else {
                directory.join(candidate)
            }
        };

        packages.push(Package {
            id,
            kind,
            name: raw.name.trim().to_string(),
            version: if raw.version.trim().is_empty() {
                "0.1.0".to_string()
            } else {
                raw.version.trim().to_string()
            },
            description: raw.description.trim().to_string(),
            author: raw.author,
            source,
            raw_source: source_text.to_string(),
        });
    }

    Ok(RepositoryIndex {
        name: raw.repository.name.trim().to_string(),
        description: raw.repository.description.trim().to_string(),
        schema,
        path: path.into(),
        packages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = r#"
[repository]
schema = 1
name = "Community"
description = "Community packages"

[[package]]
id = "aurora"
kind = "theme"
name = "Aurora"
version = "1.0.0"
description = "A cool theme"
author = "Someone"
source = "themes/aurora"

[[package]]
id = "widget"
kind = "extension"
name = "Widget"
version = "2.0.0"
description = "A widget extension"
source = "/absolute/widget"
"#;

    #[test]
    fn parses_and_resolves_packages() {
        let index = parse(INDEX, Path::new("/repo"), "/repo/index.toml").unwrap();

        assert_eq!(index.schema, 1);
        assert_eq!(index.packages.len(), 2);

        let aurora = index.find("AURORA").unwrap();

        assert_eq!(aurora.kind, PackageKind::Theme);
        assert_eq!(aurora.source, PathBuf::from("/repo/themes/aurora"));
        assert_eq!(aurora.author.as_deref(), Some("Someone"));

        assert_eq!(
            index.find("widget").unwrap().source,
            PathBuf::from("/absolute/widget")
        );
        assert_eq!(index.search("cool").len(), 1);
        assert_eq!(index.search("").len(), 2);
    }

    #[test]
    fn rejects_invalid_indexes() {
        let cases = [
            (INDEX.replace("schema = 1", "schema = 9"), "Unsupported"),
            (
                INDEX.replace("kind = \"theme\"", "kind = \"plugin\""),
                "unknown kind",
            ),
            (
                INDEX.replace("description = \"A widget extension\"", "description = \"\""),
                "empty description",
            ),
            (
                INDEX.replace("id = \"widget\"", "id = \"aurora\""),
                "duplicate package id",
            ),
            (
                INDEX.replace("id = \"widget\"", "id = \"Bad Id\""),
                "invalid theme id",
            ),
        ];

        for (contents, needle) in cases {
            let error = parse(&contents, Path::new("/repo"), "/repo/index.toml").unwrap_err();

            assert!(
                error.to_string().contains(needle),
                "expected `{needle}`, got `{error}`"
            );
        }
    }

    #[test]
    fn keeps_file_urls_as_paths() {
        let index = parse(
            &INDEX.replace(
                "source = \"themes/aurora\"",
                "source = \"file:///tmp/aurora\"",
            ),
            Path::new("/repo"),
            "/repo/index.toml",
        )
        .unwrap();

        assert_eq!(
            index.find("aurora").unwrap().source,
            PathBuf::from("/tmp/aurora")
        );
    }
}
