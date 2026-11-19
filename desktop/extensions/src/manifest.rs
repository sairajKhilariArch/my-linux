//! Extension manifests: metadata, validation and output checks.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use theme_engine::{LoadedTheme, validate_id};

use crate::registry::ExtensionError;
use crate::template;

/// Schema version understood by this build.
pub const SCHEMA_VERSION: u32 = 1;

/// File name of the manifest inside an extension directory.
pub const MANIFEST_FILE: &str = "extension.toml";

/// Directories inside the installation root that extensions must never write to.
const RESERVED_DIRECTORIES: [&str; 2] = ["state", "backups"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExtensionFormat {
    Toml,
    Css,
    Rasi,
    Ini,
    #[default]
    Text,
}

impl ExtensionFormat {
    /// Parses the `format` value of a manifest.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "toml" => Some(Self::Toml),
            "css" => Some(Self::Css),
            "rasi" => Some(Self::Rasi),
            "ini" => Some(Self::Ini),
            "text" | "txt" | "conf" | "plain" => Some(Self::Text),
            _ => None,
        }
    }

    /// The value written back into a manifest.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Toml => "toml",
            Self::Css => "css",
            Self::Rasi => "rasi",
            Self::Ini => "ini",
            Self::Text => "text",
        }
    }

    /// Conventional template file extension for a new extension.
    pub const fn file_extension(self) -> &'static str {
        match self {
            Self::Toml => "toml",
            Self::Css => "css",
            Self::Rasi => "rasi",
            Self::Ini => "ini",
            Self::Text => "txt",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionMetadata {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub format: ExtensionFormat,
    /// Destination of the generated file, relative to the installation root.
    pub destination: String,
    /// Programs that indicate the target application is installed.
    pub programs: Vec<String>,
    /// Template file name inside the extension directory.
    pub template: String,
    /// Commands that reload the target application.
    pub reload: Vec<Vec<String>>,
    /// Capabilities the extension declares (currently `reload`).
    pub permissions: Vec<String>,
}

/// A loaded extension: its manifest and template.
#[derive(Debug, Clone)]
pub struct Extension {
    pub metadata: ExtensionMetadata,
    pub directory: PathBuf,
    pub template: String,
}

#[derive(Debug, Deserialize)]
struct RawExtensionFile {
    extension: RawExtension,
}

#[derive(Debug, Deserialize)]
struct RawExtension {
    name: String,
    #[serde(default)]
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
    format: Option<String>,
    destination: String,
    #[serde(default)]
    programs: Vec<String>,
    #[serde(default)]
    template: Option<String>,
    #[serde(default)]
    reload: Vec<Vec<String>>,
    #[serde(default)]
    permissions: Vec<String>,
}

impl Extension {
    /// Loads an extension from a directory containing `extension.toml`.
    pub fn from_directory(directory: impl AsRef<Path>) -> Result<Self, ExtensionError> {
        let directory = directory.as_ref();
        let manifest = directory.join(MANIFEST_FILE);
        let contents = fs::read_to_string(&manifest)
            .map_err(|error| ExtensionError::Invalid(format!("{}: {error}", manifest.display())))?;

        Self::parse(&contents, directory)
    }

    /// Parses manifest `contents` as if it lived in `directory`.
    pub fn parse(contents: &str, directory: impl AsRef<Path>) -> Result<Self, ExtensionError> {
        let directory = directory.as_ref();
        let raw: RawExtensionFile = toml::from_str(contents)
            .map_err(|error| ExtensionError::Invalid(format!("Invalid extension TOML: {error}")))?;

        let expected_id = directory
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_lowercase());
        let metadata = validate_metadata(raw.extension, expected_id.as_deref())?;
        let template_path = directory.join(&metadata.template);
        let template = fs::read_to_string(&template_path).map_err(|error| {
            ExtensionError::Invalid(format!(
                "Cannot read the template {}: {error}",
                template_path.display()
            ))
        })?;

        Ok(Self {
            metadata,
            directory: directory.to_path_buf(),
            template,
        })
    }

    pub fn id(&self) -> &str {
        &self.metadata.id
    }

    pub fn destination(&self) -> &str {
        &self.metadata.destination
    }

    /// Renders the template against `theme`.
    pub fn render(&self, theme: &LoadedTheme) -> Result<String, ExtensionError> {
        let content = template::render(&self.template, theme)?;

        self.validate_output(&content)?;

        Ok(content)
    }

    /// Rejects output the target application could not parse.
    pub fn validate_output(&self, content: &str) -> Result<(), ExtensionError> {
        if content.trim().is_empty() {
            return Err(ExtensionError::Invalid(format!(
                "Extension `{}` produced an empty configuration",
                self.metadata.id
            )));
        }

        match self.metadata.format {
            ExtensionFormat::Toml => {
                toml::from_str::<toml::Value>(content)
                    .map(|_| ())
                    .map_err(|error| {
                        ExtensionError::Invalid(format!(
                            "Extension `{}` produced invalid TOML: {error}",
                            self.metadata.id
                        ))
                    })
            }
            ExtensionFormat::Css | ExtensionFormat::Rasi => {
                validate_braces(&self.metadata.id, content)
            }
            ExtensionFormat::Ini | ExtensionFormat::Text => Ok(()),
        }
    }

    /// Serialises the manifest back to TOML (without the template body).
    pub fn to_toml(&self) -> String {
        let metadata = &self.metadata;
        let mut output = String::from(
            "# My Linux extension. Edit the template, then run `my-linux-theme extensions list`.\n\n[extension]\n",
        );

        output.push_str(&format!("schema = {}\n", metadata.schema));
        output.push_str(&format!("id = \"{}\"\n", metadata.id));
        output.push_str(&format!("name = \"{}\"\n", escape(&metadata.name)));
        output.push_str(&format!("version = \"{}\"\n", metadata.version));
        output.push_str(&format!(
            "description = \"{}\"\n",
            escape(&metadata.description)
        ));

        if let Some(author) = &metadata.author {
            output.push_str(&format!("author = \"{}\"\n", escape(author)));
        }

        output.push_str(&format!("format = \"{}\"\n", metadata.format.label()));
        output.push_str(&format!("destination = \"{}\"\n", metadata.destination));
        output.push_str(&format!("template = \"{}\"\n", metadata.template));

        if !metadata.programs.is_empty() {
            output.push_str(&format!("programs = {}\n", array(&metadata.programs)));
        }

        if !metadata.reload.is_empty() {
            let commands: Vec<String> = metadata
                .reload
                .iter()
                .map(|command| array(command))
                .collect();

            output.push_str(&format!("reload = [{}]\n", commands.join(", ")));
        }

        if !metadata.permissions.is_empty() {
            output.push_str(&format!("permissions = {}\n", array(&metadata.permissions)));
        }

        output
    }
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn array(values: &[String]) -> String {
    let quoted: Vec<String> = values
        .iter()
        .map(|value| format!("\"{}\"", escape(value)))
        .collect();

    format!("[{}]", quoted.join(", "))
}

/// Rejects unbalanced braces in CSS-style output.
fn validate_braces(id: &str, content: &str) -> Result<(), ExtensionError> {
    let mut depth: i32 = 0;

    for character in content.chars() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;

                if depth < 0 {
                    return Err(ExtensionError::Invalid(format!(
                        "Extension `{id}` produced an unbalanced closing brace"
                    )));
                }
            }
            _ => {}
        }
    }

    if depth != 0 {
        return Err(ExtensionError::Invalid(format!(
            "Extension `{id}` produced an unbalanced opening brace"
        )));
    }

    Ok(())
}

fn validate_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();

    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

/// A destination must stay inside the installation root and outside the state
/// directories My Linux owns.
fn validate_destination(destination: &str) -> Result<(), ExtensionError> {
    let invalid = |reason: &str| {
        ExtensionError::Invalid(format!("Invalid destination `{destination}`: {reason}"))
    };

    if destination.trim().is_empty() {
        return Err(invalid("it is empty"));
    }

    if destination.starts_with('/') {
        return Err(invalid("it must be relative to the installation root"));
    }

    if destination.ends_with('/') {
        return Err(invalid("it must name a file"));
    }

    let components: Vec<&str> = destination.split('/').collect();

    if components
        .iter()
        .any(|component| component.is_empty() || *component == "." || *component == "..")
    {
        return Err(invalid("it must not contain empty, `.` or `..` components"));
    }

    if components.len() > 1 && RESERVED_DIRECTORIES.contains(&components[0]) {
        return Err(invalid("that directory belongs to My Linux"));
    }

    Ok(())
}

/// A template file name must stay inside the extension directory.
fn validate_template(template: &str) -> Result<(), ExtensionError> {
    let invalid =
        |reason: &str| ExtensionError::Invalid(format!("Invalid template `{template}`: {reason}"));

    if template.trim().is_empty() {
        return Err(invalid("it is empty"));
    }

    if template.contains('/') || template.contains('\\') {
        return Err(invalid("it must be a file name inside the extension"));
    }

    if template == "." || template == ".." {
        return Err(invalid("it must be a file name inside the extension"));
    }

    Ok(())
}

fn validate_metadata(
    raw: RawExtension,
    expected_id: Option<&str>,
) -> Result<ExtensionMetadata, ExtensionError> {
    let schema = raw.schema.unwrap_or(SCHEMA_VERSION);

    if schema != SCHEMA_VERSION {
        return Err(ExtensionError::Invalid(format!(
            "Unsupported extension schema version {schema}; this build supports {SCHEMA_VERSION}"
        )));
    }

    let id = if raw.id.trim().is_empty() {
        expected_id.unwrap_or_default().to_string()
    } else {
        raw.id.trim().to_lowercase()
    };

    validate_id(&id).map_err(|error| ExtensionError::Invalid(error.to_string()))?;

    if let Some(expected) = expected_id
        && expected != id
    {
        return Err(ExtensionError::Invalid(format!(
            "extension id `{id}` does not match its directory `{expected}`"
        )));
    }

    if raw.name.trim().is_empty() {
        return Err(ExtensionError::Invalid(
            "extension name must not be empty".to_string(),
        ));
    }

    if raw.description.trim().is_empty() {
        return Err(ExtensionError::Invalid(
            "extension description must not be empty".to_string(),
        ));
    }

    let version = if raw.version.trim().is_empty() {
        "0.1.0".to_string()
    } else {
        raw.version.trim().to_string()
    };

    if !validate_version(&version) {
        return Err(ExtensionError::Invalid(format!(
            "invalid version `{version}`: expected MAJOR.MINOR.PATCH"
        )));
    }

    let format = match raw.format.as_deref() {
        Some(value) => ExtensionFormat::parse(value).ok_or_else(|| {
            ExtensionError::Invalid(format!(
                "unknown format `{value}`: use toml, css, rasi, ini or text"
            ))
        })?,
        None => ExtensionFormat::default(),
    };

    validate_destination(&raw.destination)?;

    let template = raw
        .template
        .unwrap_or_else(|| format!("template.{}", format.file_extension()));

    validate_template(&template)?;

    let mut programs = Vec::new();

    for program in raw.programs {
        let program = program.trim().to_string();

        if program.is_empty() {
            return Err(ExtensionError::Invalid(
                "programs must not contain an empty entry".to_string(),
            ));
        }

        programs.push(program);
    }

    for command in &raw.reload {
        if command.is_empty() || command[0].trim().is_empty() {
            return Err(ExtensionError::Invalid(
                "reload commands must start with a program".to_string(),
            ));
        }
    }

    let mut permissions: Vec<String> = Vec::new();

    for permission in raw.permissions {
        let permission = permission.trim().to_lowercase();

        if permission != "reload" {
            return Err(ExtensionError::Invalid(format!(
                "unknown permission `{permission}`; supported permissions: reload"
            )));
        }

        if !permissions.contains(&permission) {
            permissions.push(permission);
        }
    }

    if !raw.reload.is_empty() && !permissions.iter().any(|permission| permission == "reload") {
        return Err(ExtensionError::Invalid(
            "reload commands require the `reload` permission in the manifest".to_string(),
        ));
    }

    Ok(ExtensionMetadata {
        schema,
        id,
        name: raw.name.trim().to_string(),
        version,
        description: raw.description.trim().to_string(),
        author: raw.author,
        format,
        destination: raw.destination.trim().to_string(),
        programs,
        template,
        reload: raw.reload,
        permissions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
[extension]
schema = 1
id = "demo"
name = "Demo"
version = "1.0.0"
description = "A demo extension"
format = "css"
destination = "demo/style.css"
programs = ["demo-app"]
template = "template.css"
"#;

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "my-linux-extensions-{}-{}",
            name,
            std::process::id()
        ));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        directory
    }

    #[test]
    fn reload_commands_require_the_reload_permission() {
        let root = scratch("permissions");
        let directory = root.join("demo");

        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("template.css"),
            ":root { color: {{colors.primary}}; }\n",
        )
        .unwrap();

        let without = format!("{VALID}\nreload = [[\"demo-app\", \"--reload\"]]\n");
        fs::write(directory.join("extension.toml"), &without).unwrap();

        let error = Extension::from_directory(directory.clone()).expect_err("permission required");
        assert!(error.to_string().contains("reload` permission"));

        fs::write(
            directory.join("extension.toml"),
            format!(
                "{VALID}\npermissions = [\"reload\"]\nreload = [[\"demo-app\", \"--reload\"]]\n"
            ),
        )
        .unwrap();

        let extension = Extension::from_directory(directory.clone()).unwrap();

        assert_eq!(extension.metadata.permissions, vec!["reload"]);
        assert!(extension.to_toml().contains("permissions = [\"reload\"]"));

        fs::write(
            directory.join("extension.toml"),
            format!("{VALID}\npermissions = [\"filesystem\"]\n"),
        )
        .unwrap();

        assert!(
            Extension::from_directory(directory)
                .expect_err("unknown permissions are rejected")
                .to_string()
                .contains("unknown permission")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parses_a_valid_manifest() {
        let root = scratch("parse");

        fs::create_dir_all(root.join("demo")).unwrap();
        fs::write(root.join("demo/extension.toml"), VALID).unwrap();
        fs::write(
            root.join("demo/template.css"),
            ":root { color: {{colors.primary}}; }\n",
        )
        .unwrap();

        let extension = Extension::from_directory(root.join("demo")).unwrap();

        assert_eq!(extension.metadata.id, "demo");
        assert_eq!(extension.metadata.format, ExtensionFormat::Css);
        assert_eq!(extension.metadata.template, "template.css");
        assert_eq!(extension.metadata.programs, vec!["demo-app"]);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_manifests() {
        let root = scratch("invalid");

        let cases = [
            ("mismatch", VALID.to_string(), "does not match"),
            (
                "demo",
                VALID.replace(
                    "destination = \"demo/style.css\"",
                    "destination = \"../escape.css\"",
                ),
                "Invalid destination",
            ),
            (
                "demo",
                VALID.replace(
                    "destination = \"demo/style.css\"",
                    "destination = \"state/active-theme\"",
                ),
                "belongs to My Linux",
            ),
            (
                "demo",
                VALID.replace("format = \"css\"", "format = \"binary\""),
                "unknown format",
            ),
            (
                "demo",
                VALID.replace("schema = 1", "schema = 99"),
                "Unsupported extension schema",
            ),
            (
                "demo",
                VALID.replace("description = \"A demo extension\"", "description = \"\""),
                "description",
            ),
            (
                "demo",
                VALID.replace(
                    "template = \"template.css\"",
                    "template = \"../outside.css\"",
                ),
                "Invalid template",
            ),
        ];

        for (name, contents, needle) in cases {
            let directory = root.join(name);

            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("extension.toml"), &contents).unwrap();
            fs::write(directory.join("template.css"), "body {}\n").unwrap();

            let error = Extension::from_directory(&directory).unwrap_err();

            assert!(
                error.to_string().contains(needle),
                "{contents}\nshould fail with `{needle}`, got `{error}`"
            );
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_output_for_the_declared_format() {
        let root = scratch("output");

        fs::create_dir_all(root.join("demo")).unwrap();
        fs::write(root.join("demo/extension.toml"), VALID).unwrap();
        fs::write(root.join("demo/template.css"), "body { color: red;\n").unwrap();

        let extension = Extension::from_directory(root.join("demo")).unwrap();

        assert!(extension.validate_output("body { }").is_ok());
        assert!(extension.validate_output("body { ").is_err());
        assert!(extension.validate_output("").is_err());

        let toml_extension = Extension {
            metadata: ExtensionMetadata {
                format: ExtensionFormat::Toml,
                ..extension.metadata.clone()
            },
            ..extension
        };

        assert!(toml_extension.validate_output("key = 1\n").is_ok());
        assert!(toml_extension.validate_output("key =\n").is_err());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn serialises_a_manifest_that_parses_again() {
        let root = scratch("serialize");

        fs::create_dir_all(root.join("demo")).unwrap();
        fs::write(root.join("demo/extension.toml"), VALID).unwrap();
        fs::write(root.join("demo/template.css"), "body { }\n").unwrap();

        let mut extension = Extension::from_directory(root.join("demo")).unwrap();

        extension.metadata.programs = vec!["demo-app".to_string()];
        extension.metadata.reload = vec![vec!["demo-app".to_string(), "--reload".to_string()]];
        extension.metadata.permissions = vec!["reload".to_string()];

        let rendered = extension.to_toml();

        assert!(rendered.contains("reload = [[\"demo-app\", \"--reload\"]]"));
        assert!(rendered.contains("permissions = [\"reload\"]"));

        let reparsed = Extension::parse(&rendered, root.join("demo")).unwrap();

        assert_eq!(reparsed.metadata, extension.metadata);

        fs::remove_dir_all(root).unwrap();
    }
}
