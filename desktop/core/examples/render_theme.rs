use desktop_core::load_desktop_theme;
use desktop_core::render::render_waybar_css;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme_name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "cyberpunk".to_string());

    let theme = load_desktop_theme("../../themes", &theme_name)?;
    let css = render_waybar_css(&theme);

    let output = format!("../../themes/{}/generated/waybar.css", theme_name);

    fs::create_dir_all(format!("../../themes/{}/generated", theme_name))?;
    fs::write(&output, css)?;

    println!("Generated Waybar theme: {}", output);

    Ok(())
}
