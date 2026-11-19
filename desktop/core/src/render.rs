use theme_engine::LoadedTheme;

/// Generates Waybar CSS from the active theme.
///
/// Kept as part of the core API; the renderer itself lives in the Waybar
/// integration so there is a single implementation.
pub fn render_waybar_css(theme: &LoadedTheme) -> String {
    desktop_integrations::waybar::render(theme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use theme_engine::loader::load_theme_by_name;

    #[test]
    fn generates_cyberpunk_waybar_css() {
        let theme =
            load_theme_by_name("../../themes", "cyberpunk").expect("Cyberpunk theme should load");

        let css = render_waybar_css(&theme);

        assert!(css.contains("@define-color main-bg #0C0812;"));
        assert!(css.contains("@define-color main-fg #FFF0FA;"));
        assert!(css.contains("@define-color wb-act-bg #FF235A;"));
    }
}
