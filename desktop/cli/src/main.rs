use desktop_core::{
    Color, Registry, ThemeMetadata, ThemeMetrics, ThemeTypography,
    config_manager::install_content,
    extensions::{ExtensionFormat, ExtensionRegistry, user_extensions_dir},
    generate_palette, integrations, read_active_theme, render_theme_full, render_theme_toml,
    transaction::{
        FileState, WAYBAR_RELATIVE_PATH, apply_theme_with_extra, apply_theme_with_extra_options,
        backup_records, destination, detect_modifications, diagnose, format_timestamp,
        installed_entries, original_backup, prune_backups, read_history,
        refresh_lock_screen_config, restore_theme, rollback_theme,
    },
    validate_id,
};

use desktop_core::assets::{
    AssetKind, AssetSettings, WallpaperLibrary, WallpaperState, apply_cursor, detect_tool,
    discover, extract_palette, hyprland_assets_file, install_asset, installed_assets,
    read_settings, remove_asset, render_preview, save_theme, set_wallpaper, write_settings,
};
use desktop_core::log::{Level, append_event, last_event, read_events};
use desktop_core::profile_apply::{ProfileApplyOptions, apply_profile_with_extensions};
use desktop_core::profiles::{Profile, ProfileRegistry};
use desktop_core::{read_active_profile, read_previous_profile};
use desktop_installer::{
    InstallLayout, InstallOptions, default_prefix, dependency_report, detect_environment, install,
    install_status, uninstall, upgrade,
};
use desktop_repository as repository;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const HELP: &str = "\
My Linux Theme Manager

Usage:
  my-linux-theme [--root <directory>] <command> [arguments]

Theme commands:
  list                     List every discovered theme
  info <theme>             Show theme metadata and the full palette
  preview <theme>          Preview theme colours with terminal swatches
  render <theme> [--integration <id>]
                           Print a generated application configuration
  create <id> [--from <theme> | --base <#RRGGBB>]
                           Create a user theme
  duplicate <from> <to>    Copy a theme, including tokens and overrides
  bump <id> <major|minor|patch>
                           Raise a theme's version and write the user copy
  import <path> [--as <id>]
                           Import a theme directory or .toml file
  export <theme> [file]    Export a theme as a portable .toml file
  remove <id>              Remove a user theme

Desktop commands:
  apply <theme> [--integration <id>]... [--reload] [--force]
                           Apply a theme as a single transaction; --force
                           replaces generated files you edited
  restore                  Restore the original configuration
  rollback                 Undo the most recent apply
  status                   Show the active theme and installation state

Integration commands:
  integrations list        List supported desktop integrations
  integrations status      Show which integrations are installed

Wallpaper commands:
  wallpaper list           List wallpapers in the library
  wallpaper preview <name> [--width <cells>]
                           Print an ANSI preview of a wallpaper
  wallpaper apply <name> [--no-set]
                           Install, record and set a wallpaper
  wallpaper current        Show the active wallpaper
  wallpaper history        Show wallpaper changes
  wallpaper restore        Return to the previous wallpaper
  wallpaper palette <name> [--name <id>] [--save] [--apply]
                           Extract a theme from a wallpaper

Asset commands:
  assets fonts|icons|cursors
                           List discovered fonts, icon and cursor themes
  assets status            Show asset settings and discovered assets
  assets set [--font <family>] [--font-size <n>] [--icon-theme <name>]
             [--cursor-theme <name>] [--cursor-size <n>] [--reload]
                           Store asset settings and write the Hyprland snippet
  assets reset             Clear asset settings
  assets install <path> [--kind font|icon|cursor]
                           Copy an asset into the My Linux library
  assets installed         List assets in the My Linux library
  assets remove <name> [--kind font|icon|cursor]
                           Remove an installed asset

Profile commands:
  profile list             List built-in and user profiles
  profile info <id>        Show a profile's theme, assets and layout
  profile preview <id>     Show a profile with theme colour swatches
  profile create <id> [--name <name>] [--theme <theme>] [--from <profile>]
                           Create a user profile
  profile set <id> <field> <value>
                           Change one profile field
  profile duplicate <from> <to>
                           Copy a profile
  profile remove <id>      Remove a user profile
  profile import <path> [--as <id>]
                           Import a profile TOML file
  profile export <id> [file]
                           Export a profile as TOML
  profile apply <id> [--reload] [--no-set]
                           Apply a complete desktop profile
  profile restore          Apply the previously active profile
  profile current          Show the active and previous profiles

Repository commands:
  repository list [--index <path|git-url>]
                           List packages in a community repository index
  repository search <query> [--index ...]
                           Search packages by id, name or description
  repository info <id> [--index ...]
                           Show one package
  repository install <id> [--index ...] [--force]
                           Install a theme or extension from the repository
  repository sync [<location>]
                           Fetch or update an index in the cache

Extension commands:
  extensions list          List built-in and user extensions
  extensions info <id>     Show an extension's manifest and template
  extensions render <id> [--theme <theme>]
                           Print an extension's generated configuration
  extensions create <id> [--name <name>] [--destination <path>] [--format <fmt>]
                           Create a user extension from a template
  extensions import <path> [--as <id>]
                           Install an extension directory
  extensions export <id> [directory]
                           Copy an extension out as a portable directory
  extensions remove <id>   Remove a user extension
  extensions enable <id>   Apply the extension with every theme
  extensions disable <id>  Stop applying the extension
  extensions status        Show enabled extensions and application detection

State commands:
  current                  Show the active theme, profile, wallpaper and assets
  logs [--limit <n>]       Show the structured event log

Recovery commands:
  backup list              List configuration backups and verify their digests
  backup prune [--keep <days>] [--dry-run]
                           Remove previous snapshots older than 30 days
                           (original backups are always kept)
  history                  Show the installation history
  doctor                   Diagnose the installation

Installation commands:
  install [--prefix <dir>] [--source <dir>] [--force] [--dry-run]
                           Install both binaries, themes, profiles and a desktop entry
  upgrade [--prefix <dir>] [--dry-run]
                           Replace program files, preserving modified content
  uninstall [--prefix <dir>] [--dry-run]
                           Remove installed program files and keep your data
  install-status [--prefix <dir>]
                           Show the installed version and verify every file

Options:
  --root <directory>       Directory that receives generated configurations
                           (default: $MY_LINUX_ROOT, otherwise target/theme-test)
  --prefix <directory>     Where install/upgrade/uninstall place program files
                           (default: $MY_LINUX_PREFIX, otherwise ~/.local)
  -h, --help               Show this help

User themes live in $MY_LINUX_USER_THEMES, otherwise
$XDG_DATA_HOME/my-linux/themes (~/.local/share/my-linux/themes).
";

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {}", error);
        std::process::exit(1);
    }
}

/// Built-in themes are read from the source tree during development.
fn builtin_themes_dir() -> PathBuf {
    desktop_core::paths::builtin_themes_dir()
}

fn user_themes_dir() -> PathBuf {
    if let Some(directory) = env::var_os("MY_LINUX_USER_THEMES") {
        return PathBuf::from(directory);
    }

    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"));

    data_home.join("my-linux").join("themes")
}

fn registry() -> Registry {
    Registry::with_user(builtin_themes_dir(), user_themes_dir())
}

/// Theme registry under a name that cannot be shadowed by a local binding.
fn theme_registry() -> Registry {
    registry()
}

fn default_root() -> PathBuf {
    if let Some(root) = env::var_os("MY_LINUX_ROOT") {
        return PathBuf::from(root);
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/theme-test")
}

/// Splits `--root <dir>` (or `--root=<dir>`) from the command and its arguments.
fn parse_args(args: Vec<String>) -> Result<(Option<PathBuf>, Vec<String>), String> {
    let mut root = None;
    let mut remaining = Vec::new();
    let mut args = args.into_iter();

    while let Some(argument) = args.next() {
        if argument == "--root" {
            let value = args
                .next()
                .ok_or_else(|| "--root requires a directory".to_string())?;

            root = Some(PathBuf::from(value));
        } else if let Some(value) = argument.strip_prefix("--root=") {
            if value.is_empty() {
                return Err("--root requires a directory".to_string());
            }

            root = Some(PathBuf::from(value));
        } else {
            remaining.push(argument);
        }
    }

    Ok((root, remaining))
}

fn color_block(hex: &str, label: &str) {
    let hex = hex.trim_start_matches('#');

    if hex.len() != 6 {
        return;
    }

    let red = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let green = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let blue = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);

    print!("\x1b[48;2;{};{};{}m   \x1b[0m ", red, green, blue);
    println!("{:<12} #{}", label, hex.to_uppercase());
}

fn describe(path: &Path) -> String {
    if path.exists() {
        format!("present ({})", path.display())
    } else {
        format!("missing ({})", path.display())
    }
}

fn display_name(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn theme_path(id: &str) -> PathBuf {
    user_themes_dir()
        .join(id)
        .join("colors")
        .join("palette.toml")
}

fn command_list() {
    let discovery = registry().list();

    println!("Available themes:");

    for theme in &discovery.themes {
        println!(
            "  - {:<14} {} v{} ({})",
            theme.id,
            theme.name,
            theme.version,
            theme.source.label()
        );
    }

    if discovery.themes.is_empty() {
        println!("  (none found)");
    }

    if !discovery.broken.is_empty() {
        println!();
        println!("Invalid themes:");

        for broken in &discovery.broken {
            println!("  - {}: {}", broken.path.display(), broken.error);
        }
    }
}

fn command_create(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let id = rest
        .first()
        .ok_or("Usage: my-linux-theme create <id> [--from <theme>] [--base <#RRGGBB>]")?
        .to_lowercase();

    validate_id(&id)?;

    let mut from: Option<String> = None;
    let mut base: Option<Color> = None;
    let mut index = 1;

    while index < rest.len() {
        match rest[index].as_str() {
            "--from" => {
                let value = rest.get(index + 1).ok_or("--from requires a theme id")?;

                from = Some(value.clone());
                index += 2;
            }
            "--base" => {
                let value = rest
                    .get(index + 1)
                    .ok_or("--base requires a colour such as #6C8CFF")?;

                base = Some(
                    Color::from_hex(value)
                        .ok_or_else(|| format!("Invalid colour `{value}`: expected #RRGGBB"))?,
                );
                index += 2;
            }
            other => return Err(format!("Unknown option: {other}").into()),
        }
    }

    let registry = registry();

    if registry.find(&id).is_some() {
        return Err(format!("Theme `{id}` already exists").into());
    }

    let (colors, typography, metrics, overrides) = match (from, base) {
        (Some(parent), _) => {
            let parent = registry.load(&parent)?;

            (
                parent.colors,
                parent.typography,
                parent.metrics,
                parent.overrides,
            )
        }
        (None, Some(base)) => (
            generate_palette(base),
            ThemeTypography::default(),
            ThemeMetrics::default(),
            std::collections::BTreeMap::new(),
        ),
        (None, None) => (
            generate_palette(Color::new(0x6C, 0x8C, 0xFF)),
            ThemeTypography::default(),
            ThemeMetrics::default(),
            std::collections::BTreeMap::new(),
        ),
    };

    let metadata = ThemeMetadata {
        schema: 1,
        name: display_name(&id),
        id: id.clone(),
        version: "0.1.0".to_string(),
        description: format!("User theme `{id}`"),
        author: None,
        extends: None,
    };

    let path = theme_path(&id);

    install_content(
        render_theme_full(&metadata, &colors, &typography, metrics, &overrides).as_bytes(),
        &path,
    )?;

    println!("Theme created.");
    println!("Id:   {id}");
    println!("File: {}", path.display());
    println!("Next: my-linux-theme preview {id}");

    Ok(())
}

fn bump_version(version: &str, part: &str) -> Result<String, Box<dyn std::error::Error>> {
    let numbers: Vec<u64> = version
        .split('.')
        .map(|number| number.parse::<u64>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("Invalid theme version `{version}`: expected MAJOR.MINOR.PATCH"))?;

    if numbers.len() != 3 {
        return Err(
            format!("Invalid theme version `{version}`: expected MAJOR.MINOR.PATCH").into(),
        );
    }

    let (major, minor, patch) = match part {
        "major" => (numbers[0] + 1, 0, 0),
        "minor" => (numbers[0], numbers[1] + 1, 0),
        "patch" => (numbers[0], numbers[1], numbers[2] + 1),
        other => {
            return Err(
                format!("Unknown version part `{other}`: use major, minor or patch").into(),
            );
        }
    };

    Ok(format!("{major}.{minor}.{patch}"))
}

fn command_bump(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let id = rest
        .first()
        .ok_or("Usage: my-linux-theme bump <id> <major|minor|patch>")?;
    let part = rest
        .get(1)
        .ok_or("Usage: my-linux-theme bump <id> <major|minor|patch>")?;

    let mut theme = registry().load(id)?;
    let next = bump_version(&theme.metadata.version, part)?;

    theme.metadata.version = next.clone();

    let path = theme_path(id);

    install_content(render_theme_toml(&theme).as_bytes(), &path)?;

    println!("Theme version updated.");
    println!("Id:      {}", theme.metadata.id);
    println!("Version: {next}");
    println!("File:    {}", path.display());

    Ok(())
}

fn command_duplicate(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let from = rest
        .first()
        .ok_or("Usage: my-linux-theme duplicate <from> <to>")?;
    let to = rest
        .get(1)
        .ok_or("Usage: my-linux-theme duplicate <from> <to>")?
        .to_lowercase();

    validate_id(&to)?;

    let registry = registry();
    let mut theme = registry.load(from)?;

    if registry.find(&to).is_some() {
        return Err(format!("Theme `{to}` already exists").into());
    }

    theme.metadata.id = to.clone();
    theme.metadata.name = format!("{} Copy", theme.metadata.name);
    theme.metadata.version = "0.1.0".to_string();
    theme.metadata.extends = None;

    let path = theme_path(&to);

    install_content(render_theme_toml(&theme).as_bytes(), &path)?;

    println!("Theme duplicated.");
    println!("From: {from}");
    println!("Id:   {to}");
    println!("File: {}", path.display());

    Ok(())
}

fn command_import(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let source = rest
        .first()
        .ok_or("Usage: my-linux-theme import <path> [--as <id>]")?;

    let mut as_id: Option<String> = None;

    if let Some(option) = rest.get(1) {
        if option != "--as" {
            return Err(format!("Unknown option: {option}").into());
        }

        as_id = Some(
            rest.get(2)
                .ok_or("--as requires a theme id")?
                .to_lowercase(),
        );
    }

    let theme = Registry::load_path(Path::new(source))?;
    let id = as_id
        .unwrap_or_else(|| theme.metadata.id.clone())
        .to_lowercase();

    validate_id(&id)?;

    if registry().find(&id).is_some() {
        return Err(format!("Theme `{id}` already exists").into());
    }

    let mut imported = theme.clone();
    imported.metadata.id = id.clone();

    let path = theme_path(&id);

    install_content(render_theme_toml(&imported).as_bytes(), &path)?;

    println!("Theme imported.");
    println!("Id:     {id}");
    println!("Source: {source}");
    println!("File:   {}", path.display());

    Ok(())
}

fn command_export(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let name = rest
        .first()
        .ok_or("Usage: my-linux-theme export <theme> [file]")?;

    let theme = registry().load(name)?;

    let output = match rest.get(1) {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(format!("{}.toml", theme.metadata.id)),
    };

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&output, render_theme_toml(&theme))?;

    println!("Theme exported successfully.");
    println!("File: {}", output.display());

    Ok(())
}

fn command_remove(root: &Path, rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let id = rest
        .first()
        .ok_or("Usage: my-linux-theme remove <id>")?
        .to_lowercase();

    validate_id(&id)?;

    let user_root = user_themes_dir();
    let directory = user_root.join(&id);
    let file = user_root.join(format!("{id}.toml"));

    if directory.is_dir() {
        fs::remove_dir_all(&directory)?;

        println!("Theme removed: {id}");
        println!("Removed: {}", directory.display());
    } else if file.is_file() {
        fs::remove_file(&file)?;

        println!("Theme removed: {id}");
        println!("Removed: {}", file.display());
    } else {
        return Err(format!(
            "No user theme `{id}` in {}. Built-in themes cannot be removed.",
            user_root.display()
        )
        .into());
    }

    if read_active_theme(root).as_deref() == Some(id.as_str()) {
        println!(
            "Note: `{id}` is the active theme; run `my-linux-theme restore` to return to your original configuration."
        );
    }

    Ok(())
}

/// Extensions shipped with My Linux plus the user's own extensions.
fn extension_registry() -> ExtensionRegistry {
    ExtensionRegistry::new(
        desktop_core::paths::builtin_extensions_dir(),
        user_extensions_dir(),
    )
}

/// Enabled extensions rendered for `theme`, ready for a transaction.
fn enabled_extension_files(
    theme: &desktop_core::LoadedTheme,
) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let generated = extension_registry().render_enabled(theme)?;

    Ok(generated
        .into_iter()
        .map(|extension| (extension.destination, extension.content))
        .collect())
}

fn command_extensions(root: &Path, rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let registry = extension_registry();
    let subcommand = rest.first().map(String::as_str).unwrap_or("list");
    let arguments = rest.get(1..).unwrap_or(&[]);

    match subcommand {
        "list" => {
            let discovery = registry.list();

            if discovery.extensions.is_empty() {
                println!(
                    "No extensions found in {} or {}.",
                    registry.user_root().display(),
                    registry.builtin_root().display()
                );
            } else {
                println!("Extensions:");

                for summary in &discovery.extensions {
                    println!(
                        "  {} {:<14} {:<10} {:<8} {}",
                        if summary.enabled { "*" } else { " " },
                        summary.id,
                        summary.format,
                        summary.source.label(),
                        summary.description
                    );
                    println!("      writes: {}", summary.destination);

                    if !summary.programs.is_empty() {
                        println!("      programs: {}", summary.programs.join(", "));
                    }
                }
            }

            for broken in &discovery.broken {
                println!("Warning:  {}: {}", broken.path.display(), broken.error);
            }

            let enabled = registry.enabled()?;

            println!();
            println!(
                "Enabled: {}",
                if enabled.is_empty() {
                    "none (enable one with `extensions enable <id>`)".to_string()
                } else {
                    enabled.join(", ")
                }
            );
        }

        "info" => {
            let id = arguments
                .first()
                .ok_or("Usage: my-linux-theme extensions info <id>")?;
            let extension = registry.load(id)?;
            let metadata = &extension.metadata;

            println!("{}", metadata.name);
            println!("  id:          {}", metadata.id);
            println!("  version:     {}", metadata.version);
            println!("  schema:      {}", metadata.schema);
            println!("  description: {}", metadata.description);

            if let Some(author) = &metadata.author {
                println!("  author:      {author}");
            }

            println!("  format:      {}", metadata.format.label());
            println!("  destination: {}", metadata.destination);
            println!("  template:    {}", metadata.template);
            println!("  path:        {}", extension.directory.display());

            if !metadata.programs.is_empty() {
                println!("  programs:    {}", metadata.programs.join(", "));
            }

            if !metadata.permissions.is_empty() {
                println!("  permissions: {}", metadata.permissions.join(", "));
            }

            for command in &metadata.reload {
                println!("  reload:      {}", command.join(" "));
            }

            println!(
                "  enabled:     {}",
                if registry.enabled()?.contains(&metadata.id) {
                    "yes"
                } else {
                    "no"
                }
            );
        }

        "render" => {
            let id = arguments
                .first()
                .ok_or("Usage: my-linux-theme extensions render <id> [--theme <theme>]")?;
            let theme_id = match value_of(arguments, "--theme") {
                Some(theme) => theme.to_string(),
                None => read_active_theme(root)
                    .ok_or("No active theme; pass --theme <theme> to render an extension")?,
            };
            let theme = theme_registry().load(&theme_id)?;
            let generated = extension_registry().render(id, &theme)?;

            print!("{}", generated.content);
        }

        "create" => {
            let id = arguments
                .first()
                .ok_or("Usage: my-linux-theme extensions create <id> [--name <name>] [--destination <path>] [--format <fmt>]")?;
            let name = value_of(arguments, "--name")
                .map(str::to_string)
                .unwrap_or_else(|| display_name(id));
            let destination = value_of(arguments, "--destination")
                .map(str::to_string)
                .unwrap_or_else(|| format!("{id}/{id}.conf"));
            let format = match value_of(arguments, "--format") {
                Some(value) => ExtensionFormat::parse(value).ok_or_else(|| {
                    format!("Unknown format `{value}`: use toml, css, rasi, ini or text")
                })?,
                None => ExtensionFormat::Text,
            };

            let directory = registry.scaffold(id, &name, &destination, format)?;

            println!("Extension created: {id}");
            println!("Directory:        {}", directory.display());
            println!("Destination:      {destination}");
            println!("Format:           {}", format.label());
            println!();
            println!(
                "Edit {0}, then run `my-linux-theme extensions render {1} --theme cyberpunk`.",
                directory.join("extension.toml").display(),
                id
            );
        }

        "import" => {
            let source = arguments
                .first()
                .ok_or("Usage: my-linux-theme extensions import <path> [--as <id>]")?;
            let id = value_of(arguments, "--as");
            let directory = registry.import(source, id)?;

            println!("Extension installed: {}", directory.display());
        }

        "export" => {
            let id = arguments
                .first()
                .ok_or("Usage: my-linux-theme extensions export <id> [directory]")?;
            let destination = arguments.get(1).map(String::as_str).unwrap_or(".");
            let directory = registry.export(id, destination)?;

            println!("Extension exported: {}", directory.display());
        }

        "remove" => {
            let id = arguments
                .first()
                .ok_or("Usage: my-linux-theme extensions remove <id>")?;
            let directory = registry.remove(id)?;

            println!("Extension removed: {id}");
            println!("Removed: {}", directory.display());

            let active = read_active_theme(root);

            if active.is_some() {
                println!(
                    "Note: the generated files from the last apply remain; run `my-linux-theme restore` to remove them."
                );
            }
        }

        "enable" => {
            let id = arguments
                .first()
                .ok_or("Usage: my-linux-theme extensions enable <id>")?;
            let settings = registry.enable(id)?;

            println!("Extension enabled: {id}");
            println!("Settings:          {}", settings.display());
            println!("It is written on the next `my-linux-theme apply`.");
        }

        "disable" => {
            let id = arguments
                .first()
                .ok_or("Usage: my-linux-theme extensions disable <id>")?;
            let settings = registry.disable(id)?;

            println!("Extension disabled: {id}");
            println!("Settings:           {}", settings.display());
        }

        "status" => {
            let discovery = registry.list();
            let enabled = registry.enabled()?;

            println!("Extension roots:");
            println!("  user:    {}", registry.user_root().display());
            println!("  builtin: {}", registry.builtin_root().display());
            println!();
            println!("Enabled extensions: {}", enabled.len());

            for id in &enabled {
                match registry.load(id) {
                    Ok(extension) => {
                        let detected = integrations::detect(
                            &extension
                                .metadata
                                .programs
                                .iter()
                                .map(String::as_str)
                                .collect::<Vec<&str>>(),
                        );
                        let generated = destination(root, &extension.metadata.destination);
                        let state = if generated.is_file() {
                            "installed"
                        } else {
                            "not applied yet"
                        };
                        let detection = match detected {
                            Some(path) => format!("found ({})", path.display()),
                            None if extension.metadata.programs.is_empty() => {
                                "no detection program".to_string()
                            }
                            None => "not found".to_string(),
                        };

                        println!("  - {id}: {state}, {detection}");
                    }
                    Err(error) => println!("  - {id}: broken ({error})"),
                }
            }

            let available = discovery
                .extensions
                .iter()
                .filter(|summary| !summary.enabled)
                .count();

            println!();
            println!("Available but disabled: {available}");

            for broken in &discovery.broken {
                println!("Broken: {}: {}", broken.path.display(), broken.error);
            }
        }

        "apply" => command_apply(root, arguments)?,

        other => {
            return Err(format!("Unknown extensions command: {other}\n\n{HELP}").into());
        }
    }

    Ok(())
}

/// Index location from `--index` or `$MY_LINUX_REPOSITORY`.
fn repository_location(args: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(location) = value_of(args, "--index") {
        return Ok(location.to_string());
    }

    if let Some(location) = env::var_os("MY_LINUX_REPOSITORY") {
        return Ok(PathBuf::from(location).display().to_string());
    }

    Err("No repository index: pass --index <path|git-url> or set MY_LINUX_REPOSITORY".into())
}

fn command_repository(root: &Path, rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let subcommand = rest.first().map(String::as_str).unwrap_or("list");
    let arguments = rest.get(1..).unwrap_or(&[]);

    match subcommand {
        "list" => {
            let location = repository_location(arguments)?;
            let index = repository::load(&location, repository::cache_dir())?;

            println!("{}", index.name);

            if !index.description.is_empty() {
                println!("  {}", index.description);
            }

            println!("  index: {}", index.path.display());
            println!();

            if index.packages.is_empty() {
                println!("No packages in this index.");
            }

            for package in &index.packages {
                println!(
                    "  {:<16} {:<10} {:<9} {}",
                    package.id,
                    package.kind.label(),
                    package.version,
                    package.description
                );
            }
        }

        "search" => {
            let query = arguments
                .first()
                .ok_or("Usage: my-linux-theme repository search <query> [--index <location>]")?;
            let location = repository_location(arguments)?;
            let index = repository::load(&location, repository::cache_dir())?;
            let matches = index.search(query);

            println!(
                "{} package(s) matching `{query}` in {}:",
                matches.len(),
                index.name
            );

            for package in matches {
                println!(
                    "  {:<16} {:<10} {}",
                    package.id,
                    package.kind.label(),
                    package.description
                );
            }
        }

        "info" => {
            let id = arguments
                .first()
                .ok_or("Usage: my-linux-theme repository info <id> [--index <location>]")?;
            let location = repository_location(arguments)?;
            let index = repository::load(&location, repository::cache_dir())?;
            let package = index
                .find(id)
                .ok_or_else(|| format!("Unknown package `{id}` in {}", index.name))?;

            println!("{}", package.name);
            println!("  id:          {}", package.id);
            println!("  kind:        {}", package.kind.label());
            println!("  version:     {}", package.version);
            println!("  description: {}", package.description);

            if let Some(author) = &package.author {
                println!("  author:      {author}");
            }

            println!("  source:      {}", package.source.display());
            println!(
                "  available:   {}",
                if package.source.exists() {
                    "yes"
                } else {
                    "no (the content is missing)"
                }
            );
        }

        "install" => {
            let id = arguments.first().ok_or(
                "Usage: my-linux-theme repository install <id> [--index <location>] [--force]",
            )?;
            let location = repository_location(arguments)?;
            let index = repository::load(&location, repository::cache_dir())?;
            let package = index
                .find(id)
                .ok_or_else(|| format!("Unknown package `{id}` in {}", index.name))?;
            let path = repository::install(
                package,
                user_themes_dir(),
                &extension_registry(),
                has_flag(arguments, "--force"),
            )?;

            println!("Installed {} `{}`.", package.kind.label(), package.id);
            println!("Path: {}", path.display());

            if package.kind == repository::index::PackageKind::Theme {
                println!("Apply with: my-linux-theme apply {}", package.id);
            } else {
                println!(
                    "Enable with: my-linux-theme extensions enable {}",
                    package.id
                );
            }

            if !root.is_dir() {
                println!(
                    "Note: the installation root {} does not exist yet; run `apply` once.",
                    root.display()
                );
            }
        }

        "sync" => {
            let location = match arguments.first() {
                Some(location) => location.clone(),
                None => repository_location(arguments)?,
            };
            let path = repository::fetch(&location, repository::cache_dir())?;

            println!("Repository ready: {}", path.display());

            if path.is_dir() && !path.join(repository::INDEX_FILE).is_file() {
                println!("Warning: no {} in that repository.", repository::INDEX_FILE);
            }
        }

        other => {
            return Err(format!("Unknown repository command: {other}\n\n{HELP}").into());
        }
    }

    Ok(())
}

fn command_apply(root: &Path, rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let name = rest.first().ok_or(
        "Usage: my-linux-theme apply <theme> [--integration <id>]... [--reload] [--force]",
    )?;

    let mut selected_ids: Vec<String> = Vec::new();
    let mut reload = false;
    let mut force = false;
    let mut index = 1;

    while index < rest.len() {
        match rest[index].as_str() {
            "--integration" => {
                selected_ids.push(
                    rest.get(index + 1)
                        .ok_or("--integration requires an integration id")?
                        .clone(),
                );
                index += 2;
            }
            "--reload" => {
                reload = true;
                index += 1;
            }
            "--force" => {
                force = true;
                index += 1;
            }
            other => return Err(format!("Unknown option: {other}").into()),
        }
    }

    let selected = integrations::select(&selected_ids)?;
    let theme = registry().load(name)?;
    let enabled_extensions = extension_registry().render_enabled(&theme)?;
    let extra: Vec<(String, String)> = enabled_extensions
        .iter()
        .map(|extension| (extension.destination.clone(), extension.content.clone()))
        .collect();
    let had_previous_configuration = selected
        .iter()
        .any(|integration| destination(root, integration.relative_path()).exists());

    let report = apply_theme_with_extra_options(root, &theme, &selected, &extra, force)?;

    println!("Theme applied successfully.");
    println!("Theme:     {} ({})", theme.metadata.name, report.theme_id);
    println!("Root:      {}", root.display());

    for file in &report.installed_files {
        println!("Installed: {}", destination(root, file).display());
    }

    if enabled_extensions.is_empty() {
        println!("Extensions: none enabled");
    } else {
        println!(
            "Extensions: {}",
            enabled_extensions
                .iter()
                .map(|extension| extension.id.as_str())
                .collect::<Vec<&str>>()
                .join(", ")
        );
    }

    if had_previous_configuration {
        println!(
            "Backup:    {}",
            original_backup(root, WAYBAR_RELATIVE_PATH).display()
        );
    } else {
        println!("Backup:    none (there was no previous configuration)");
    }

    for backup in &report.created_backups {
        println!("Created:   {backup}");
    }

    if reload {
        for integration in &selected {
            match integrations::reload(*integration) {
                Ok(commands) if commands.is_empty() => {}
                Ok(commands) => {
                    for command in commands {
                        println!("Reloaded:  {} ({command})", integration.id());
                    }
                }
                Err(error) => println!("Warning:   {} did not reload: {error}", integration.id()),
            }
        }
    } else {
        println!("Reload:    skipped (pass --reload to reload running applications)");
    }

    Ok(())
}

fn command_integrations_list() {
    println!("Supported integrations:");

    for integration in integrations::all() {
        println!(
            "  - {:<10} {:<24} {}",
            integration.id(),
            integration.name(),
            integration.relative_path()
        );
    }
}

fn command_status(root: &Path) {
    let path = destination(root, WAYBAR_RELATIVE_PATH);
    let backup = original_backup(root, WAYBAR_RELATIVE_PATH);
    let active = read_active_theme(root).unwrap_or_else(|| "none".to_string());

    println!("My Linux status");
    println!("Root:             {}", root.display());
    println!("Active theme:     {}", active);
    println!(
        "Active profile:   {}",
        read_active_profile(root).unwrap_or_else(|| "none".to_string())
    );
    println!("Configuration:    {}", describe(&path));
    println!("Original backup:  {}", describe(&backup));

    let installed = installed_entries(root);

    if installed.is_empty() {
        println!("Installed files:  not recorded");
    } else {
        for modification in detect_modifications(root) {
            println!(
                "Installed files:  {} ({})",
                modification.relative_path,
                modification.state.label()
            );
        }
    }

    println!(
        "Integrations:     {} supported, {} configured",
        integrations::all().len(),
        installed.len()
    );

    let discovery = registry().list();
    let user_themes = discovery
        .themes
        .iter()
        .filter(|theme| theme.source == desktop_core::ThemeSource::User)
        .count();

    println!(
        "Themes:           {} available ({} user, {} built-in)",
        discovery.themes.len(),
        user_themes,
        discovery.themes.len() - user_themes
    );

    if let Some(event) = last_event(root) {
        println!(
            "Last action:      {} {} ({})",
            format_timestamp(event.timestamp),
            event.action,
            event.detail
        );
    }
}

fn command_current(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("My Linux current state");
    println!("Root:            {}", root.display());
    println!(
        "Active theme:    {}",
        read_active_theme(root).unwrap_or_else(|| "none".to_string())
    );
    println!(
        "Active profile:  {}",
        read_active_profile(root).unwrap_or_else(|| "none".to_string())
    );

    match WallpaperState::new(root).current() {
        Some(path) => println!(
            "Wallpaper:       {} ({})",
            path.display(),
            if path.is_file() { "present" } else { "missing" }
        ),
        None => println!("Wallpaper:       none recorded"),
    }

    let settings = read_settings(root);

    println!(
        "Font:            {} {}",
        settings.font.as_deref().unwrap_or("system default"),
        settings.font_size
    );
    println!(
        "Icon theme:      {}",
        settings.icon_theme.as_deref().unwrap_or("system default")
    );
    println!(
        "Cursor theme:    {} ({})",
        settings.cursor_theme.as_deref().unwrap_or("system default"),
        settings.cursor_size
    );

    let modifications = detect_modifications(root);
    let attention = modifications
        .iter()
        .filter(|modification| modification.state != FileState::Unchanged)
        .count();

    println!(
        "Installed files: {} ({} need attention)",
        modifications.len(),
        attention
    );

    match last_event(root) {
        Some(event) => println!(
            "Last action:     {} {} ({})",
            format_timestamp(event.timestamp),
            event.action,
            event.detail
        ),
        None => println!("Last action:     none recorded"),
    }

    Ok(())
}

fn command_logs(root: &Path, rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let limit = value_of(rest, "--limit")
        .map(|value| value.parse().unwrap_or(20))
        .unwrap_or(20);
    let events = read_events(root);

    if events.is_empty() {
        println!("No events recorded for {}.", root.display());
        return Ok(());
    }

    let start = events.len().saturating_sub(limit);

    for event in &events[start..] {
        println!(
            "{}  {:<5} {:<14} {}",
            format_timestamp(event.timestamp),
            event.level.label().to_uppercase(),
            event.action,
            event.detail
        );
    }

    Ok(())
}

fn command_integrations_status(root: &Path) {
    let modifications = detect_modifications(root);

    println!("Integration status:");

    for status in integrations::status() {
        let detected = match &status.detected {
            Some(path) => format!("yes ({})", path.display()),
            None => format!("no ({})", status.programs.join(", ")),
        };

        let configuration = match modifications
            .iter()
            .find(|modification| modification.relative_path == status.destination)
        {
            Some(modification) => {
                format!("{} ({})", status.destination, modification.state.label())
            }
            None => format!("not installed ({})", status.destination),
        };

        println!(
            "  - {:<10} detected: {:<34} config: {}",
            status.id, detected, configuration
        );
    }
}

fn command_backup_list(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let directory = root.join("backups");

    if !directory.is_dir() {
        println!("No backups found; {} does not exist.", directory.display());

        return Ok(());
    }

    let records = backup_records(root);

    if records.is_empty() {
        println!("No backups found in {}.", directory.display());

        return Ok(());
    }

    println!("Backups in {}:", directory.display());

    for record in records {
        let integrity = if !record.has_metadata {
            "legacy (no metadata)"
        } else if record.valid {
            "verified"
        } else {
            "CORRUPT"
        };

        println!(
            "  {:<32} {:>6} bytes  {}  theme {:<12} {:<20} digest {}",
            format!("{}.{}", record.relative_path, record.kind),
            record.size,
            format_timestamp(record.created),
            record.theme,
            integrity,
            record.digest
        );
    }

    Ok(())
}

fn command_backup_prune(
    root: &Path,
    keep_days: u64,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if dry_run {
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0)
            .saturating_sub(keep_days.saturating_mul(86_400));
        let records: Vec<_> = backup_records(root)
            .into_iter()
            .filter(|record| record.kind == "previous" && record.created <= cutoff)
            .collect();

        for record in records {
            println!("Would remove: {}", record.path.display());
        }

        println!(
            "Dry run: no snapshots older than {keep_days} day(s) were removed; original backups are always kept."
        );

        return Ok(());
    }

    let removed = prune_backups(root, keep_days)?;

    for path in &removed {
        println!("Removed: {}", path.display());
    }

    println!(
        "Pruned {} previous snapshot(s) older than {keep_days} day(s); original backups are kept.",
        removed.len()
    );

    Ok(())
}

fn command_history(root: &Path) {
    let history = read_history(root);

    if history.is_empty() {
        println!("No installation history recorded for {}.", root.display());

        return;
    }

    println!("Installation history for {}:", root.display());

    for entry in history {
        let files = if entry.files.is_empty() {
            String::new()
        } else {
            format!("  [{}]", entry.files.join(", "))
        };

        println!(
            "  {}  {:<8} {:<12}{}",
            format_timestamp(entry.timestamp),
            entry.action,
            entry.theme,
            files
        );
    }
}

fn command_doctor(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut diagnostics = diagnose(root, builtin_themes_dir(), Some(&profile_registry()));
    let mut problems = 0;

    diagnostics.extend(
        extension_registry()
            .diagnose(root)
            .into_iter()
            .map(|diagnostic| desktop_core::transaction::Diagnostic {
                label: diagnostic.label,
                detail: diagnostic.detail,
                ok: diagnostic.ok,
            }),
    );

    println!("My Linux doctor");
    println!("Root: {}", root.display());
    println!();

    for diagnostic in &diagnostics {
        if !diagnostic.ok {
            problems += 1;
        }

        println!(
            "  [{}] {}: {}",
            if diagnostic.ok { "ok" } else { "warn" },
            diagnostic.label,
            diagnostic.detail
        );
    }

    println!();
    println!(
        "Summary: {} checks, {} problem(s)",
        diagnostics.len(),
        problems
    );

    if problems > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn value_of<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|argument| argument == flag)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|argument| argument == flag)
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.0} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn sanitize_id(text: &str) -> String {
    let mut id = String::new();

    for character in text.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            id.push(character);
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }

    let id = id.trim_matches('-').to_string();

    if id.is_empty() {
        "wallpaper".to_string()
    } else {
        id
    }
}

fn resolve_wallpaper(
    library: &WallpaperLibrary,
    query: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = PathBuf::from(query);

    if path.is_file() {
        return Ok(path);
    }

    library
        .find(query)
        .map(|wallpaper| wallpaper.path)
        .ok_or_else(|| format!("Unknown wallpaper: {query}").into())
}

fn apply_wallpaper_to_desktop(path: &Path, set: bool) {
    if !set {
        println!("Desktop:    skipped (pass --no-set only when you set it yourself)");
        return;
    }

    match detect_tool() {
        Some(tool) => match set_wallpaper(tool, path) {
            Ok(commands) => {
                for command in commands {
                    println!("Applied:    {command}");
                }
            }
            Err(error) => println!("Warning:    wallpaper tool failed: {error}"),
        },
        None => {
            println!("Desktop:    unchanged (install swww or hyprpaper to switch automatically)")
        }
    }
}

fn install_prefix(rest: &[String]) -> PathBuf {
    value_of(rest, "--prefix")
        .map(PathBuf::from)
        .unwrap_or_else(default_prefix)
}

fn install_options(rest: &[String]) -> Result<InstallOptions, Box<dyn std::error::Error>> {
    let prefix = install_prefix(rest);
    let mut options = match value_of(rest, "--source") {
        Some(source) => InstallOptions::from_source(source, &prefix)?,
        None => InstallOptions::from_current_installation(&prefix)?,
    };

    options.force = has_flag(rest, "--force");
    options.dry_run = has_flag(rest, "--dry-run");

    Ok(options)
}

fn command_install(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let options = install_options(rest)?;
    let layout = InstallLayout::new(&options.prefix);
    let report = install(&options)?;
    let environment = detect_environment();

    println!(
        "My Linux {} {}",
        report.version,
        if report.dry_run {
            "(dry run)"
        } else {
            "installed"
        }
    );
    println!("Host:     {}", environment.summary());
    println!("Prefix:   {}", report.prefix.display());

    if let Some(previous) = &report.previous_version {
        println!("Previous: {previous}");
    }

    if !environment.arch_based {
        println!(
            "Warning:  this is not an Arch-based system; Arch packaging is not available here."
        );
    }

    println!();
    println!("Binaries:");

    for (name, _) in &options.binaries {
        println!("  {}", layout.bin(name).display());
    }

    println!("Desktop:  {}", layout.desktop_file().display());
    println!("Themes:   {}", layout.themes_dir().display());
    println!("Profiles: {}", layout.profiles_dir().display());
    println!("Files:    {}", report.files.len());

    for preserved in &report.preserved {
        println!("Preserved: {preserved} (modified since the last install)");
    }

    if report.dry_run {
        println!("\nDry run: nothing was written.");
    } else {
        println!(
            "\nStart the GUI:   {}",
            layout.bin("my-linux-gui").display()
        );
        println!(
            "Verify install:  my-linux-theme install-status --prefix {}",
            report.prefix.display()
        );
    }

    println!("\nRuntime dependencies:");

    for check in dependency_report() {
        println!(
            "  [{}] {}",
            if check.missing() { "missing" } else { "ok" },
            check.description()
        );
    }

    Ok(())
}

fn command_upgrade(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let options = install_options(rest)?;
    let report = upgrade(&options)?;
    let layout = InstallLayout::new(&options.prefix);

    println!(
        "My Linux upgraded to {} {}",
        report.version,
        if report.dry_run { "(dry run)" } else { "" }
    );

    if let Some(previous) = &report.previous_version {
        println!("From:     {previous}");
    }

    println!("Prefix:   {}", report.prefix.display());
    println!("Files:    {}", report.files.len());

    for preserved in &report.preserved {
        println!("Preserved: {preserved} (modified)");
    }

    println!("Desktop:  {}", layout.desktop_file().display());

    if report.dry_run {
        println!("\nDry run: nothing was written.");
    }

    Ok(())
}

fn command_uninstall(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let layout = InstallLayout::new(install_prefix(rest));
    let report = uninstall(&layout, has_flag(rest, "--dry-run"))?;

    println!(
        "My Linux {} {}",
        report.version,
        if report.dry_run {
            "(dry run)"
        } else {
            "uninstalled"
        }
    );
    println!("Removed:  {} program file(s)", report.removed.len());

    for path in &report.missing {
        println!("Missing:  {path} (already absent)");
    }

    println!(
        "Kept:     user themes, user profiles, installation state and original configuration backups"
    );

    if report.dry_run {
        println!("\nDry run: nothing was removed.");
    }

    Ok(())
}

fn command_install_status(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let prefix = install_prefix(rest);
    let layout = InstallLayout::new(&prefix);

    let Some(status) = install_status(&layout)? else {
        println!("My Linux is not installed under {}.", prefix.display());

        return Ok(());
    };

    println!("My Linux {}", status.manifest.version);
    println!("Prefix:       {}", prefix.display());
    println!("Platform:     {}", status.manifest.platform);
    println!(
        "Installed:    {} ({} file(s))",
        format_timestamp(status.manifest.installed_at),
        status.manifest.files.len()
    );
    println!(
        "Verification: {} ({} problem(s))",
        if status.verify.ok() {
            "ok"
        } else {
            "needs attention"
        },
        status.verify.problems().len()
    );

    for check in status.verify.problems() {
        println!("  [FAIL] {}: {}", check.label, check.detail);
    }

    if status.manifest.version != status.verify.current_version {
        println!(
            "Upgrade:      {} -> {} available",
            status.manifest.version, status.verify.current_version
        );
    }

    for preserved in &status.manifest.preserved {
        println!("Preserved:    {preserved} (modified)");
    }

    Ok(())
}

/// Re-renders the installed lock screen after a wallpaper change. A missing
/// theme or lock screen configuration is not an error; the wallpaper change
/// itself still succeeded.
fn sync_lock_screen(root: &Path) {
    let Some(id) = read_active_theme(root) else {
        return;
    };

    let theme = match registry().load(&id) {
        Ok(theme) => theme,
        Err(error) => {
            eprintln!("warning: could not refresh the lock screen: {error}");
            return;
        }
    };

    if let Err(error) = refresh_lock_screen_config(root, &theme) {
        eprintln!("warning: could not refresh the lock screen: {error}");
    }
}

fn command_wallpaper(root: &Path, rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let subcommand = rest.first().map(String::as_str).unwrap_or("list");
    let args = &rest[1..];

    match subcommand {
        "list" => {
            let library = WallpaperLibrary::from_environment();

            println!("Wallpaper library:");

            for directory in library.dirs() {
                println!("  {}", describe(directory));
            }

            let wallpapers = library.list();

            if wallpapers.is_empty() {
                println!("\nNo wallpapers found. Add PNG or JPEG files to a library directory.");
                return Ok(());
            }

            println!();

            for wallpaper in wallpapers {
                println!(
                    "  {:<24} {:>9}  {}",
                    wallpaper.name,
                    format_bytes(wallpaper.bytes),
                    wallpaper.path.display()
                );
            }
        }

        "preview" => {
            let query = args
                .first()
                .ok_or("Usage: my-linux-theme wallpaper preview <file|name> [--width <cells>]")?;
            let width = value_of(args, "--width")
                .map(|value| value.parse().unwrap_or(40))
                .unwrap_or(40);
            let path = resolve_wallpaper(&WallpaperLibrary::from_environment(), query)?;

            println!("{}", path.display());
            print!("{}", render_preview(&path, width)?);
        }

        "apply" => {
            let query = args
                .first()
                .ok_or("Usage: my-linux-theme wallpaper apply <file|name> [--no-set]")?;
            let library = WallpaperLibrary::from_environment();
            let source = resolve_wallpaper(&library, query)?;
            let installed = library.install(&source)?;

            WallpaperState::new(root).set(&installed.path, "apply")?;
            let _ = append_event(
                root,
                Level::Info,
                "wallpaper",
                &format!("path={}", installed.path.display()),
            );

            println!("Wallpaper:  {}", installed.path.display());

            if let Some(directory) = library.install_dir() {
                println!("Library:    {}", directory.display());
            }

            sync_lock_screen(root);
            apply_wallpaper_to_desktop(&installed.path, !has_flag(args, "--no-set"));
        }

        "current" => {
            let state = WallpaperState::new(root);

            match state.current() {
                Some(path) => {
                    println!("Current wallpaper: {}", path.display());
                    println!(
                        "Exists:            {}",
                        if path.is_file() { "yes" } else { "no" }
                    );
                }
                None => println!("Current wallpaper: none recorded"),
            }

            if let Some(previous) = state.previous() {
                println!("Previous:          {}", previous.display());
            }
        }

        "history" => {
            let entries = WallpaperState::new(root).history();

            if entries.is_empty() {
                println!("No wallpaper changes recorded.");
            }

            for entry in entries {
                println!(
                    "{}  {:<8} {}",
                    format_timestamp(entry.timestamp),
                    entry.action,
                    entry.path
                );
            }
        }

        "restore" => {
            let state = WallpaperState::new(root);
            let previous = state
                .previous()
                .ok_or("No previous wallpaper is recorded")?;

            if !previous.is_file() {
                return Err(format!(
                    "The previous wallpaper no longer exists: {}",
                    previous.display()
                )
                .into());
            }

            state.set(&previous, "restore")?;
            let _ = append_event(
                root,
                Level::Info,
                "wallpaper-restore",
                &format!("path={}", previous.display()),
            );

            println!("Wallpaper restored: {}", previous.display());
            sync_lock_screen(root);
            apply_wallpaper_to_desktop(&previous, true);
        }

        "palette" => {
            let query = args.first().ok_or(
                "Usage: my-linux-theme wallpaper palette <file|name> [--name <id>] [--save] [--apply]",
            )?;
            let path = resolve_wallpaper(&WallpaperLibrary::from_environment(), query)?;
            let proposal = extract_palette(&path)?;

            println!("Wallpaper: {}x{}", proposal.width, proposal.height);
            println!("Source:    {}", proposal.wallpaper.display());
            println!();
            println!("Dominant colours:");

            for (color, count) in proposal.dominant.iter().take(5) {
                color_block(&color.to_hex(), &format!("{count} px"));
            }

            println!();
            println!("Proposed palette:");

            for (label, color) in [
                ("Background", proposal.palette.background),
                ("Surface", proposal.palette.surface),
                ("Primary", proposal.palette.primary),
                ("Secondary", proposal.palette.secondary),
                ("Text", proposal.palette.text),
                ("Border", proposal.palette.border),
            ] {
                color_block(&color.to_hex(), label);
            }

            if !has_flag(args, "--save") && !has_flag(args, "--apply") {
                println!(
                    "\nNothing was saved. Pass --save to store this as a theme, or --apply to save and apply it."
                );
                return Ok(());
            }

            let id = value_of(args, "--name")
                .map(sanitize_id)
                .unwrap_or_else(|| {
                    sanitize_id(
                        path.file_stem()
                            .and_then(|name| name.to_str())
                            .unwrap_or("wallpaper"),
                    )
                });

            let theme = save_theme(
                user_themes_dir(),
                &id,
                &display_name(&id),
                &path,
                &proposal.palette,
            )?;

            println!(
                "\nSaved theme: {} ({})",
                theme.metadata.name,
                theme_path(&id).display()
            );

            if has_flag(args, "--apply") {
                let enabled_extensions = extension_registry().render_enabled(&theme)?;
                let extra: Vec<(String, String)> = enabled_extensions
                    .iter()
                    .map(|extension| (extension.destination.clone(), extension.content.clone()))
                    .collect();
                let report = apply_theme_with_extra(root, &theme, integrations::all(), &extra)?;

                println!(
                    "Applied:     {} theme, {} files",
                    report.theme_id,
                    report.installed_files.len()
                );

                for file in &report.installed_files {
                    println!("Installed:   {}", destination(root, file).display());
                }

                println!(
                    "Reload:      skipped (run `apply {} --reload` to reload running applications)",
                    report.theme_id
                );
            }
        }

        other => {
            return Err(format!("Unknown wallpaper command: {other}\n\n{HELP}").into());
        }
    }

    Ok(())
}

fn list_assets(kind: AssetKind, title: &str) {
    let assets = discover(kind);

    println!("{title}:");

    if assets.is_empty() {
        println!("  none found in the usual XDG and /usr/share directories");
        return;
    }

    for asset in assets {
        println!("  {:<28} {}", asset.name, asset.path.display());
    }
}

fn asset_kind_of(path: &Path) -> Result<AssetKind, Box<dyn std::error::Error>> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_lowercase();

    if ["ttf", "otf", "ttc", "woff", "woff2"].contains(&extension.as_str()) {
        return Ok(AssetKind::Font);
    }

    if path.is_dir() {
        if path.join("cursors").is_dir() {
            return Ok(AssetKind::Cursor);
        }

        if path.join("index.theme").is_file() {
            return Ok(AssetKind::Icon);
        }
    }

    Err("Pass --kind font, --kind icon or --kind cursor for this asset".into())
}

fn command_assets(root: &Path, rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let subcommand = rest.first().map(String::as_str).unwrap_or("status");
    let args = &rest[1..];

    match subcommand {
        "fonts" => list_assets(AssetKind::Font, "Fonts"),
        "icons" => list_assets(AssetKind::Icon, "Icon themes"),
        "cursors" => list_assets(AssetKind::Cursor, "Cursor themes"),

        "status" => {
            let settings = read_settings(root);

            println!("Asset settings ({})", root.display());
            println!(
                "Font:              {}",
                settings.font.as_deref().unwrap_or("system default")
            );
            println!("Font size:         {}", settings.font_size);
            println!(
                "Icon theme:        {}",
                settings.icon_theme.as_deref().unwrap_or("system default")
            );
            println!(
                "Cursor theme:      {}",
                settings.cursor_theme.as_deref().unwrap_or("system default")
            );
            println!("Cursor size:       {}", settings.cursor_size);
            println!(
                "Hyprland snippet:  {}",
                describe(&hyprland_assets_file(root))
            );
            println!();
            println!("Discovered on this system:");

            for kind in [AssetKind::Font, AssetKind::Icon, AssetKind::Cursor] {
                println!("  {:<8} {}", kind.label(), discover(kind).len());
            }

            let installed = installed_assets(root);

            println!();

            if installed.is_empty() {
                println!("Installed assets:  none");
            } else {
                for asset in installed {
                    println!("Installed:         {} {}", asset.kind.label(), asset.name);
                }
            }
        }

        "set" => {
            let mut settings = read_settings(root);

            if let Some(value) = value_of(args, "--font") {
                settings.font = Some(value.to_string());
            }

            if let Some(value) = value_of(args, "--font-size") {
                settings.font_size = value.parse().map_err(|_| "--font-size requires a number")?;
            }

            if let Some(value) = value_of(args, "--icon-theme") {
                settings.icon_theme = Some(value.to_string());
            }

            if let Some(value) = value_of(args, "--cursor-theme") {
                settings.cursor_theme = Some(value.to_string());
            }

            if let Some(value) = value_of(args, "--cursor-size") {
                settings.cursor_size = value
                    .parse()
                    .map_err(|_| "--cursor-size requires a number")?;
            }

            write_settings(root, &settings)?;
            let _ = append_event(
                root,
                Level::Info,
                "assets",
                &format!(
                    "font={} size={} icons={} cursor={} {}",
                    settings.font.as_deref().unwrap_or("default"),
                    settings.font_size,
                    settings.icon_theme.as_deref().unwrap_or("default"),
                    settings.cursor_theme.as_deref().unwrap_or("default"),
                    settings.cursor_size
                ),
            );

            println!("Asset settings written.");
            println!("Snippet: {}", hyprland_assets_file(root).display());

            if has_flag(args, "--reload") {
                match &settings.cursor_theme {
                    Some(theme) => match apply_cursor(theme, settings.cursor_size) {
                        Ok(command) => println!("Reloaded: {command}"),
                        Err(error) => println!("Warning:  {error}"),
                    },
                    None => println!("Reload:   skipped (no cursor theme is set)"),
                }
            }
        }

        "reset" => {
            write_settings(root, &AssetSettings::default())?;
            println!("Asset settings cleared.");
        }

        "install" => {
            let source = args
                .first()
                .ok_or("Usage: my-linux-theme assets install <path> [--kind <font|icon|cursor>]")?;
            let kind = match value_of(args, "--kind") {
                Some(kind) => {
                    AssetKind::parse(kind).ok_or("--kind must be font, icon or cursor")?
                }
                None => asset_kind_of(Path::new(source))?,
            };

            let installed = install_asset(root, kind, source)?;

            println!("Installed {}: {}", kind.label(), installed.display());
        }

        "installed" => {
            let installed = installed_assets(root);

            if installed.is_empty() {
                println!("No assets installed.");
            }

            for asset in installed {
                println!(
                    "  {:<8} {:<28} {}",
                    asset.kind.label(),
                    asset.name,
                    asset.path.display()
                );
            }
        }

        "remove" => {
            let name = args
                .first()
                .ok_or("Usage: my-linux-theme assets remove <name> [--kind <font|icon|cursor>]")?;

            let removed = match value_of(args, "--kind") {
                Some(kind) => {
                    let kind =
                        AssetKind::parse(kind).ok_or("--kind must be font, icon or cursor")?;

                    remove_asset(root, kind, name)?
                }
                None => {
                    let mut removed = None;

                    for kind in [AssetKind::Font, AssetKind::Icon, AssetKind::Cursor] {
                        if let Ok(path) = remove_asset(root, kind, name) {
                            removed = Some(path);
                            break;
                        }
                    }

                    removed.ok_or_else(|| format!("No asset named `{name}` is installed"))?
                }
            };

            println!("Removed: {}", removed.display());
        }

        other => {
            return Err(format!("Unknown assets command: {other}\n\n{HELP}").into());
        }
    }

    Ok(())
}

fn builtin_profiles_dir() -> PathBuf {
    desktop_core::paths::builtin_profiles_dir()
}

fn user_profiles_dir() -> PathBuf {
    if let Some(directory) = env::var_os("MY_LINUX_USER_PROFILES") {
        return PathBuf::from(directory);
    }

    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"));

    data_home.join("my-linux").join("profiles")
}

fn profile_registry() -> ProfileRegistry {
    ProfileRegistry::new(builtin_profiles_dir(), user_profiles_dir())
}

fn print_profile(profile: &Profile) {
    println!("{}", profile.metadata.name);
    println!("  id:           {}", profile.metadata.id);
    println!("  version:      {}", profile.metadata.version);
    println!("  schema:       {}", profile.metadata.schema);
    println!("  theme:        {}", profile.metadata.theme);
    println!(
        "  wallpaper:    {}",
        profile.metadata.wallpaper.as_deref().unwrap_or("none")
    );
    println!("  description:  {}", profile.metadata.description);
    println!("  path:         {}", profile.path.display());
    println!(
        "  integrations: {}",
        if profile.metadata.integrations.is_empty() {
            "all".to_string()
        } else {
            profile.metadata.integrations.join(", ")
        }
    );
    println!(
        "  dependencies: {}",
        if profile.metadata.dependencies.is_empty() {
            "none".to_string()
        } else {
            profile.metadata.dependencies.join(", ")
        }
    );
    println!();
    println!("Assets:");
    println!(
        "  font:        {}",
        profile.assets.font.as_deref().unwrap_or("system default")
    );
    println!("  font size:   {}", profile.assets.font_size);
    println!(
        "  icon theme:  {}",
        profile
            .assets
            .icon_theme
            .as_deref()
            .unwrap_or("system default")
    );
    println!(
        "  cursor:      {} ({})",
        profile
            .assets
            .cursor_theme
            .as_deref()
            .unwrap_or("system default"),
        profile.assets.cursor_size
    );
    println!();
    println!("Layout:");
    println!(
        "  gaps:        {} in / {} out",
        profile.layout.gaps_in, profile.layout.gaps_out
    );
    println!("  border:      {}", profile.layout.border_size);
    println!("  rounding:    {}", profile.layout.rounding);
    println!(
        "  opacity:     {:.2} active / {:.2} inactive",
        profile.layout.active_opacity, profile.layout.inactive_opacity
    );
    println!(
        "  blur:        {}",
        if profile.layout.blur { "on" } else { "off" }
    );
}

fn command_profile(root: &Path, rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let subcommand = rest.first().map(String::as_str).unwrap_or("list");
    let args = &rest[1..];
    let profiles = profile_registry();

    match subcommand {
        "list" => {
            let summaries = profiles.list();

            println!("Desktop profiles:");

            for summary in &summaries {
                println!(
                    "  - {:<22} {:<26} theme {:<12} {:<18} ({})",
                    summary.id,
                    summary.name,
                    summary.theme,
                    if summary.integrations == 0 {
                        "all integrations".to_string()
                    } else {
                        format!("{} integrations", summary.integrations)
                    },
                    summary.source.label()
                );
            }

            if summaries.is_empty() {
                println!("  none");
            }

            println!();
            println!("User profiles: {}", profiles.user_root().display());
        }

        "info" => {
            let id = args
                .first()
                .ok_or("Usage: my-linux-theme profile info <id>")?;
            let profile = profiles.load(id)?;

            print_profile(&profile);
        }

        "preview" => {
            let id = args
                .first()
                .ok_or("Usage: my-linux-theme profile preview <id>")?;
            let profile = profiles.load(id)?;

            print_profile(&profile);
            println!();
            println!("Theme colours ({}):", profile.metadata.theme);

            let theme = registry().load(&profile.metadata.theme)?;
            let colors = &theme.colors;

            color_block(&colors.background.to_hex(), "Background");
            color_block(&colors.surface.to_hex(), "Surface");
            color_block(&colors.primary.to_hex(), "Primary");
            color_block(&colors.secondary.to_hex(), "Secondary");
            color_block(&colors.text.to_hex(), "Text");
            color_block(&colors.border.to_hex(), "Border");
        }

        "create" => {
            let id = args.first().ok_or(
                "Usage: my-linux-theme profile create <id> [--name <name>] [--theme <theme>] [--from <profile>]",
            )?;
            let id = sanitize_id(id);

            if profiles.find(&id).is_some() {
                return Err(format!("Profile `{id}` already exists").into());
            }

            let name = value_of(args, "--name")
                .map(str::to_string)
                .unwrap_or_else(|| display_name(&id));

            let mut profile = match value_of(args, "--from") {
                Some(source) => {
                    let mut profile = profiles.load(source)?;

                    profile.metadata.id = id.clone();
                    profile.metadata.name = name.clone();
                    profile
                }
                None => {
                    let theme = value_of(args, "--theme").unwrap_or("cyberpunk");

                    registry().load(theme)?;
                    Profile::new(&id, &name, theme)?
                }
            };

            if let Some(theme) = value_of(args, "--theme") {
                registry().load(theme)?;
                profile.metadata.theme = theme.to_string();
            }

            profile.normalize();
            profile.validate()?;

            let file = profiles.save(&profile)?;

            println!(
                "Created profile {} ({})",
                profile.metadata.name,
                file.display()
            );
        }

        "set" => {
            let id = args
                .first()
                .ok_or("Usage: my-linux-theme profile set <id> <field> <value>")?;
            let field = args
                .get(1)
                .ok_or("Usage: my-linux-theme profile set <id> <field> <value>")?;
            let value = args
                .get(2)
                .ok_or("Usage: my-linux-theme profile set <id> <field> <value>")?;

            let mut profile = profiles.load(id)?;

            match field.as_str() {
                "theme" => {
                    registry().load(value)?;
                    profile.metadata.theme = value.to_lowercase();
                }
                "wallpaper" => {
                    profile.metadata.wallpaper = if value.trim().is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    };
                }
                "integrations" => {
                    profile.metadata.integrations = value
                        .split(',')
                        .map(|integration| integration.trim().to_string())
                        .filter(|integration| !integration.is_empty())
                        .collect();
                }
                "dependencies" => {
                    profile.metadata.dependencies = value
                        .split(',')
                        .map(|dependency| dependency.trim().to_string())
                        .filter(|dependency| !dependency.is_empty())
                        .collect();
                }
                "font" => {
                    profile.assets.font = if value.trim().is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    };
                }
                "font-size" => {
                    profile.assets.font_size =
                        value.parse().map_err(|_| "font-size requires a number")?;
                }
                "icon-theme" => {
                    profile.assets.icon_theme = if value.trim().is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    };
                }
                "cursor-theme" => {
                    profile.assets.cursor_theme = if value.trim().is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    };
                }
                "cursor-size" => {
                    profile.assets.cursor_size =
                        value.parse().map_err(|_| "cursor-size requires a number")?;
                }
                "gaps-in" => {
                    profile.layout.gaps_in =
                        value.parse().map_err(|_| "gaps-in requires a number")?;
                }
                "gaps-out" => {
                    profile.layout.gaps_out =
                        value.parse().map_err(|_| "gaps-out requires a number")?;
                }
                "border-size" => {
                    profile.layout.border_size =
                        value.parse().map_err(|_| "border-size requires a number")?;
                }
                "rounding" => {
                    profile.layout.rounding =
                        value.parse().map_err(|_| "rounding requires a number")?;
                }
                "active-opacity" => {
                    profile.layout.active_opacity = value
                        .parse()
                        .map_err(|_| "active-opacity requires a number between 0 and 1")?;
                }
                "inactive-opacity" => {
                    profile.layout.inactive_opacity = value
                        .parse()
                        .map_err(|_| "inactive-opacity requires a number between 0 and 1")?;
                }
                "blur" => {
                    profile.layout.blur = matches!(value.as_str(), "true" | "on" | "yes" | "1");
                }
                other => {
                    return Err(format!(
                        "Unknown profile field `{other}`. Fields: theme, wallpaper, integrations, dependencies, font, font-size, icon-theme, cursor-theme, cursor-size, gaps-in, gaps-out, border-size, rounding, active-opacity, inactive-opacity, blur"
                    )
                    .into());
                }
            }

            profile.normalize();
            profile.validate()?;

            let file = profiles.save(&profile)?;

            println!("Updated {} ({})", profile.metadata.name, file.display());
        }

        "duplicate" => {
            let from = args
                .first()
                .ok_or("Usage: my-linux-theme profile duplicate <from> <to>")?;
            let to = args
                .get(1)
                .ok_or("Usage: my-linux-theme profile duplicate <from> <to>")?;
            let to = sanitize_id(to);

            if profiles.find(&to).is_some() {
                return Err(format!("Profile `{to}` already exists").into());
            }

            let mut profile = profiles.load(from)?;

            profile.metadata.id = to.clone();
            profile.metadata.name = display_name(&to);

            let file = profiles.save(&profile)?;

            println!(
                "Duplicated {} as {} ({})",
                from,
                profile.metadata.name,
                file.display()
            );
        }

        "remove" => {
            let id = args
                .first()
                .ok_or("Usage: my-linux-theme profile remove <id>")?;
            let removed = profiles.remove(id)?;

            println!("Removed profile: {}", removed.display());
        }

        "import" => {
            let path = args
                .first()
                .ok_or("Usage: my-linux-theme profile import <path> [--as <id>]")?;
            let mut profile = Profile::from_path(path)?;

            if let Some(id) = value_of(args, "--as") {
                profile.metadata.id = sanitize_id(id);
                profile.normalize();
                profile.validate()?;
            }

            if profiles.find(&profile.metadata.id).is_some() {
                return Err(format!("Profile `{}` already exists", profile.metadata.id).into());
            }

            let file = profiles.save(&profile)?;

            println!(
                "Imported profile {} ({})",
                profile.metadata.name,
                file.display()
            );
        }

        "export" => {
            let id = args
                .first()
                .ok_or("Usage: my-linux-theme profile export <id> [file]")?;
            let profile = profiles.load(id)?;
            let rendered = profile.to_toml()?;

            match args.get(1) {
                Some(path) => {
                    fs::write(path, rendered)?;
                    println!("Exported {} to {path}", profile.metadata.name);
                }
                None => print!("{rendered}"),
            }
        }

        "apply" => {
            let id = args
                .first()
                .ok_or("Usage: my-linux-theme profile apply <id> [--reload] [--no-set]")?;
            let profile = profiles.load(id)?;
            let theme = registry().load(&profile.metadata.theme)?;
            let extensions = enabled_extension_files(&theme)?;

            let report = apply_profile_with_extensions(
                root,
                &profile,
                &theme,
                &ProfileApplyOptions {
                    set_wallpaper: !has_flag(args, "--no-set"),
                    reload: has_flag(args, "--reload"),
                },
                &extensions,
            )?;

            println!("Profile applied successfully.");
            println!(
                "Profile:   {} ({})",
                profile.metadata.name, report.profile_id
            );
            println!("Theme:     {} ({})", theme.metadata.name, report.theme_id);
            println!(
                "Layout:    {}",
                destination(root, &report.layout_file).display()
            );
            println!("Assets:    {}", report.asset_snippet.display());

            for file in &report.installed_files {
                println!("Installed: {}", destination(root, file).display());
            }

            match &report.wallpaper {
                Some(path) => println!("Wallpaper: {}", path.display()),
                None => println!("Wallpaper: unchanged"),
            }

            for command in &report.wallpaper_commands {
                println!("Applied:   {command}");
            }

            for command in &report.reloaded {
                println!("Reloaded:  {command}");
            }

            for warning in &report.warnings {
                println!("Warning:   {warning}");
            }

            if report.reloaded.is_empty() {
                println!("Reload:    skipped (pass --reload to reload running applications)");
            }
        }

        "restore" => {
            let id = read_previous_profile(root).ok_or("No previous profile is recorded")?;
            let profile = profiles.load(&id)?;
            let theme = registry().load(&profile.metadata.theme)?;
            let extensions = enabled_extension_files(&theme)?;

            let report = apply_profile_with_extensions(
                root,
                &profile,
                &theme,
                &ProfileApplyOptions {
                    set_wallpaper: true,
                    reload: false,
                },
                &extensions,
            )?;

            println!(
                "Restored profile {} ({})",
                profile.metadata.name, report.profile_id
            );

            for file in &report.installed_files {
                println!("Installed: {}", destination(root, file).display());
            }

            for warning in &report.warnings {
                println!("Warning:   {warning}");
            }
        }

        "current" => {
            println!(
                "Active profile: {}",
                read_active_profile(root).unwrap_or_else(|| "none".to_string())
            );

            if let Some(previous) = read_previous_profile(root) {
                println!("Previous:       {previous}");
            }

            println!(
                "Active theme:   {}",
                read_active_theme(root).unwrap_or_else(|| "none".to_string())
            );
        }

        other => {
            return Err(format!("Unknown profile command: {other}\n\n{HELP}").into());
        }
    }

    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (root_flag, args) = parse_args(env::args().skip(1).collect())?;

    if args.is_empty() || matches!(args[0].as_str(), "help" | "-h" | "--help") {
        print!("{}", HELP);
        return Ok(());
    }

    let root = root_flag.unwrap_or_else(default_root);
    let command = args[0].as_str();
    let rest = &args[1..];

    match command {
        "list" => command_list(),

        "info" => {
            let name = rest.first().ok_or("Usage: my-linux-theme info <theme>")?;

            let theme = registry().load(name)?;

            println!("{}", theme.metadata.name);
            println!("  id:          {}", theme.metadata.id);
            println!("  version:     {}", theme.metadata.version);
            println!("  schema:      {}", theme.metadata.schema);
            println!("  description: {}", theme.metadata.description);

            if let Some(author) = &theme.metadata.author {
                println!("  author:      {author}");
            }

            if let Some(parent) = &theme.metadata.extends {
                println!("  extends:     {parent}");
            }

            println!("  path:        {}", theme.path.display());

            if !theme.overrides.is_empty() {
                println!("  overrides:");

                for (integration, over) in &theme.overrides {
                    let mut values = Vec::new();

                    if let Some(family) = &over.font_family {
                        values.push(format!("font_family={family}"));
                    }

                    if let Some(family) = &over.mono_family {
                        values.push(format!("mono_family={family}"));
                    }

                    if let Some(size) = over.font_size {
                        values.push(format!("font_size={size}"));
                    }

                    if let Some(radius) = over.radius {
                        values.push(format!("radius={radius}"));
                    }

                    if let Some(spacing) = over.spacing {
                        values.push(format!("spacing={spacing}"));
                    }

                    if let Some(border) = over.border_width {
                        values.push(format!("border_width={border}"));
                    }

                    if let Some(opacity) = over.opacity {
                        values.push(format!("opacity={opacity}"));
                    }

                    if let Some(range) = over.shadow_range {
                        values.push(format!("shadow_range={range}"));
                    }

                    if let Some(opacity) = over.shadow_opacity {
                        values.push(format!("shadow_opacity={opacity}"));
                    }

                    println!("    {integration}: {}", values.join(" "));
                }
            }

            println!();
            println!("Palette:");

            let colors = &theme.colors;

            for (label, color) in [
                ("background", colors.background),
                ("surface", colors.surface),
                ("surface_alt", colors.surface_alt),
                ("primary", colors.primary),
                ("secondary", colors.secondary),
                ("text", colors.text),
                ("text_muted", colors.text_muted),
                ("border", colors.border),
                ("success", colors.success),
                ("warning", colors.warning),
                ("error", colors.error),
                ("active", colors.active),
                ("hover", colors.hover),
                ("selected", colors.selected),
            ] {
                println!("  {:<12} {}", label, color.to_hex());
            }

            let typography = &theme.typography;
            let metrics = theme.metrics;

            println!();
            println!("Typography:");
            println!("  interface    {}", typography.font_family);
            println!("  monospace    {}", typography.mono_family);
            println!("  size         {} px", typography.font_size);
            println!();
            println!("Metrics:");
            println!("  radius       {} px", metrics.radius);
            println!("  spacing      {} px", metrics.spacing);
            println!("  border       {} px", metrics.border_width);
            println!("  opacity      {} %", metrics.opacity);
            println!("  shadow range {} px", metrics.shadow_range);
            println!("  shadow       {} %", metrics.shadow_opacity);
        }

        "render" => {
            let name = rest
                .first()
                .ok_or("Usage: my-linux-theme render <theme> [--integration <id>]")?;

            let theme = registry().load(name)?;

            let integration_id = match rest.get(1) {
                None => "waybar",
                Some(option) if option == "--integration" => rest
                    .get(2)
                    .map(String::as_str)
                    .ok_or("--integration requires an integration id")?,
                Some(other) => return Err(format!("Unknown option: {other}").into()),
            };

            let integration = integrations::find(integration_id)
                .ok_or_else(|| format!("Unknown integration `{integration_id}`"))?;

            let content = integration.render(&theme);

            integration.validate(&content).map_err(|error| {
                format!(
                    "Generated {} configuration is invalid: {error}",
                    integration.id()
                )
            })?;

            println!("{content}");
        }

        "preview" => {
            let name = rest
                .first()
                .ok_or("Usage: my-linux-theme preview <theme>")?;

            let theme = registry().load(name)?;

            println!();
            println!("================================");
            println!("       MY LINUX THEME");
            println!("================================");
            println!("Theme: {}", theme.metadata.name);
            println!("About: {}", theme.metadata.description);
            println!();
            println!("Color palette:");

            let colors = &theme.colors;

            color_block(&colors.background.to_hex(), "Background");
            color_block(&colors.surface.to_hex(), "Surface");
            color_block(&colors.primary.to_hex(), "Primary");
            color_block(&colors.secondary.to_hex(), "Secondary");
            color_block(&colors.text.to_hex(), "Text");
            color_block(&colors.border.to_hex(), "Border");

            println!();
            println!("================================");
        }

        "create" => command_create(rest)?,

        "bump" => command_bump(rest)?,

        "duplicate" => command_duplicate(rest)?,

        "import" => command_import(rest)?,

        "export" => command_export(rest)?,

        "remove" => command_remove(&root, rest)?,

        "apply" => command_apply(&root, rest)?,

        "restore" => {
            let restored = restore_theme(&root)?;

            println!("Original configuration restored.");
            println!("Root: {}", root.display());

            for file in &restored {
                println!("Restored: {}", destination(&root, file).display());
            }

            let backup = original_backup(&root, WAYBAR_RELATIVE_PATH);

            if backup.exists() {
                println!("Backup kept at: {}", backup.display());
            } else {
                println!("Backup:        none (no original configuration was replaced)");
            }
        }

        "rollback" => {
            let restored = rollback_theme(&root)?;
            let active = read_active_theme(&root).unwrap_or_else(|| "none".to_string());

            println!("Rolled back to the previous configuration.");
            println!("Active theme: {}", active);

            for file in &restored {
                println!("Restored: {}", destination(&root, file).display());
            }
        }

        "status" => command_status(&root),

        "current" => command_current(&root)?,

        "logs" => command_logs(&root, rest)?,

        "extensions" => command_extensions(&root, rest)?,

        "repository" => command_repository(&root, rest)?,

        "integrations" => {
            let subcommand = rest.first().map(String::as_str).unwrap_or("list");

            match subcommand {
                "list" => command_integrations_list(),
                "status" => command_integrations_status(&root),
                other => {
                    return Err(format!("Unknown integrations command: {other}\n\n{HELP}").into());
                }
            }
        }

        "backup" => {
            let subcommand = rest.first().map(String::as_str).unwrap_or("list");

            match subcommand {
                "list" => command_backup_list(&root)?,
                "prune" => {
                    let mut keep_days = 30;
                    let mut dry_run = false;
                    let mut index = 1;

                    while index < rest.len() {
                        match rest[index].as_str() {
                            "--keep" => {
                                keep_days = rest
                                    .get(index + 1)
                                    .ok_or("--keep requires a number of days")?
                                    .parse()
                                    .map_err(|_| "--keep requires a number of days")?;
                                index += 2;
                            }
                            "--dry-run" => {
                                dry_run = true;
                                index += 1;
                            }
                            other => {
                                return Err(format!("Unknown backup prune option: {other}").into());
                            }
                        }
                    }

                    command_backup_prune(&root, keep_days, dry_run)?;
                }
                _ => {
                    return Err(format!("Unknown backup command: {subcommand}\n\n{HELP}").into());
                }
            }
        }

        "history" => command_history(&root),

        "doctor" => command_doctor(&root)?,

        "install" => command_install(rest)?,

        "upgrade" => command_upgrade(rest)?,

        "uninstall" => command_uninstall(rest)?,

        "install-status" => command_install_status(rest)?,

        "wallpaper" => command_wallpaper(&root, rest)?,

        "assets" => command_assets(&root, rest)?,

        "profile" => command_profile(&root, rest)?,

        other => {
            return Err(format!("Unknown command: {other}\n\n{HELP}").into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_theme_ids_as_names() {
        assert_eq!(display_name("my-theme"), "My Theme");
        assert_eq!(display_name("neon_horizon"), "Neon Horizon");
        assert_eq!(display_name("minimal"), "Minimal");
    }

    #[test]
    fn user_themes_are_written_under_the_user_root() {
        let path = theme_path("my-theme");

        assert!(path.ends_with("my-theme/colors/palette.toml"));
    }
}
