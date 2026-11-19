//! GUI state: everything the graphical application shows is read from the
//! existing `desktop-core` crates. The GUI never renders a theme itself.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use desktop_core::assets::{
    AssetSettings, Proposal, WallpaperLibrary, WallpaperState, detect_tool, extract_palette,
    read_settings, save_theme as save_wallpaper_theme, set_wallpaper, write_settings,
};
use desktop_core::config_manager::{digest_file, install_content};
use desktop_core::extensions::{ExtensionRegistry, user_extensions_dir};
use desktop_core::integrations;
use desktop_core::log::{Event, read_events};
use desktop_core::profile_apply::{ProfileApplyOptions, apply_profile};
use desktop_core::profiles::{Profile, ProfileRegistry};
use desktop_core::transaction::{
    Diagnostic, FileState, HistoryEntry, Modification, apply_theme_with_extra,
    detect_modifications, diagnose, read_history, recover_interrupted, restore_theme,
    rollback_theme,
};
use desktop_core::{
    LoadedTheme, Registry, ThemeColors, ThemeMetadata, ThemeMetrics, ThemeSource, ThemeTypography,
    read_active_profile, read_active_theme, read_previous_profile, render_theme_parts,
    render_theme_toml, validate_id,
};

/// Directory of the built-in themes, in an installation or a source tree.
pub fn builtin_themes_dir() -> PathBuf {
    desktop_core::paths::builtin_themes_dir()
}

/// Directory of the built-in desktop profiles, in an installation or a source tree.
pub fn builtin_profiles_dir() -> PathBuf {
    desktop_core::paths::builtin_profiles_dir()
}

fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"))
}

/// Where user-created themes live (`MY_LINUX_USER_THEMES` overrides it).
pub fn user_themes_dir() -> PathBuf {
    std::env::var_os("MY_LINUX_USER_THEMES")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_home().join("my-linux/themes"))
}

/// Where user-created profiles live (`MY_LINUX_USER_PROFILES` overrides it).
pub fn user_profiles_dir() -> PathBuf {
    std::env::var_os("MY_LINUX_USER_PROFILES")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_home().join("my-linux/profiles"))
}

/// Default installation root: `MY_LINUX_ROOT`, else the XDG state directory.
pub fn default_root() -> PathBuf {
    if let Some(root) = std::env::var_os("MY_LINUX_ROOT") {
        return PathBuf::from(root);
    }

    let state_home = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from(".local/state"));

    state_home.join("my-linux")
}

/// One theme as shown in the gallery.
#[derive(Debug, Clone)]
pub struct ThemeCard {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: &'static str,
    pub colors: ThemeColors,
    pub typography: ThemeTypography,
    pub metrics: ThemeMetrics,
}

/// One extension row in the application manager.
#[derive(Debug, Clone)]
pub struct ExtensionRow {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub destination: String,
    pub source: &'static str,
    pub programs: Vec<String>,
    pub detected: Option<PathBuf>,
    pub enabled: bool,
    pub installed: bool,
}

impl ExtensionRow {
    pub fn available(&self) -> bool {
        self.detected.is_some()
    }

    pub fn state_label(&self) -> &'static str {
        if self.installed {
            "installed"
        } else if self.enabled {
            "enabled, not applied yet"
        } else {
            "not applied"
        }
    }
}

/// One integration row in the application manager.
#[derive(Debug, Clone)]
pub struct IntegrationRow {
    pub id: &'static str,
    pub name: &'static str,
    pub programs: Vec<&'static str>,
    pub destination: &'static str,
    pub detected: Option<PathBuf>,
    pub state: Option<FileState>,
}

impl IntegrationRow {
    pub fn available(&self) -> bool {
        self.detected.is_some()
    }

    pub fn state_label(&self) -> &'static str {
        match self.state {
            Some(state) => state.label(),
            None => "not installed",
        }
    }
}

/// One profile as shown in the profile manager.
#[derive(Debug, Clone)]
pub struct ProfileCard {
    pub id: String,
    pub name: String,
    pub theme: String,
    pub wallpaper: Option<String>,
    pub integrations: usize,
    pub source: &'static str,
    pub path: PathBuf,
}

/// One wallpaper in the library.
#[derive(Debug, Clone)]
pub struct WallpaperEntry {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
}

/// One configuration backup.
#[derive(Debug, Clone)]
pub struct BackupEntry {
    pub name: String,
    pub bytes: u64,
    pub written: u64,
    pub digest: String,
}

/// Everything the GUI displays, refreshed from the real installation.
pub struct GuiState {
    pub root: PathBuf,
    pub builtin_themes: PathBuf,
    pub user_themes: PathBuf,
    pub profiles: ProfileRegistry,
    pub library: WallpaperLibrary,
    pub themes: Vec<ThemeCard>,
    pub broken: Vec<String>,
    pub profiles_list: Vec<ProfileCard>,
    pub integrations: Vec<IntegrationRow>,
    pub extensions_registry: ExtensionRegistry,
    pub extensions: Vec<ExtensionRow>,
    pub wallpapers: Vec<WallpaperEntry>,
    pub active_theme: Option<String>,
    pub active_profile: Option<String>,
    pub previous_profile: Option<String>,
    pub wallpaper: Option<PathBuf>,
    pub assets: AssetSettings,
    pub events: Vec<Event>,
    pub modifications: Vec<Modification>,
    pub history: Vec<HistoryEntry>,
    pub backups: Vec<BackupEntry>,
}

impl GuiState {
    /// Builds the state for `root` and reads the installation once.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let mut state = Self::with_library(
            root,
            builtin_themes_dir(),
            user_themes_dir(),
            user_profiles_dir(),
            WallpaperLibrary::from_environment(),
        );

        state.refresh();

        state
    }

    /// Builds the state from explicit paths, without reading anything yet.
    pub fn with_library(
        root: impl Into<PathBuf>,
        builtin_themes: PathBuf,
        user_themes: PathBuf,
        user_profiles: PathBuf,
        library: WallpaperLibrary,
    ) -> Self {
        Self {
            root: root.into(),
            builtin_themes,
            user_themes,
            profiles: ProfileRegistry::new(builtin_profiles_dir(), user_profiles),
            library,
            themes: Vec::new(),
            broken: Vec::new(),
            profiles_list: Vec::new(),
            integrations: Vec::new(),
            extensions_registry: ExtensionRegistry::new(
                desktop_core::paths::builtin_extensions_dir(),
                user_extensions_dir(),
            ),
            extensions: Vec::new(),
            wallpapers: Vec::new(),
            active_theme: None,
            active_profile: None,
            previous_profile: None,
            wallpaper: None,
            assets: AssetSettings::default(),
            events: Vec::new(),
            modifications: Vec::new(),
            history: Vec::new(),
            backups: Vec::new(),
        }
    }

    /// Re-reads every piece of state from disk.
    pub fn refresh(&mut self) {
        self.active_theme = read_active_theme(&self.root);
        self.active_profile = read_active_profile(&self.root);
        self.previous_profile = read_previous_profile(&self.root);
        self.assets = read_settings(&self.root);
        self.wallpaper = WallpaperState::new(&self.root).current();
        self.events = read_events(&self.root);
        self.history = read_history(&self.root);

        let modifications = detect_modifications(&self.root);
        let mut integrations_rows = Vec::new();

        for status in integrations::status() {
            let state = modifications
                .iter()
                .find(|modification| modification.relative_path == status.destination)
                .map(|modification| modification.state);

            integrations_rows.push(IntegrationRow {
                id: status.id,
                name: status.name,
                programs: status.programs,
                destination: status.destination,
                detected: status.detected,
                state,
            });
        }

        self.integrations = integrations_rows;
        self.modifications = modifications;

        let discovery = self.extensions_registry.list();
        let mut extension_rows = Vec::new();

        for summary in &discovery.extensions {
            let programs = summary.programs.clone();
            let detected = {
                let borrowed: Vec<&str> = programs.iter().map(String::as_str).collect();

                desktop_core::integrations::detect(&borrowed)
            };

            extension_rows.push(ExtensionRow {
                id: summary.id.clone(),
                name: summary.name.clone(),
                version: summary.version.clone(),
                description: summary.description.clone(),
                destination: summary.destination.clone(),
                source: summary.source.label(),
                programs,
                detected,
                enabled: summary.enabled,
                installed: self.root.join(&summary.destination).is_file(),
            });
        }

        self.extensions = extension_rows;

        let discovery = self.registry().list();
        let mut themes = Vec::new();

        for summary in &discovery.themes {
            if let Ok(theme) = self.registry().load(&summary.id) {
                themes.push(ThemeCard {
                    id: summary.id.clone(),
                    name: summary.name.clone(),
                    version: summary.version.clone(),
                    description: summary.description.clone(),
                    source: summary.source.label(),
                    colors: theme.colors.clone(),
                    typography: theme.typography.clone(),
                    metrics: theme.metrics,
                });
            }
        }

        self.broken = discovery.errors();
        self.themes = themes;

        self.profiles_list = self
            .profiles
            .list()
            .into_iter()
            .map(|summary| ProfileCard {
                id: summary.id,
                name: summary.name,
                theme: summary.theme,
                wallpaper: summary.wallpaper,
                integrations: summary.integrations,
                source: summary.source.label(),
                path: summary.path,
            })
            .collect();

        self.wallpapers = self
            .library
            .list()
            .into_iter()
            .map(|wallpaper| WallpaperEntry {
                name: wallpaper.name,
                path: wallpaper.path,
                bytes: wallpaper.bytes,
            })
            .collect();

        self.backups = read_backups(&self.root);
    }

    pub fn registry(&self) -> Registry {
        Registry::with_user(self.builtin_themes.clone(), self.user_themes.clone())
    }

    pub fn theme(&self, id: &str) -> Result<LoadedTheme, String> {
        self.registry().load(id).map_err(|error| error.to_string())
    }

    /// Applies a theme. An empty `selected` list means every integration.
    pub fn apply_theme(&mut self, id: &str, selected: &[&'static str]) -> Result<String, String> {
        let theme = self.theme(id)?;
        let ids: Vec<String> = selected.iter().map(|id| id.to_string()).collect();
        let chosen = integrations::select(&ids)?;
        let generated = self
            .extensions_registry
            .render_enabled(&theme)
            .map_err(|error| error.to_string())?;
        let extra: Vec<(String, String)> = generated
            .iter()
            .map(|extension| (extension.destination.clone(), extension.content.clone()))
            .collect();
        let report = apply_theme_with_extra(&self.root, &theme, &chosen, &extra)
            .map_err(|error| error.to_string())?;

        self.refresh();

        Ok(format!(
            "Applied {} to {} file(s).",
            theme.metadata.name,
            report.installed_files.len()
        ))
    }

    /// Applies a complete desktop profile, including its wallpaper when a
    /// wallpaper daemon is available.
    pub fn apply_profile(&mut self, id: &str) -> Result<String, String> {
        let profile = self.profiles.load(id).map_err(|error| error.to_string())?;
        let theme = self
            .theme(&profile.metadata.theme)
            .map_err(|error| format!("Profile theme `{}`: {error}", profile.metadata.theme))?;
        let options = ProfileApplyOptions {
            set_wallpaper: detect_tool().is_some(),
            reload: false,
        };
        let report = apply_profile(&self.root, &profile, &theme, &options)
            .map_err(|error| error.to_string())?;

        self.refresh();

        let warnings = if report.warnings.is_empty() {
            String::new()
        } else {
            format!(" Warnings: {}.", report.warnings.join("; "))
        };

        Ok(format!(
            "Applied profile {} ({} files).{warnings}",
            profile.metadata.name,
            report.installed_files.len()
        ))
    }

    /// Applies the profile that was active before the current one.
    pub fn restore_previous_profile(&mut self) -> Result<String, String> {
        let id = self
            .previous_profile
            .clone()
            .ok_or("No previous profile is recorded")?;

        self.apply_profile(&id)
    }

    /// Restores every original configuration.
    pub fn restore(&mut self) -> Result<String, String> {
        let restored = restore_theme(&self.root).map_err(|error| error.to_string())?;

        self.refresh();

        Ok(format!("Restored {} original file(s).", restored.len()))
    }

    /// Undoes the most recent apply.
    pub fn rollback(&mut self) -> Result<String, String> {
        let restored = rollback_theme(&self.root).map_err(|error| error.to_string())?;

        self.refresh();

        Ok(format!("Rolled back {} file(s).", restored.len()))
    }

    /// Finishes an interrupted transaction, if one was left behind.
    pub fn recover(&mut self) -> Result<String, String> {
        let restored = recover_interrupted(&self.root).map_err(|error| error.to_string())?;

        self.refresh();

        if restored.is_empty() {
            Ok("No interrupted operation was found.".to_string())
        } else {
            Ok(format!("Recovered {} file(s).", restored.len()))
        }
    }

    /// Installs a wallpaper into the library, records it and asks a detected
    /// wallpaper daemon to display it.
    pub fn apply_wallpaper(&mut self, path: &Path) -> Result<String, String> {
        let installed = self
            .library
            .install(path)
            .map_err(|error| error.to_string())?;
        WallpaperState::new(&self.root)
            .set(&installed.path, "apply")
            .map_err(|error| error.to_string())?;

        let mut note = String::new();

        if let Some(tool) = detect_tool() {
            match set_wallpaper(tool, &installed.path) {
                Ok(_) => note = " The wallpaper daemon was updated.".to_string(),
                Err(error) => note = format!(" The wallpaper daemon failed: {error}"),
            }
        }

        self.refresh();

        Ok(format!("Wallpaper: {}.{note}", installed.path.display()))
    }

    /// Extracts a palette proposal from a wallpaper without saving anything.
    pub fn proposal(&self, path: &Path) -> Result<Proposal, String> {
        extract_palette(path).map_err(|error| error.to_string())
    }

    /// Saves a wallpaper-derived theme; the user decides whether to apply it.
    pub fn save_wallpaper_theme(
        &mut self,
        path: &Path,
        id: &str,
        name: &str,
    ) -> Result<String, String> {
        let proposal = self.proposal(path)?;
        let theme = save_wallpaper_theme(&self.user_themes, id, name, path, &proposal.palette)
            .map_err(|error| error.to_string())?;

        self.refresh();

        Ok(format!(
            "Saved theme {} ({}).",
            theme.metadata.name, theme.metadata.id
        ))
    }

    /// Saves an edited theme into the user theme directory and verifies that it
    /// loads back through the normal registry.
    pub fn save_theme(
        &mut self,
        id: &str,
        name: &str,
        base: &ThemeCard,
        colors: &ThemeColors,
        typography: &ThemeTypography,
        metrics: ThemeMetrics,
    ) -> Result<String, String> {
        validate_id(id).map_err(|error| error.to_string())?;

        if let Some(existing) = self.registry().find(id)
            && existing.source == ThemeSource::Builtin
        {
            return Err(format!(
                "`{id}` is a built-in theme and cannot be overwritten"
            ));
        }

        let metadata = ThemeMetadata {
            name: name.to_string(),
            id: id.to_string(),
            version: "1.0.0".to_string(),
            description: format!("Custom theme based on {}", base.name),
            author: Some("My Linux".to_string()),
            schema: 1,
            extends: None,
        };
        let file = self
            .user_themes
            .join(id)
            .join("colors")
            .join("palette.toml");

        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        install_content(
            render_theme_parts(&metadata, colors, typography, metrics).as_bytes(),
            &file,
        )
        .map_err(|error| error.to_string())?;

        self.registry()
            .load(id)
            .map_err(|error| format!("The saved theme does not load: {error}"))?;

        self.refresh();

        Ok(format!("Saved theme {name} ({id}) to {}.", file.display()))
    }

    /// Imports a theme directory or `.toml` file into the user theme directory.
    pub fn import_theme(&mut self, source: &Path, as_id: Option<&str>) -> Result<String, String> {
        let theme = Registry::load_path(source)
            .map_err(|error| format!("{}: {error}", source.display()))?;
        let id = as_id
            .map(str::to_lowercase)
            .unwrap_or_else(|| theme.metadata.id.to_lowercase());

        validate_id(&id).map_err(|error| error.to_string())?;

        if self.registry().find(&id).is_some() {
            return Err(format!("Theme `{id}` already exists"));
        }

        let mut imported = theme.clone();
        imported.metadata.id = id.clone();
        let file = self
            .user_themes
            .join(&id)
            .join("colors")
            .join("palette.toml");

        install_content(render_theme_toml(&imported).as_bytes(), &file)
            .map_err(|error| error.to_string())?;

        self.refresh();

        Ok(format!("Imported theme {id} from {}.", source.display()))
    }

    /// Exports a theme as a portable `.toml` file.
    pub fn export_theme(&mut self, id: &str, path: &Path) -> Result<String, String> {
        let theme = self.theme(id)?;
        let rendered = render_theme_toml(&theme);
        let output = if path.is_dir() {
            path.join(format!("{id}.toml"))
        } else {
            path.to_path_buf()
        };

        fs::write(&output, rendered).map_err(|error| error.to_string())?;

        Ok(format!("Exported {id} to {}.", output.display()))
    }

    /// Enables an extension so every later apply writes it too.
    pub fn enable_extension(&mut self, id: &str) -> Result<String, String> {
        let settings = self
            .extensions_registry
            .enable(id)
            .map_err(|error| error.to_string())?;

        self.refresh();

        Ok(format!("Enabled {id}. Settings: {}.", settings.display()))
    }

    /// Stops applying an extension.
    pub fn disable_extension(&mut self, id: &str) -> Result<String, String> {
        let settings = self
            .extensions_registry
            .disable(id)
            .map_err(|error| error.to_string())?;

        self.refresh();

        Ok(format!("Disabled {id}. Settings: {}.", settings.display()))
    }

    /// Removes a user extension. Built-in extensions are refused.
    pub fn remove_extension(&mut self, id: &str) -> Result<String, String> {
        let directory = self
            .extensions_registry
            .remove(id)
            .map_err(|error| error.to_string())?;

        self.refresh();

        Ok(format!(
            "Removed extension {id} from {}.",
            directory.display()
        ))
    }

    /// Renders one extension with a theme, for the live preview.
    pub fn render_extension(&self, id: &str, theme_id: &str) -> Result<String, String> {
        let theme = self.theme(theme_id)?;

        self.extensions_registry
            .render(id, &theme)
            .map(|generated| generated.content)
            .map_err(|error| error.to_string())
    }

    /// Removes a user theme. Built-in themes are refused.
    pub fn remove_theme(&mut self, id: &str) -> Result<String, String> {
        let directory = self.user_themes.join(id);

        if !directory.is_dir() {
            return Err(format!(
                "`{id}` is not a user theme; built-in themes cannot be removed"
            ));
        }

        fs::remove_dir_all(&directory).map_err(|error| error.to_string())?;

        self.refresh();

        Ok(format!("Removed user theme {id}."))
    }

    /// Writes font, icon and cursor settings and regenerates the snippet.
    pub fn save_assets(&mut self, settings: &AssetSettings) -> Result<String, String> {
        write_settings(&self.root, settings).map_err(|error| error.to_string())?;

        self.refresh();

        Ok("Asset settings written.".to_string())
    }

    /// Creates a new user profile referencing an existing theme.
    pub fn create_profile(&mut self, id: &str, name: &str, theme: &str) -> Result<String, String> {
        if self.profiles.find(id).is_some() {
            return Err(format!("Profile `{id}` already exists"));
        }

        let profile = Profile::new(id, name, theme).map_err(|error| error.to_string())?;

        self.profiles
            .save(&profile)
            .map_err(|error| error.to_string())?;

        self.refresh();

        Ok(format!("Created profile {name} ({id})."))
    }

    /// Imports a profile TOML file into the user profile directory.
    pub fn import_profile(&mut self, source: &Path) -> Result<String, String> {
        let contents = fs::read_to_string(source).map_err(|error| error.to_string())?;
        let profile = Profile::parse(&contents, source).map_err(|error| error.to_string())?;
        let id = profile.metadata.id.clone();
        let name = profile.metadata.name.clone();

        self.profiles
            .save(&profile)
            .map_err(|error| error.to_string())?;

        self.refresh();

        Ok(format!("Imported profile {name} ({id})."))
    }

    /// Exports a profile TOML file.
    pub fn export_profile(&mut self, id: &str, path: &Path) -> Result<String, String> {
        let profile = self.profiles.load(id).map_err(|error| error.to_string())?;
        let rendered = profile.to_toml().map_err(|error| error.to_string())?;
        let output = if path.is_dir() {
            path.join(format!("{id}.toml"))
        } else {
            path.to_path_buf()
        };

        fs::write(&output, rendered).map_err(|error| error.to_string())?;

        Ok(format!("Exported profile {id} to {}.", output.display()))
    }

    /// Runs every installation check.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        diagnose(&self.root, &self.builtin_themes, Some(&self.profiles))
    }

    /// Location of the generated Hyprland asset snippet.
    pub fn asset_snippet(&self) -> PathBuf {
        desktop_core::assets::hyprland_assets_file(&self.root)
    }
}

fn read_backups(root: &Path) -> Vec<BackupEntry> {
    let directory = root.join("backups");
    let mut paths: Vec<PathBuf> = match fs::read_dir(&directory) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect(),
        Err(_) => return Vec::new(),
    };

    paths.sort();

    paths
        .into_iter()
        .filter_map(|path| {
            let metadata = fs::metadata(&path).ok()?;
            let written = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(0);

            Some(BackupEntry {
                name: path.file_name()?.to_string_lossy().to_string(),
                bytes: metadata.len(),
                written,
                digest: digest_file(&path).unwrap_or_else(|_| "unreadable".to_string()),
            })
        })
        .collect()
}

/// Formats a byte count for the GUI.
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let kilobytes = bytes as f64 / 1024.0;

    if kilobytes < 1024.0 {
        return format!("{kilobytes:.1} KiB");
    }

    format!("{:.1} MiB", kilobytes / 1024.0)
}

/// Capitalises a theme or profile id for display.
pub fn display_name(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), characters.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use desktop_core::Color;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("my-linux-gui-{}-{}", name, std::process::id()));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(directory.join("wallpapers")).unwrap();

        directory
    }

    fn state_for(name: &str) -> GuiState {
        let directory = scratch(name);
        let wallpapers = directory.join("wallpapers");
        let mut state = GuiState::with_library(
            directory.join("install"),
            builtin_themes_dir(),
            directory.join("themes"),
            directory.join("profiles"),
            WallpaperLibrary::new(vec![wallpapers]),
        );

        state.refresh();

        state
    }

    #[test]
    fn reads_themes_and_integrations() {
        let state = state_for("read");

        assert_eq!(state.themes.len(), 4);
        assert!(state.themes.iter().any(|theme| theme.id == "cyberpunk"));
        assert_eq!(state.integrations.len(), 13);
        assert!(
            state
                .integrations
                .iter()
                .any(|integration| integration.id == "waybar")
        );
        assert!(state.profiles_list.len() >= 4);
        assert!(state.active_theme.is_none());
    }

    #[test]
    fn applies_themes_and_reports_state() {
        let mut state = state_for("apply");

        state.apply_theme("cyberpunk", &["waybar"]).unwrap();

        assert_eq!(state.active_theme.as_deref(), Some("cyberpunk"));
        assert_eq!(state.modifications.len(), 1);
        assert_eq!(state.modifications[0].state, FileState::Unchanged);
        assert!(
            state
                .events
                .iter()
                .any(|event| event.action == "apply" && event.detail.contains("cyberpunk"))
        );

        state.apply_theme("nature", &["waybar"]).unwrap();

        assert_eq!(state.active_theme.as_deref(), Some("nature"));

        // The first original configuration is what gets restored.
        let backups = state.backups.len();
        let restored = state.restore().unwrap();

        assert!(restored.contains("Restored"));
        assert_eq!(state.active_theme, None);
        assert!(state.backups.len() <= backups);
    }

    #[test]
    fn saves_custom_themes_through_the_registry() {
        let mut state = state_for("edit");
        let base = state
            .themes
            .iter()
            .find(|theme| theme.id == "cyberpunk")
            .unwrap()
            .clone();
        let mut colors = base.colors.clone();
        colors.primary = Color {
            red: 0x11,
            green: 0x22,
            blue: 0x33,
        };

        state
            .save_theme(
                "my-custom",
                "My Custom",
                &base,
                &colors,
                &base.typography,
                base.metrics,
            )
            .unwrap();

        let loaded = state.registry().load("my-custom").unwrap();

        assert_eq!(loaded.colors.primary.to_hex(), "#112233");
        assert!(
            state
                .themes
                .iter()
                .any(|theme| theme.id == "my-custom" && theme.source == "user")
        );

        // Built-in ids cannot be overwritten.
        let error = state
            .save_theme(
                "cyberpunk",
                "Hijack",
                &base,
                &colors,
                &base.typography,
                base.metrics,
            )
            .unwrap_err();

        assert!(error.contains("built-in"));

        state.remove_theme("my-custom").unwrap();
        assert!(!state.themes.iter().any(|theme| theme.id == "my-custom"));
    }

    #[test]
    fn saves_design_tokens_and_manages_extensions() {
        let mut state = state_for("design-tokens");
        let user_extensions = state.root.join("user-extensions");

        state.extensions_registry = ExtensionRegistry::new(
            desktop_core::paths::builtin_extensions_dir(),
            user_extensions,
        );
        state.refresh();

        assert!(state.extensions.iter().any(|row| row.id == "swaync"));
        assert!(state.extensions.iter().all(|row| !row.enabled));

        state.enable_extension("swaync").unwrap();
        state.apply_theme("cyberpunk", &["waybar"]).unwrap();

        assert!(state.root.join("swaync/style.css").is_file());
        assert!(
            state
                .extensions
                .iter()
                .any(|row| row.id == "swaync" && row.enabled && row.installed)
        );

        state.disable_extension("swaync").unwrap();
        assert!(state.extensions.iter().all(|row| !row.enabled));

        let base = state
            .themes
            .iter()
            .find(|theme| theme.id == "cyberpunk")
            .unwrap()
            .clone();
        let colors = base.colors.clone();
        let typography = ThemeTypography {
            font_family: "Inter".to_string(),
            mono_family: "Iosevka".to_string(),
            font_size: 13,
        };
        let metrics = ThemeMetrics {
            radius: 5,
            spacing: 9,
            border_width: 1,
            opacity: 96,
        };

        state
            .save_theme("tokens", "Tokens", &base, &colors, &typography, metrics)
            .unwrap();

        let loaded = state.registry().load("tokens").unwrap();

        assert_eq!(loaded.typography.mono_family, "Iosevka");
        assert_eq!(loaded.typography.font_size, 13);
        assert_eq!(loaded.metrics.radius, 5);
        assert_eq!(loaded.metrics.border_width, 1);
        assert_eq!(loaded.metrics.opacity, 96);
    }

    #[test]
    fn creates_and_applies_profiles() {
        let mut state = state_for("profile");

        state
            .create_profile("ui-profile", "UI Profile", "nature")
            .unwrap();

        assert!(
            state
                .profiles_list
                .iter()
                .any(|profile| profile.id == "ui-profile")
        );

        state.apply_profile("ui-profile").unwrap();

        assert_eq!(state.active_profile.as_deref(), Some("ui-profile"));
        assert_eq!(state.active_theme.as_deref(), Some("nature"));
        assert!(
            state
                .events
                .iter()
                .any(|event| event.action == "profile" && event.detail.contains("ui-profile"))
        );
    }

    #[test]
    fn diagnostics_include_the_new_checks() {
        let mut state = state_for("diagnostics");

        state.apply_theme("cyberpunk", &["waybar"]).unwrap();

        let diagnostics = state.diagnostics();

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.label == "Event log" && diagnostic.ok)
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.label == "Active profile")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.label == "Asset settings")
        );
    }
}
