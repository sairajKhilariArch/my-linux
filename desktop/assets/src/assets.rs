//! Fonts, icon themes and cursor themes.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use config_manager::install_content;

use crate::AssetError;
use crate::wallpaper::program_exists;

const FONT_EXTENSIONS: [&str; 5] = ["ttf", "otf", "ttc", "woff", "woff2"];
const MAX_DEPTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    Font,
    Icon,
    Cursor,
}

impl AssetKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Font => "font",
            Self::Icon => "icon",
            Self::Cursor => "cursor",
        }
    }

    /// Directory name inside the My Linux asset library.
    pub const fn directory(self) -> &'static str {
        match self {
            Self::Font => "fonts",
            Self::Icon => "icons",
            Self::Cursor => "cursors",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_lowercase().as_str() {
            "font" | "fonts" => Some(Self::Font),
            "icon" | "icons" | "icon-theme" => Some(Self::Icon),
            "cursor" | "cursors" | "cursor-theme" => Some(Self::Cursor),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredAsset {
    pub kind: AssetKind,
    pub name: String,
    pub path: PathBuf,
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn data_home() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"))
}

/// Directories searched for `kind`, user directories first.
pub fn search_dirs(kind: AssetKind) -> Vec<PathBuf> {
    let data_home = data_home();

    match kind {
        AssetKind::Font => {
            let mut dirs = vec![data_home.join("fonts")];

            if let Some(home) = home_dir() {
                dirs.push(home.join(".fonts"));
            }

            dirs.push(PathBuf::from("/usr/share/fonts"));
            dirs
        }
        AssetKind::Icon | AssetKind::Cursor => {
            let mut dirs = vec![data_home.join("icons")];

            if let Some(home) = home_dir() {
                dirs.push(home.join(".icons"));
            }

            dirs.push(PathBuf::from("/usr/share/icons"));
            dirs
        }
    }
}

/// Discovers the assets installed on the system.
pub fn discover(kind: AssetKind) -> Vec<DiscoveredAsset> {
    discover_in(&search_dirs(kind), kind)
}

/// Discovers assets in an explicit set of directories.
pub fn discover_in(dirs: &[PathBuf], kind: AssetKind) -> Vec<DiscoveredAsset> {
    let mut found: BTreeMap<String, PathBuf> = BTreeMap::new();

    for directory in dirs {
        match kind {
            AssetKind::Font => collect_fonts(directory, 0, &mut found),
            AssetKind::Icon => collect_theme_dirs(directory, false, &mut found),
            AssetKind::Cursor => collect_theme_dirs(directory, true, &mut found),
        }
    }

    found
        .into_iter()
        .map(|(name, path)| DiscoveredAsset { kind, name, path })
        .collect()
}

fn collect_fonts(directory: &Path, depth: usize, found: &mut BTreeMap<String, PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }

    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            collect_fonts(&path, depth + 1, found);
            continue;
        }

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_lowercase);

        if !extension
            .as_deref()
            .is_some_and(|extension| FONT_EXTENSIONS.contains(&extension))
        {
            continue;
        }

        if let Some(family) = font_family(&path) {
            found.entry(family).or_insert(path);
        }
    }
}

/// Font family from the file name (`JetBrainsMono-Bold.ttf` → `JetBrainsMono`).
fn font_family(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let family = stem.split('-').next().unwrap_or(stem).replace('_', " ");
    let family = family.trim();

    if family.is_empty() {
        None
    } else {
        Some(family.to_string())
    }
}

fn collect_theme_dirs(directory: &Path, cursors: bool, found: &mut BTreeMap<String, PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let is_theme = if cursors {
            path.join("cursors").is_dir()
        } else {
            path.join("index.theme").is_file()
        };

        if !is_theme {
            continue;
        }

        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            found.entry(name.to_string()).or_insert(path);
        }
    }
}

/// Interface font, icon theme and cursor theme chosen for this installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetSettings {
    pub font: Option<String>,
    pub font_size: u32,
    pub icon_theme: Option<String>,
    pub cursor_theme: Option<String>,
    pub cursor_size: u32,
}

impl Default for AssetSettings {
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

impl AssetSettings {
    /// Stable, human readable representation written to `state/assets.conf`.
    pub fn render(&self) -> String {
        let mut output = String::new();

        if let Some(font) = &self.font {
            output.push_str(&format!("font={font}\n"));
        }

        output.push_str(&format!("font_size={}\n", self.font_size));

        if let Some(icon) = &self.icon_theme {
            output.push_str(&format!("icon_theme={icon}\n"));
        }

        if let Some(cursor) = &self.cursor_theme {
            output.push_str(&format!("cursor_theme={cursor}\n"));
        }

        output.push_str(&format!("cursor_size={}\n", self.cursor_size));

        output
    }

    pub fn parse(contents: &str) -> Self {
        let mut settings = Self::default();

        for line in contents.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            let value = value.trim();

            if value.is_empty() {
                continue;
            }

            match key.trim() {
                "font" => settings.font = Some(value.to_string()),
                "font_size" => {
                    if let Ok(size) = value.parse() {
                        settings.font_size = size;
                    }
                }
                "icon_theme" => settings.icon_theme = Some(value.to_string()),
                "cursor_theme" => settings.cursor_theme = Some(value.to_string()),
                "cursor_size" => {
                    if let Ok(size) = value.parse() {
                        settings.cursor_size = size;
                    }
                }
                _ => {}
            }
        }

        settings
    }
}

fn settings_file(root: &Path) -> PathBuf {
    root.join("state").join("assets.conf")
}

/// The asset snippet sourced by the Hyprland configuration.
pub fn hyprland_assets_file(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join("hyprland").join("assets.conf")
}

pub fn read_settings(root: impl AsRef<Path>) -> AssetSettings {
    fs::read_to_string(settings_file(root.as_ref()))
        .map(|contents| AssetSettings::parse(&contents))
        .unwrap_or_default()
}

pub fn write_settings(root: impl AsRef<Path>, settings: &AssetSettings) -> Result<(), AssetError> {
    let root = root.as_ref();

    install_content(settings.render().as_bytes(), settings_file(root))?;

    if settings.font.is_none()
        && settings.icon_theme.is_none()
        && settings.cursor_theme.is_none()
        && settings.cursor_size == AssetSettings::default().cursor_size
    {
        remove_if_exists(&hyprland_assets_file(root))?;
        return Ok(());
    }

    install_content(
        render_hyprland_assets(settings).as_bytes(),
        hyprland_assets_file(root),
    )?;

    Ok(())
}

/// Renders the Hyprland snippet that applies cursor settings.
pub fn render_hyprland_assets(settings: &AssetSettings) -> String {
    let mut output = String::new();

    output.push_str("# Generated by My Linux Theme Engine. Do not edit manually.\n");
    output.push_str("# Source it from ~/.config/hypr/hyprland.conf:\n");
    output.push_str("#   source = ~/.config/hypr/assets.conf\n\n");

    if let Some(theme) = &settings.cursor_theme {
        output.push_str(&format!("env = XCURSOR_THEME,{theme}\n"));
    }

    output.push_str(&format!("env = XCURSOR_SIZE,{}\n", settings.cursor_size));
    output.push_str(&format!(
        "\ncursor {{\n    size = {}\n}}\n",
        settings.cursor_size
    ));

    if let Some(font) = &settings.font {
        output.push_str(&format!(
            "\n# Interface font: {font} {}\n",
            settings.font_size
        ));
    }

    if let Some(icon) = &settings.icon_theme {
        output.push_str(&format!("# Icon theme: {icon}\n"));
    }

    output
}

/// Applies the cursor theme to a running Hyprland session.
pub fn apply_cursor(theme: &str, size: u32) -> Result<String, AssetError> {
    if !program_exists("hyprctl") {
        return Err(AssetError::Tool("hyprctl is not installed".to_string()));
    }

    let status = Command::new("hyprctl")
        .args(["setcursor", theme, &size.to_string()])
        .status()
        .map_err(|error| AssetError::Tool(format!("hyprctl: {error}")))?;

    if !status.success() {
        return Err(AssetError::Tool(format!(
            "`hyprctl setcursor` exited with {}",
            status.code().unwrap_or(-1)
        )));
    }

    Ok(format!("hyprctl setcursor {theme} {size}"))
}

/// Directory that holds assets installed by My Linux.
pub fn library_dir(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join("assets")
}

/// Copies a font file or a theme directory into the asset library.
pub fn install_asset(
    root: impl AsRef<Path>,
    kind: AssetKind,
    source: impl AsRef<Path>,
) -> Result<PathBuf, AssetError> {
    let source = source.as_ref();

    if !source.exists() {
        return Err(AssetError::Invalid(format!(
            "{} does not exist",
            source.display()
        )));
    }

    let directory = library_dir(root).join(kind.directory());

    fs::create_dir_all(&directory)?;

    let name = source
        .file_name()
        .ok_or_else(|| AssetError::Invalid("The asset has no name".to_string()))?;

    let target = directory.join(name);

    if target.exists() {
        return Ok(target);
    }

    if source.is_dir() {
        copy_directory(source, &target)?;
    } else {
        fs::copy(source, &target)?;
    }

    Ok(target)
}

/// Assets installed in the My Linux library.
pub fn installed_assets(root: impl AsRef<Path>) -> Vec<DiscoveredAsset> {
    let mut assets = Vec::new();

    for kind in [AssetKind::Font, AssetKind::Icon, AssetKind::Cursor] {
        let directory = library_dir(root.as_ref()).join(kind.directory());

        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();

            if name.is_empty() {
                continue;
            }

            assets.push(DiscoveredAsset { kind, name, path });
        }
    }

    assets.sort_by(|left, right| {
        left.kind
            .directory()
            .cmp(right.kind.directory())
            .then_with(|| left.name.cmp(&right.name))
    });

    assets
}

/// Removes an asset from the My Linux library.
pub fn remove_asset(
    root: impl AsRef<Path>,
    kind: AssetKind,
    name: &str,
) -> Result<PathBuf, AssetError> {
    let target = library_dir(root).join(kind.directory()).join(name);

    if !target.exists() {
        return Err(AssetError::Invalid(format!(
            "No {} named `{name}` is installed",
            kind.label()
        )));
    }

    if target.is_dir() {
        fs::remove_dir_all(&target)?;
    } else {
        fs::remove_file(&target)?;
    }

    Ok(target)
}

fn copy_directory(source: &Path, target: &Path) -> Result<(), AssetError> {
    fs::create_dir_all(target)?;

    for entry in fs::read_dir(source)?.flatten() {
        let path = entry.path();
        let destination = target.join(entry.file_name());

        if path.is_dir() {
            copy_directory(&path, &destination)?;
        } else {
            fs::copy(&path, &destination)?;
        }
    }

    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), AssetError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("my-linux-assets-{}-{}", name, std::process::id()));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        directory
    }

    #[test]
    fn discovers_fonts_icons_and_cursors_in_explicit_directories() {
        let directory = scratch("discover");

        let fonts = directory.join("fonts");
        fs::create_dir_all(fonts.join("nested")).unwrap();
        fs::write(fonts.join("Inter-Regular.ttf"), "x").unwrap();
        fs::write(fonts.join("nested").join("FiraCode-Bold.otf"), "x").unwrap();
        fs::write(fonts.join("README.txt"), "x").unwrap();

        let icons = directory.join("icons");
        fs::create_dir_all(icons.join("Papirus")).unwrap();
        fs::write(icons.join("Papirus").join("index.theme"), "x").unwrap();
        fs::create_dir_all(icons.join("Bibata").join("cursors")).unwrap();

        let font_list = discover_in(std::slice::from_ref(&fonts), AssetKind::Font);
        assert_eq!(
            font_list
                .iter()
                .map(|asset| asset.name.as_str())
                .collect::<Vec<&str>>(),
            vec!["FiraCode", "Inter"]
        );

        let icon_list = discover_in(std::slice::from_ref(&icons), AssetKind::Icon);
        assert_eq!(icon_list.len(), 1);
        assert_eq!(icon_list[0].name, "Papirus");

        let cursor_list = discover_in(std::slice::from_ref(&icons), AssetKind::Cursor);
        assert_eq!(cursor_list.len(), 1);
        assert_eq!(cursor_list[0].name, "Bibata");

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn settings_round_trip_through_the_state_file() {
        let directory = scratch("settings");

        let settings = AssetSettings {
            font: Some("Inter".to_string()),
            font_size: 12,
            icon_theme: Some("Papirus".to_string()),
            cursor_theme: Some("Bibata".to_string()),
            cursor_size: 28,
        };

        assert_eq!(AssetSettings::parse(&settings.render()), settings);

        write_settings(&directory, &settings).unwrap();

        assert_eq!(read_settings(&directory), settings);
        assert!(hyprland_assets_file(&directory).is_file());

        let snippet = fs::read_to_string(hyprland_assets_file(&directory)).unwrap();

        assert!(snippet.contains("env = XCURSOR_THEME,Bibata"));
        assert!(snippet.contains("env = XCURSOR_SIZE,28"));
        assert!(snippet.contains("size = 28"));
        assert!(snippet.contains("Interface font: Inter 12"));

        // Defaults remove the snippet again.
        write_settings(&directory, &AssetSettings::default()).unwrap();
        assert!(!hyprland_assets_file(&directory).exists());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn installs_and_removes_assets_in_the_library() {
        let directory = scratch("library");
        let source = directory.join("Inter-Regular.ttf");

        fs::write(&source, "font").unwrap();

        let installed = install_asset(&directory, AssetKind::Font, &source).unwrap();

        assert_eq!(
            installed,
            library_dir(&directory)
                .join("fonts")
                .join("Inter-Regular.ttf")
        );
        assert_eq!(installed_assets(&directory).len(), 1);

        remove_asset(&directory, AssetKind::Font, "Inter-Regular.ttf").unwrap();
        assert!(installed_assets(&directory).is_empty());
        assert!(remove_asset(&directory, AssetKind::Font, "Inter-Regular.ttf").is_err());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn parses_asset_kinds() {
        assert_eq!(AssetKind::parse("fonts"), Some(AssetKind::Font));
        assert_eq!(AssetKind::parse("Icon"), Some(AssetKind::Icon));
        assert_eq!(AssetKind::parse("cursor-theme"), Some(AssetKind::Cursor));
        assert_eq!(AssetKind::parse("wallpapers"), None);
    }
}
