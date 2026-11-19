//! Extension discovery, installation, removal and enabled state.
//!
//! Extensions are read from one or more roots. A user extension shadows a
//! built-in extension with the same id. Enabled extensions are recorded in
//! `extensions.toml` next to the user root, so a single theme apply can also
//! update every third-party application the user has opted into.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use config_manager::install_content;
use serde::{Deserialize, Serialize};
use theme_engine::LoadedTheme;

use crate::manifest::{Extension, ExtensionFormat, MANIFEST_FILE};
use crate::template::render;

#[derive(Debug)]
pub enum ExtensionError {
    Io(io::Error),
    Invalid(String),
    Template(String),
    Unknown(String),
    Builtin(String),
}

impl std::fmt::Display for ExtensionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Invalid(message) => write!(formatter, "{message}"),
            Self::Template(message) => write!(formatter, "{message}"),
            Self::Unknown(id) => write!(formatter, "Unknown extension: {id}"),
            Self::Builtin(id) => write!(
                formatter,
                "`{id}` is a built-in extension and cannot be removed"
            ),
        }
    }
}

impl std::error::Error for ExtensionError {}

impl From<io::Error> for ExtensionError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<config_manager::ConfigError> for ExtensionError {
    fn from(error: config_manager::ConfigError) -> Self {
        Self::Invalid(error.to_string())
    }
}

/// Where an extension was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionSource {
    User,
    Builtin,
}

impl ExtensionSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Builtin => "builtin",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub destination: String,
    pub format: &'static str,
    pub programs: Vec<String>,
    pub source: ExtensionSource,
    pub enabled: bool,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenExtension {
    pub path: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtensionDiscovery {
    pub extensions: Vec<ExtensionSummary>,
    pub broken: Vec<BrokenExtension>,
}

/// The file produced by rendering one extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedExtension {
    pub id: String,
    /// Destination relative to the installation root.
    pub destination: String,
    pub content: String,
}

/// The enabled extensions recorded for the user.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub enabled: Vec<String>,
}

/// Directory holding user extensions.
///
/// `$MY_LINUX_USER_EXTENSIONS`, otherwise `$XDG_CONFIG_HOME/my-linux/extensions`.
pub fn user_extensions_dir() -> PathBuf {
    if let Some(directory) = std::env::var_os("MY_LINUX_USER_EXTENSIONS") {
        return PathBuf::from(directory);
    }

    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));

    config_home.join("my-linux").join("extensions")
}

#[derive(Debug, Clone)]
pub struct ExtensionRegistry {
    builtin: PathBuf,
    user: PathBuf,
}

impl ExtensionRegistry {
    pub fn new(builtin: impl Into<PathBuf>, user: impl Into<PathBuf>) -> Self {
        Self {
            builtin: builtin.into(),
            user: user.into(),
        }
    }

    pub fn user_root(&self) -> &Path {
        &self.user
    }

    pub fn builtin_root(&self) -> &Path {
        &self.builtin
    }

    /// Enabled state lives beside the user extensions, not inside a theme.
    pub fn settings_file(&self) -> PathBuf {
        let parent = self.user.parent().unwrap_or(Path::new("."));

        parent.join("extensions.toml")
    }

    /// Reads the enabled extensions; a missing file means none are enabled.
    pub fn settings(&self) -> Result<Settings, ExtensionError> {
        match fs::read_to_string(self.settings_file()) {
            Ok(contents) => toml::from_str(&contents).map_err(|error| {
                ExtensionError::Invalid(format!(
                    "Invalid {}: {error}",
                    self.settings_file().display()
                ))
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(error) => Err(error.into()),
        }
    }

    fn write_settings(&self, settings: &Settings) -> Result<PathBuf, ExtensionError> {
        let mut ids = settings.enabled.clone();

        ids.sort();
        ids.dedup();

        let mut content =
            String::from("# Extensions applied together with every theme. Managed by My Linux.\n");

        content.push_str("enabled = ");

        let quoted: Vec<String> = ids.iter().map(|id| format!("\"{id}\"")).collect();

        content.push_str(&format!("[{}]\n", quoted.join(", ")));

        let path = self.settings_file();

        install_content(content.as_bytes(), &path)?;

        Ok(path)
    }

    /// Enabled extension ids, in the recorded order.
    pub fn enabled(&self) -> Result<Vec<String>, ExtensionError> {
        Ok(self.settings()?.enabled)
    }

    /// Enables an extension; the extension must resolve first.
    pub fn enable(&self, id: &str) -> Result<PathBuf, ExtensionError> {
        self.load(id)?;

        let mut settings = self.settings()?;

        if !settings
            .enabled
            .iter()
            .any(|enabled| enabled == &id.to_lowercase())
        {
            settings.enabled.push(id.to_lowercase());
        }

        self.write_settings(&settings)
    }

    /// Disables an extension. Disabling an extension that is not enabled is fine.
    pub fn disable(&self, id: &str) -> Result<PathBuf, ExtensionError> {
        let id = id.trim().to_lowercase();
        let mut settings = self.settings()?;

        settings.enabled.retain(|enabled| enabled != &id);

        self.write_settings(&settings)
    }

    /// Directory of an extension; user extensions win.
    pub fn find(&self, id: &str) -> Option<PathBuf> {
        let id = id.trim().to_lowercase();

        if id.is_empty() {
            return None;
        }

        for root in [&self.user, &self.builtin] {
            let directory = root.join(&id);

            if directory.join(MANIFEST_FILE).is_file() {
                return Some(directory);
            }
        }

        None
    }

    /// Loads an extension by id.
    pub fn load(&self, id: &str) -> Result<Extension, ExtensionError> {
        let directory = self
            .find(id)
            .ok_or_else(|| ExtensionError::Unknown(id.trim().to_string()))?;

        Extension::from_directory(directory)
    }

    /// Every extension, user extensions first and shadowing built-ins.
    pub fn list(&self) -> ExtensionDiscovery {
        let enabled = self.enabled().unwrap_or_default();
        let mut discovery = ExtensionDiscovery::default();

        for (root, source) in [
            (&self.user, ExtensionSource::User),
            (&self.builtin, ExtensionSource::Builtin),
        ] {
            let Ok(entries) = fs::read_dir(root) else {
                continue;
            };

            let mut paths: Vec<PathBuf> = entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect();

            paths.sort();

            for path in paths {
                let Some(id) = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.to_lowercase())
                else {
                    continue;
                };

                if !path.join(MANIFEST_FILE).is_file()
                    || discovery.extensions.iter().any(|summary| summary.id == id)
                {
                    continue;
                }

                match Extension::from_directory(&path) {
                    Ok(extension) => discovery.extensions.push(ExtensionSummary {
                        id: extension.metadata.id.clone(),
                        name: extension.metadata.name.clone(),
                        version: extension.metadata.version.clone(),
                        description: extension.metadata.description.clone(),
                        destination: extension.metadata.destination.clone(),
                        format: extension.metadata.format.label(),
                        programs: extension.metadata.programs.clone(),
                        source,
                        enabled: enabled.contains(&extension.metadata.id),
                        path,
                    }),
                    Err(error) => discovery.broken.push(BrokenExtension {
                        path,
                        error: error.to_string(),
                    }),
                }
            }
        }

        discovery
            .extensions
            .sort_by(|left, right| left.id.cmp(&right.id));

        discovery
    }

    /// Renders one extension against `theme` and validates the output.
    pub fn render(
        &self,
        id: &str,
        theme: &LoadedTheme,
    ) -> Result<GeneratedExtension, ExtensionError> {
        let extension = self.load(id)?;
        let content = extension.render(theme)?;

        Ok(GeneratedExtension {
            id: extension.metadata.id.clone(),
            destination: extension.metadata.destination.clone(),
            content,
        })
    }

    /// Renders every enabled extension for a theme apply.
    ///
    /// A destination can only be produced by one extension; a collision is an
    /// error so a theme apply never silently overwrites another extension.
    pub fn render_enabled(
        &self,
        theme: &LoadedTheme,
    ) -> Result<Vec<GeneratedExtension>, ExtensionError> {
        let mut generated: Vec<GeneratedExtension> = Vec::new();

        for id in self.enabled()? {
            let extension = self.render(&id, theme)?;

            if let Some(existing) = generated
                .iter()
                .find(|existing| existing.destination == extension.destination)
            {
                return Err(ExtensionError::Invalid(format!(
                    "Extensions `{}` and `{}` both write {}",
                    existing.id, extension.id, extension.destination
                )));
            }

            generated.push(extension);
        }

        Ok(generated)
    }

    /// Copies an extension directory into the user root.
    ///
    /// `as_id` renames the extension, which allows importing a newer version of
    /// an extension that is already installed.
    pub fn import(
        &self,
        source: impl AsRef<Path>,
        as_id: Option<&str>,
    ) -> Result<PathBuf, ExtensionError> {
        let source = source.as_ref();
        let mut extension = Extension::from_directory(source)?;

        if let Some(id) = as_id {
            let id = id.trim().to_lowercase();

            theme_engine::validate_id(&id)
                .map_err(|error| ExtensionError::Invalid(error.to_string()))?;

            extension.metadata.id = id;
        }

        let target = self.user.join(&extension.metadata.id);

        if target.exists() {
            return Err(ExtensionError::Invalid(format!(
                "Extension `{}` is already installed; remove it first or import it as another id",
                extension.metadata.id
            )));
        }

        copy_dir(source, &target)?;

        if as_id.is_some() {
            install_content(extension.to_toml().as_bytes(), target.join(MANIFEST_FILE))?;
        }

        Ok(target)
    }

    /// Copies an extension into `destination_parent/<id>`.
    pub fn export(
        &self,
        id: &str,
        destination_parent: impl AsRef<Path>,
    ) -> Result<PathBuf, ExtensionError> {
        let extension = self.load(id)?;
        let target = destination_parent.as_ref().join(&extension.metadata.id);

        if target.exists() {
            return Err(ExtensionError::Invalid(format!(
                "{} already exists; choose another destination",
                target.display()
            )));
        }

        copy_dir(&extension.directory, &target)?;

        Ok(target)
    }

    /// Removes a user extension and disables it.
    pub fn remove(&self, id: &str) -> Result<PathBuf, ExtensionError> {
        let id = id.trim().to_lowercase();
        let directory = self.user.join(&id);

        if directory.join(MANIFEST_FILE).is_file() {
            fs::remove_dir_all(&directory)?;
            self.disable(&id)?;

            return Ok(directory);
        }

        if self.builtin.join(&id).join(MANIFEST_FILE).is_file() {
            return Err(ExtensionError::Builtin(id));
        }

        Err(ExtensionError::Unknown(id))
    }

    /// Creates a starter extension in the user root.
    pub fn scaffold(
        &self,
        id: &str,
        name: &str,
        destination: &str,
        format: ExtensionFormat,
    ) -> Result<PathBuf, ExtensionError> {
        let id = id.trim().to_lowercase();

        theme_engine::validate_id(&id)
            .map_err(|error| ExtensionError::Invalid(error.to_string()))?;

        let directory = self.user.join(&id);

        if directory.exists() {
            return Err(ExtensionError::Invalid(format!(
                "Extension `{id}` already exists in {}",
                self.user.display()
            )));
        }

        let template_name = format!("template.{}", format.file_extension());
        let template = starter_template(format, name, destination);
        let metadata = crate::manifest::ExtensionMetadata {
            schema: crate::manifest::SCHEMA_VERSION,
            id: id.clone(),
            name: name.to_string(),
            version: "0.1.0".to_string(),
            description: format!("{name} extension for My Linux"),
            author: None,
            format,
            destination: destination.to_string(),
            programs: Vec::new(),
            template: template_name.clone(),
            reload: Vec::new(),
        };
        let extension = Extension {
            metadata,
            directory: directory.clone(),
            template: template.clone(),
        };

        // Serialise once so validation runs before anything is written.
        extension.validate_output(&render(&template, &sample_theme())?)?;

        fs::create_dir_all(&directory)?;
        fs::write(directory.join(MANIFEST_FILE), extension.to_toml())?;
        fs::write(directory.join(template_name), template)?;

        Ok(directory)
    }
}

fn starter_template(format: ExtensionFormat, name: &str, destination: &str) -> String {
    match format {
        ExtensionFormat::Toml => format!(
            "# {name} configuration generated by My Linux.\n# Installed at {destination}.\n\
             background = \"{{{{colors.background}}}}\"\n\
             foreground = \"{{{{colors.text}}}}\"\n\
             accent = \"{{{{colors.primary}}}}\"\n\
             border = \"{{{{colors.border}}}}\"\n"
        ),
        ExtensionFormat::Css | ExtensionFormat::Rasi => format!(
            "/* {name} styling generated by My Linux. Installed at {destination}. */\n\
             :root {{\n\
             \x20   background: {{{{colors.background}}}};\n\
             \x20   foreground: {{{{colors.text}}}};\n\
             \x20   accent: {{{{colors.primary}}}};\n\
             \x20   border: {{{{colors.border}}}};\n\
             }}\n"
        ),
        ExtensionFormat::Ini | ExtensionFormat::Text => format!(
            "# {name} configuration generated by My Linux.\n\
             # Installed at {destination}.\n\
             background = {{{{colors.background}}}}\n\
             foreground = {{{{colors.text}}}}\n\
             accent = {{{{colors.primary}}}}\n"
        ),
    }
}

/// A theme used only to check a scaffolded template before writing it.
fn sample_theme() -> LoadedTheme {
    use theme_engine::{Color, ThemeColors, ThemeMetadata, ThemeMetrics, ThemeTypography};

    let color = Color::new(0, 0, 0);

    LoadedTheme {
        metadata: ThemeMetadata {
            name: "Sample".to_string(),
            id: "sample".to_string(),
            version: "1.0.0".to_string(),
            description: "Sample theme".to_string(),
            author: None,
            schema: 1,
            extends: None,
        },
        colors: ThemeColors {
            background: color,
            surface: color,
            surface_alt: color,
            primary: color,
            secondary: color,
            text: color,
            text_muted: color,
            border: color,
            success: color,
            warning: color,
            error: color,
            active: color,
            hover: color,
            selected: color,
        },
        typography: ThemeTypography::default(),
        metrics: ThemeMetrics::default(),
        path: PathBuf::from("sample"),
    }
}

fn copy_dir(source: &Path, destination: &Path) -> Result<(), ExtensionError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use theme_engine::Registry;

    fn theme() -> LoadedTheme {
        Registry::new(vec!["../../themes".into()])
            .load("cyberpunk")
            .expect("cyberpunk should load")
    }

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "my-linux-extension-registry-{}-{}",
            name,
            std::process::id()
        ));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        directory
    }

    fn write_extension(root: &Path, id: &str, destination: &str, color: &str) {
        let directory = root.join(id);

        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join(MANIFEST_FILE),
            format!(
                "[extension]\nschema = 1\nid = \"{id}\"\nname = \"{id}\"\nversion = \"1.0.0\"\ndescription = \"Test extension\"\nformat = \"text\"\ndestination = \"{destination}\"\ntemplate = \"template.txt\"\n"
            ),
        )
        .unwrap();
        fs::write(directory.join("template.txt"), format!("color = {color}\n")).unwrap();
    }

    #[test]
    fn lists_builtin_and_user_extensions_with_shadowing() {
        let root = scratch("list");
        let builtin = root.join("builtin");
        let user = root.join("user");

        write_extension(&builtin, "demo", "demo/one.conf", "#111111");
        write_extension(&builtin, "other", "other/one.conf", "#222222");
        write_extension(&user, "demo", "demo/two.conf", "#333333");

        let registry = ExtensionRegistry::new(&builtin, &user);
        let discovery = registry.list();

        assert_eq!(discovery.extensions.len(), 2);
        assert!(discovery.broken.is_empty());

        let demo = discovery
            .extensions
            .iter()
            .find(|summary| summary.id == "demo")
            .unwrap();

        assert_eq!(demo.source, ExtensionSource::User);
        assert_eq!(demo.destination, "demo/two.conf");

        assert_eq!(registry.load("other").unwrap().metadata.id, "other");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_broken_extensions_without_failing_the_list() {
        let root = scratch("broken");
        let builtin = root.join("builtin");

        write_extension(&builtin, "good", "good/one.conf", "#111111");
        fs::create_dir_all(builtin.join("bad")).unwrap();
        fs::write(builtin.join("bad").join(MANIFEST_FILE), "not toml").unwrap();

        let discovery = ExtensionRegistry::new(&builtin, root.join("user")).list();

        assert_eq!(discovery.extensions.len(), 1);
        assert_eq!(discovery.broken.len(), 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn enable_and_disable_round_trip() {
        let root = scratch("enable");
        let builtin = root.join("builtin");
        let user = root.join("user");

        write_extension(&builtin, "demo", "demo/one.conf", "#111111");

        let registry = ExtensionRegistry::new(&builtin, &user);

        assert!(registry.enabled().unwrap().is_empty());

        registry.enable("demo").unwrap();
        assert_eq!(registry.enabled().unwrap(), vec!["demo".to_string()]);
        assert!(registry.list().extensions[0].enabled);
        assert!(registry.settings_file().is_file());

        registry.disable("demo").unwrap();
        assert!(registry.enabled().unwrap().is_empty());
        assert!(!registry.list().extensions[0].enabled);

        assert!(registry.enable("unknown").is_err());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn renders_enabled_extensions_and_rejects_destination_collisions() {
        let root = scratch("render");
        let builtin = root.join("builtin");

        write_extension(&builtin, "first", "app/one.conf", "#111111");
        write_extension(&builtin, "second", "app/two.conf", "#222222");

        let registry = ExtensionRegistry::new(&builtin, root.join("user"));

        registry.enable("first").unwrap();
        registry.enable("second").unwrap();

        let generated = registry.render_enabled(&theme()).unwrap();

        assert_eq!(generated.len(), 2);
        assert!(generated[0].content.contains("#111111"));
        assert!(generated[1].content.contains("#222222"));
        assert!(generated[0].content.contains("color = #111111"));

        write_extension(&builtin, "third", "app/one.conf", "#333333");
        registry.enable("third").unwrap();

        let error = registry.render_enabled(&theme()).unwrap_err();

        assert!(error.to_string().contains("both write"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn imports_exports_and_removes_a_user_extension() {
        let root = scratch("import");
        let builtin = root.join("builtin");
        let user = root.join("user");
        let package = root.join("package/demo");

        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join(MANIFEST_FILE),
            "[extension]\nschema = 1\nid = \"demo\"\nname = \"Demo\"\nversion = \"1.0.0\"\ndescription = \"Test\"\nformat = \"text\"\ndestination = \"demo/style.conf\"\ntemplate = \"template.txt\"\n",
        )
        .unwrap();
        fs::write(
            package.join("template.txt"),
            "accent = {{colors.primary}}\n",
        )
        .unwrap();

        let registry = ExtensionRegistry::new(&builtin, &user);

        let installed = registry.import(&package, None).unwrap();

        assert_eq!(installed, user.join("demo"));
        assert!(registry.load("demo").is_ok());

        // Importing the same id twice is refused rather than overwriting.
        assert!(registry.import(&package, None).is_err());

        // A renamed import is allowed.
        let renamed = registry.import(&package, Some("demo-copy")).unwrap();

        assert!(renamed.ends_with("demo-copy"));
        assert_eq!(registry.load("demo-copy").unwrap().metadata.id, "demo-copy");

        let exported = registry.export("demo", root.join("export")).unwrap();

        assert!(exported.join(MANIFEST_FILE).is_file());
        assert!(exported.join("template.txt").is_file());

        registry.enable("demo").unwrap();
        registry.remove("demo").unwrap();

        assert!(registry.load("demo").is_err());
        assert!(registry.enabled().unwrap().is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_to_remove_builtin_extensions() {
        let root = scratch("builtin-remove");
        let builtin = root.join("builtin");

        write_extension(&builtin, "demo", "demo/one.conf", "#111111");

        let registry = ExtensionRegistry::new(&builtin, root.join("user"));

        match registry.remove("demo") {
            Err(ExtensionError::Builtin(id)) => assert_eq!(id, "demo"),
            other => panic!("built-in extensions must be protected, got {other:?}"),
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scaffolds_a_renderable_extension() {
        let root = scratch("scaffold");
        let registry = ExtensionRegistry::new(root.join("builtin"), root.join("user"));

        let directory = registry
            .scaffold("my-app", "My App", "my-app/style.css", ExtensionFormat::Css)
            .unwrap();

        assert!(directory.join(MANIFEST_FILE).is_file());
        assert!(directory.join("template.css").is_file());

        let rendered = registry.render("my-app", &theme()).unwrap();

        assert!(rendered.content.contains("#FF235A"));
        assert!(
            registry
                .scaffold("my-app", "My App", "x/y.css", ExtensionFormat::Css)
                .is_err()
        );

        fs::remove_dir_all(root).unwrap();
    }
}
