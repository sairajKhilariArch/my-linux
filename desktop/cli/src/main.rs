use desktop_core::{install_waybar_theme, load_desktop_theme, render::render_waybar_css};
use std::env;
use std::path::PathBuf;

fn color_block(hex: &str, label: &str) {
    let hex = hex.trim_start_matches('#');

    if hex.len() != 6 {
        return;
    }

    let red = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let green = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let blue = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);

    print!(
        "\x1b[48;2;{};{};{}m   \x1b[0m {} ({})",
        red, green, blue, label, hex
    );

    println!();
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {}", error);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let themes_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes");

    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("list") => {
            println!("Available themes:");

            for theme in ["futuristic", "minimal", "cyberpunk", "nature"] {
                println!("  - {}", theme);
            }
        }

        Some("render") => {
            let name = args.get(1).ok_or("Usage: my-linux-theme render <theme>")?;

            let theme = load_desktop_theme(&themes_dir, name)?;
            let css = render_waybar_css(&theme);

            println!("{}", css);
        }

        Some("preview") => {
            let name = args.get(1).ok_or("Usage: my-linux-theme preview <theme>")?;

            let theme = load_desktop_theme(&themes_dir, name)?;

            println!();
            println!("================================");
            println!("       MY LINUX THEME");
            println!("================================");

            println!("Theme: {}", theme.metadata.name);
            println!();
            println!("Color palette:");
            color_block(&theme.colors.background.to_hex(), "Background");
            color_block(&theme.colors.surface.to_hex(), "Surface");
            color_block(&theme.colors.primary.to_hex(), "Primary");
            color_block(&theme.colors.secondary.to_hex(), "Secondary");
            color_block(&theme.colors.text.to_hex(), "Text");
            color_block(&theme.colors.border.to_hex(), "Border");

            println!();
            println!("================================");
            println!("CSS generation: Ready");
            println!("================================");
        }

        Some("apply") => {
            let name = args.get(1).ok_or("Usage: my-linux-theme apply <theme>")?;

            let theme = load_desktop_theme(&themes_dir, name)?;

            // Keep installation inside the project's test directory.
            let project_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let test_dir = project_dir.join("target/theme-test");

            std::fs::create_dir_all(&test_dir)?;

            let destination = test_dir.join("waybar.css");
            let backup = test_dir.join("backups/waybar.css.original");

            // Create a fake original configuration for backup testing.
            if !destination.exists() {
                std::fs::write(&destination, "/* Original test Waybar configuration */\n")?;
            }

            install_waybar_theme(&theme, &destination, &backup)?;

            println!("Theme applied successfully!");
            println!("Theme: {}", theme.metadata.name);
            println!("Installed: {}", destination.display());
            println!("Backup: {}", backup.display());
        }

        Some("export") => {
            let name = args.get(1).ok_or("Usage: my-linux-theme export <theme>")?;

            let theme = load_desktop_theme(&themes_dir, name)?;
            let css = render_waybar_css(&theme);

            let output_dir = PathBuf::from("generated");
            std::fs::create_dir_all(&output_dir)?;

            let output_file = output_dir.join(format!("{}.css", name));

            std::fs::write(&output_file, css)?;

            println!("Theme exported successfully!");
            println!("File: {}", output_file.display());
        }

        _ => {
            println!("My Linux Theme Manager");
            println!();
            println!("Commands:");
            println!("  list              List available themes");
            println!("  preview <theme>   Preview theme colors");
            println!("  render <theme>    Preview generated Waybar CSS");
            println!("  export <theme>    Export generated CSS to a file");
            println!("  apply <theme>     Apply theme to test configuration");
        }
    }

    Ok(())
}
