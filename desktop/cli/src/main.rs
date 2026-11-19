use desktop_core::{
    Theme, clear_active_theme, install_waybar_theme, load_desktop_theme, read_active_theme,
    render::render_waybar_css, restore_waybar_theme, waybar_paths, write_active_theme,
};
use std::env;
use std::path::{Path, PathBuf};

const HELP: &str = "\
My Linux Theme Manager

Usage:
  my-linux-theme [--root <directory>] <command> [arguments]

Commands:
  list                List available themes
  info <theme>        Show theme metadata and the full palette
  preview <theme>     Preview theme colours with terminal swatches
  render <theme>      Print the generated Waybar CSS
  export <theme> [file]
                      Write the generated Waybar CSS to a file
                      (default: generated/<theme>.css)
  apply <theme>       Install the theme into the target root
  restore             Restore the original configuration from its backup
  status              Show the active theme and installation state

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

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (root_flag, args) = parse_args(env::args().skip(1).collect())?;

    if args.is_empty() || matches!(args[0].as_str(), "help" | "-h" | "--help") {
        print!("{}", HELP);
        return Ok(());
    }

    let root = root_flag.unwrap_or_else(default_root);
    let themes = themes_dir();
    let command = args[0].as_str();

    match command {
        "list" => {
            println!("Available themes:");

            for theme in Theme::all() {
                println!("  - {}", theme.name().to_lowercase());
            }
        }

        "info" => {
            let name = args.get(1).ok_or("Usage: my-linux-theme info <theme>")?;

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
            let name = args.get(1).ok_or("Usage: my-linux-theme render <theme>")?;

            let theme = load_desktop_theme(&themes, name)?;

            println!("{}", render_waybar_css(&theme));
        }

        "preview" => {
            let name = args.get(1).ok_or("Usage: my-linux-theme preview <theme>")?;

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

        "apply" => {
            let name = args.get(1).ok_or("Usage: my-linux-theme apply <theme>")?;

            let theme = load_desktop_theme(&themes, name)?;
            let paths = waybar_paths(&root);
            let had_previous_configuration = paths.destination.exists();

            install_waybar_theme(&theme, &paths.destination, &paths.backup)?;
            write_active_theme(&root, &theme.metadata.id)?;

            println!("Theme applied successfully.");
            println!("Theme:     {} ({})", theme.metadata.name, theme.metadata.id);
            println!("Root:      {}", root.display());
            println!("Installed: {}", paths.destination.display());

            if had_previous_configuration {
                println!("Backup:    {}", paths.backup.display());
            } else {
                println!("Backup:    none (there was no previous configuration)");
            }
        }

        "restore" => {
            let paths = waybar_paths(&root);

            restore_waybar_theme(&paths.destination, &paths.backup)?;
            clear_active_theme(&root)?;

            println!("Original configuration restored.");
            println!("Restored: {}", paths.destination.display());
            println!("Backup kept at: {}", paths.backup.display());
        }

        "status" => {
            let paths = waybar_paths(&root);
            let active = read_active_theme(&root).unwrap_or_else(|| "none".to_string());

            println!("My Linux status");
            println!("Root:             {}", root.display());
            println!("Active theme:     {}", active);
            println!("Configuration:    {}", describe(&paths.destination));
            println!("Original backup:  {}", describe(&paths.backup));
            println!("Integrations:     Waybar (CSS)");
        }

        "export" => {
            let name = args
                .get(1)
                .ok_or("Usage: my-linux-theme export <theme> [file]")?;

            let theme = load_desktop_theme(&themes, name)?;
            let css = render_waybar_css(&theme);

            let output_file = match args.get(2) {
                Some(path) => PathBuf::from(path),
                None => PathBuf::from("generated").join(format!("{}.css", name)),
            };

            if let Some(parent) = output_file.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(&output_file, css)?;

            println!("Theme exported successfully.");
            println!("File: {}", output_file.display());
        }

        other => {
            return Err(format!("Unknown command: {other}\n\n{HELP}").into());
        }
    }

    Ok(())
}
