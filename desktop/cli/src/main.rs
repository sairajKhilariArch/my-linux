use desktop_core::{
    Theme, load_desktop_theme, read_active_theme,
    render::render_waybar_css,
    transaction::{
        WAYBAR_RELATIVE_PATH, apply_theme, destination, detect_modifications, diagnose,
        format_timestamp, installed_entries, original_backup, read_history, restore_theme,
        rollback_theme,
    },
};

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const HELP: &str = "\
My Linux Theme Manager

Usage:
  my-linux-theme [--root <directory>] <command> [arguments]

Theme commands:
  list                List available themes
  info <theme>        Show theme metadata and the full palette
  preview <theme>     Preview theme colours with terminal swatches
  render <theme>      Print the generated Waybar CSS
  export <theme> [file]
                      Write the generated Waybar CSS to a file
                      (default: generated/<theme>.css)

Desktop commands:
  apply <theme>       Apply a theme as a single transaction
  restore             Restore the original configuration
  rollback            Undo the most recent apply
  status              Show the active theme and installation state

Recovery commands:
  backup list         List configuration backups
  history             Show the installation history
  doctor              Diagnose the installation

Options:
  --root <directory>  Directory that receives generated configurations
                      (default: $MY_LINUX_ROOT, otherwise target/theme-test)
  -h, --help          Show this help
";

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {}", error);
        std::process::exit(1);
    }
}

/// Themes are read from the source tree while the project is in development.
fn themes_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes")
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

fn command_apply(
    root: &Path,
    themes: &Path,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let name = args.first().ok_or("Usage: my-linux-theme apply <theme>")?;

    let theme = load_desktop_theme(themes, name)?;
    let path = destination(root, WAYBAR_RELATIVE_PATH);
    let had_previous_configuration = path.exists();

    let report = apply_theme(root, &theme)?;

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

    Ok(())
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

fn command_doctor(root: &Path, themes: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = diagnose(root, themes);
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
    let themes = themes_dir();
    let command = args[0].as_str();
    let rest = &args[1..];

    match command {
        "list" => {
            println!("Available themes:");

            for theme in Theme::all() {
                println!("  - {}", theme.name().to_lowercase());
            }
        }

        "info" => {
            let name = rest.first().ok_or("Usage: my-linux-theme info <theme>")?;

            let theme = load_desktop_theme(&themes, name)?;

            println!("{}", theme.metadata.name);
            println!("  id:          {}", theme.metadata.id);
            println!("  version:     {}", theme.metadata.version);
            println!("  description: {}", theme.metadata.description);
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
            let name = rest.first().ok_or("Usage: my-linux-theme render <theme>")?;

            let theme = load_desktop_theme(&themes, name)?;

            println!("{}", render_waybar_css(&theme));
        }

        "preview" => {
            let name = rest
                .first()
                .ok_or("Usage: my-linux-theme preview <theme>")?;

            let theme = load_desktop_theme(&themes, name)?;

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

        "apply" => command_apply(&root, &themes, rest)?,

        "restore" => {
            let restored = restore_theme(&root)?;

            println!("Original configuration restored.");
            println!("Root: {}", root.display());

            for file in &restored {
                println!("Restored: {}", destination(&root, file).display());
            }

            println!(
                "Backup kept at: {}",
                original_backup(&root, WAYBAR_RELATIVE_PATH).display()
            );
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

        "backup" => {
            let subcommand = rest.first().map(String::as_str).unwrap_or("list");

            if subcommand != "list" {
                return Err(format!("Unknown backup command: {subcommand}\n\n{HELP}").into());
            }

            command_backup_list(&root)?;
        }

        "history" => command_history(&root),

        "doctor" => command_doctor(&root, &themes)?,

        "export" => {
            let name = rest
                .first()
                .ok_or("Usage: my-linux-theme export <theme> [file]")?;

            let theme = load_desktop_theme(&themes, name)?;
            let css = render_waybar_css(&theme);

            let output_file = match rest.get(1) {
                Some(path) => PathBuf::from(path),
                None => PathBuf::from("generated").join(format!("{}.css", name)),
            };

            if let Some(parent) = output_file.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(&output_file, css)?;

            println!("Theme exported successfully.");
            println!("File: {}", output_file.display());
        }

        other => {
            return Err(format!("Unknown command: {other}\n\n{HELP}").into());
        }
    }

    Ok(())
}
