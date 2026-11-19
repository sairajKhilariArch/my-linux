//! Rendering extension templates from the central theme palette.
//!
//! Placeholders use `{{name}}` syntax. Supported names are the theme metadata
//! (`theme.id`, `theme.name`, `theme.version`, `theme.author`) and every theme
//! colour (`colors.background`, `colors.primary`, `colors.active`, ...).
//! An unknown or unterminated placeholder is an error rather than silently
//! producing a broken configuration.

use theme_engine::{LoadedTheme, ThemeColors};

use crate::registry::ExtensionError;

/// Replaces every placeholder in `template` with a value from `theme`.
pub fn render(template: &str, theme: &LoadedTheme) -> Result<String, ExtensionError> {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);

        let after = &rest[start + 2..];
        let end = after.find("}}").ok_or_else(|| {
            ExtensionError::Template("unterminated `{{` placeholder in the template".to_string())
        })?;
        let key = after[..end].trim();

        output.push_str(&resolve(key, theme)?);

        rest = &after[end + 2..];
    }

    output.push_str(rest);

    Ok(output)
}

/// Resolves a single placeholder name.
pub fn resolve(key: &str, theme: &LoadedTheme) -> Result<String, ExtensionError> {
    let metadata = &theme.metadata;

    if let Some(name) = key.strip_prefix("colors.") {
        return Ok(color(&theme.colors, name)?.to_hex());
    }

    match key {
        "theme.id" => Ok(metadata.id.clone()),
        "theme.name" => Ok(metadata.name.clone()),
        "theme.version" => Ok(metadata.version.clone()),
        "theme.description" => Ok(metadata.description.clone()),
        "theme.author" => Ok(metadata.author.clone().unwrap_or_default()),
        "" => Err(ExtensionError::Template(
            "empty `{{}}` placeholder in the template".to_string(),
        )),
        other => Err(ExtensionError::Template(format!(
            "unknown placeholder `{{{{{other}}}}}`"
        ))),
    }
}

fn color(colors: &ThemeColors, name: &str) -> Result<theme_engine::Color, ExtensionError> {
    let color = match name {
        "background" => colors.background,
        "surface" => colors.surface,
        "surface_alt" => colors.surface_alt,
        "primary" => colors.primary,
        "secondary" => colors.secondary,
        "text" => colors.text,
        "text_muted" => colors.text_muted,
        "border" => colors.border,
        "success" => colors.success,
        "warning" => colors.warning,
        "error" => colors.error,
        "active" => colors.active,
        "hover" => colors.hover,
        "selected" => colors.selected,
        other => {
            return Err(ExtensionError::Template(format!(
                "unknown colour `colors.{other}`"
            )));
        }
    };

    Ok(color)
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

    #[test]
    fn renders_theme_metadata_and_colours() {
        let rendered = render(
            "/* {{theme.name}} {{theme.id}} {{theme.version}} */\n\
             background: {{colors.background}};\n\
             accent: {{colors.primary}};\n\
             hover: {{colors.hover}};\n",
            &theme(),
        )
        .unwrap();

        assert!(rendered.contains("Cyberpunk cyberpunk 1.0.0"));
        assert!(rendered.contains("background: #0C0812;"));
        assert!(rendered.contains("accent: #FF235A;"));
        assert!(rendered.contains("hover: #00E6FF;"));
    }

    #[test]
    fn leaves_text_without_placeholders_untouched() {
        assert_eq!(render("plain text\n", &theme()).unwrap(), "plain text\n");
    }

    #[test]
    fn rejects_unknown_and_unterminated_placeholders() {
        let error = render("{{colors.nope}}", &theme()).unwrap_err();

        assert!(error.to_string().contains("unknown colour"));

        let error = render("{{theme.unknown}}", &theme()).unwrap_err();

        assert!(error.to_string().contains("unknown placeholder"));

        let error = render("{{colors.primary", &theme()).unwrap_err();

        assert!(error.to_string().contains("unterminated"));

        assert!(render("{{}}", &theme()).is_err());
    }
}
