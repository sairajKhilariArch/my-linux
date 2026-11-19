//! Serialises a theme back to the TOML format understood by the loader.

use crate::loader::{LoadedTheme, ThemeColors, ThemeMetadata, ThemeMetrics, ThemeTypography};

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Renders a theme definition, including typography and metrics, that can be
/// re-imported without changes.
pub fn render_theme_toml(theme: &LoadedTheme) -> String {
    render_theme(theme)
}

/// Renders the complete theme definition.
pub fn render_theme(theme: &LoadedTheme) -> String {
    render_theme_parts(
        &theme.metadata,
        &theme.colors,
        &theme.typography,
        theme.metrics,
    )
}

/// Renders a complete theme definition from explicit parts.
pub fn render_theme_parts(
    metadata: &ThemeMetadata,
    colors: &ThemeColors,
    typography: &ThemeTypography,
    metrics: ThemeMetrics,
) -> String {
    let mut output = render_colors(metadata, colors);

    output.push_str("\n[typography]\n");
    output.push_str(&format!(
        "font_family = \"{}\"\n",
        escape(&typography.font_family)
    ));
    output.push_str(&format!(
        "mono_family = \"{}\"\n",
        escape(&typography.mono_family)
    ));
    output.push_str(&format!("font_size = {}\n", typography.font_size));

    output.push_str("\n[metrics]\n");
    output.push_str(&format!("radius = {}\n", metrics.radius));
    output.push_str(&format!("spacing = {}\n", metrics.spacing));
    output.push_str(&format!("border_width = {}\n", metrics.border_width));
    output.push_str(&format!("opacity = {}\n", metrics.opacity));

    output
}

/// Renders a theme definition from metadata and colours.
pub fn render_from_parts(metadata: &ThemeMetadata, colors: &ThemeColors) -> String {
    render_theme_parts(
        metadata,
        colors,
        &ThemeTypography::default(),
        ThemeMetrics::default(),
    )
}

fn render_colors(metadata: &ThemeMetadata, colors: &ThemeColors) -> String {
    let mut output = String::new();

    output.push_str("# My Linux theme definition.\n");
    output.push_str("# Edit the colours, then import this file with `my-linux-theme import`.\n\n");
    output.push_str("[theme]\n");
    output.push_str(&format!("schema = {}\n", metadata.schema));
    output.push_str(&format!("name = \"{}\"\n", escape(&metadata.name)));
    output.push_str(&format!("id = \"{}\"\n", metadata.id));
    output.push_str(&format!("version = \"{}\"\n", metadata.version));
    output.push_str(&format!(
        "description = \"{}\"\n",
        escape(&metadata.description)
    ));

    if let Some(author) = &metadata.author {
        output.push_str(&format!("author = \"{}\"\n", escape(author)));
    }

    if let Some(parent) = &metadata.extends {
        output.push_str(&format!("extends = \"{parent}\"\n"));
    }

    output.push_str("\n[colors]\n");

    for (key, color) in [
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
    ] {
        output.push_str(&format!("{key} = \"{}\"\n", color.to_hex()));
    }

    output.push_str("\n[colors.states]\n");
    output.push_str(&format!("active = \"{}\"\n", colors.active.to_hex()));
    output.push_str(&format!("hover = \"{}\"\n", colors.hover.to_hex()));
    output.push_str(&format!("selected = \"{}\"\n", colors.selected.to_hex()));

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::Registry;

    #[test]
    fn exported_theme_round_trips() {
        let registry = Registry::new(vec!["../../themes".into()]);
        let theme = registry.load("cyberpunk").expect("cyberpunk should load");

        let rendered = render_theme_toml(&theme);

        assert!(rendered.contains("[theme]"));
        assert!(rendered.contains("[colors.states]"));
        assert!(rendered.contains("primary = \"#FF235A\""));

        let directory = std::env::temp_dir().join(format!(
            "my-linux-template-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.subsec_nanos())
                .unwrap_or(0)
        ));

        std::fs::create_dir_all(&directory).unwrap();

        let file = directory.join("cyberpunk.toml");
        std::fs::write(&file, rendered).unwrap();

        let reimported = Registry::load_path(&file).expect("exported theme should load");

        assert_eq!(reimported.metadata.id, theme.metadata.id);
        assert_eq!(reimported.metadata.name, theme.metadata.name);
        assert_eq!(
            reimported.colors.primary.to_hex(),
            theme.colors.primary.to_hex()
        );
        assert_eq!(
            reimported.colors.selected.to_hex(),
            theme.colors.selected.to_hex()
        );
        assert_eq!(reimported.typography, theme.typography);
        assert_eq!(reimported.metrics, theme.metrics);

        std::fs::remove_dir_all(directory).unwrap();
    }
}
