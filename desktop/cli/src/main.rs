use desktop_core::{
    Color, Registry, ThemeMetadata,
    config_manager::install_content,
    generate_palette, integrations, read_active_theme, render_from_parts, render_theme_toml,
    transaction::{
        WAYBAR_RELATIVE_PATH, apply_theme_with, destination, detect_modifications, diagnose,
        format_timestamp, installed_entries, original_backup, read_history, restore_theme,
        rollback_theme,
    },
    validate_id,
};

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
  import <path> [--as <id>]
                           Import a theme directory or .toml file
  export <theme> [file]    Export a theme as a portable .toml file
  remove <id>              Remove a user theme

Desktop commands:
  apply <theme> [--integration <id>]... [--reload]
                           Apply a theme as a single transaction
  restore                  Restore the original configuration
  rollback                 Undo the most recent apply
  status                   Show the active theme and installation state

Integration commands:
  integrations list        List supported desktop integrations
  integrations status      Show which integrations are installed

Recovery commands:
  backup list              List configuration backups
  history                  Show the installation history
  doctor                   Diagnose the installation

Options:
  --root <directory>       Directory that receives generated configurations
                           (default: $MY_LINUX_ROOT, otherwise target/theme-test)
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
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes")
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

    let colors = match (from, base) {
        (Some(parent), _) => registry.load(&parent)?.colors,
        (None, Some(base)) => generate_palette(base),
        (None, None) => generate_palette(Color::new(0x6C, 0x8C, 0xFF)),
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

    install_content(render_from_parts(&metadata, &colors).as_bytes(), &path)?;

    println!("Theme created.");
    println!("Id:   {id}");
    println!("File: {}", path.display());
    println!("Next: my-linux-theme preview {id}");

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

    let mut metadata = theme.metadata.clone();
    metadata.id = id.clone();

    let path = theme_path(&id);

    install_content(
        render_from_parts(&metadata, &theme.colors).as_bytes(),
        &path,
    )?;

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

fn command_apply(root: &Path, rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let name = rest
        .first()
        .ok_or("Usage: my-linux-theme apply <theme> [--integration <id>]... [--reload]")?;

    let mut selected_ids: Vec<String> = Vec::new();
    let mut reload = false;
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
            other => return Err(format!("Unknown option: {other}").into()),
        }
    }

    let selected = integrations::select(&selected_ids)?;
    let theme = registry().load(name)?;
    let had_previous_configuration = selected
        .iter()
        .any(|integration| destination(root, integration.relative_path()).exists());

    let report = apply_theme_with(root, &theme, &selected)?;

    println!("Theme applied successfully.");
    println!("Theme:     {} ({})", theme.metadata.name, report.theme_id);
    println!("Root:      {}", root.display());

    for file in &report.installed_files {
        println!("Installed: {}", destination(root, file).display());
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

    println!("Integrations:     Waybar (CSS)");
}

fn command_backup_list(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let directory = root.join("backups");

    if !directory.is_dir() {
        println!("No backups found; {} does not exist.", directory.display());

        return Ok(());
    }

    let mut entries: Vec<PathBuf> = fs::read_dir(&directory)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();

    entries.sort();

    if entries.is_empty() {
        println!("No backups found in {}.", directory.display());

        return Ok(());
    }

    println!("Backups in {}:", directory.display());

    for entry in entries {
        let name = entry
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();

        let size = fs::metadata(&entry)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let digest = desktop_core::config_manager::digest_file(&entry)
            .unwrap_or_else(|_| "unreadable".to_string());

        println!("  {:<28} {:>6} bytes  digest {}", name, size, digest);
    }

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
    let diagnostics = diagnose(root, builtin_themes_dir());
    let mut problems = 0;

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

        "integrations" => {
            let subcommand = rest.first().map(String::as_str).unwrap_or("list");

            match subcommand {
                "list" => command_integrations_list(),
                "status" => command_status(&root),
                other => {
                    return Err(format!("Unknown integrations command: {other}\n\n{HELP}").into());
                }
            }
        }

        "backup" => {
            let subcommand = rest.first().map(String::as_str).unwrap_or("list");

            if subcommand != "list" {
                return Err(format!("Unknown backup command: {subcommand}\n\n{HELP}").into());
            }

            command_backup_list(&root)?;
        }

        "history" => command_history(&root),

        "doctor" => command_doctor(&root)?,

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
