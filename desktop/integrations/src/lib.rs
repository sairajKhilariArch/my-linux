//! Desktop application integrations.
//!
//! Each integration turns the central theme palette into the configuration
//! format of one application. Integrations are independent: an application
//! that is not installed simply never reads its generated file, and a reload
//! is skipped instead of failing the whole desktop.

use std::path::PathBuf;
use std::process::Command;

use theme_engine::LoadedTheme;

pub mod btop;
pub mod dunst;
pub mod gtk3;
pub mod gtk4;
pub mod hyprland;
pub mod hyprlock;
pub mod kitty;
pub mod mako;
pub mod neovim;
pub mod qt;
pub mod rofi;
pub mod waybar;
pub mod wofi;
pub mod yazi;
pub mod zsh;

#[derive(Debug)]
pub enum RenderError {
    Empty(&'static str),
    Invalid(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Empty(integration) => {
                write!(f, "{integration} produced an empty configuration")
            }
            RenderError::Invalid(message) => write!(f, "invalid configuration: {message}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// One desktop application that My Linux can configure.
pub trait Integration: Sync {
    /// Stable identifier used by the CLI and the installed-state files.
    fn id(&self) -> &'static str;

    /// Human readable name.
    fn name(&self) -> &'static str;

    /// Destination of the generated file, relative to the installation root.
    fn relative_path(&self) -> &'static str;

    /// Programs that indicate the application is available.
    fn programs(&self) -> &'static [&'static str];

    /// Renders the application configuration for `theme`.
    fn render(&self, theme: &LoadedTheme) -> String;

    /// Rejects output that the application could not parse.
    fn validate(&self, content: &str) -> Result<(), RenderError>;

    /// Commands that reload the application after installation.
    fn reload_commands(&self) -> &'static [&'static [&'static str]] {
        &[]
    }
}

static INTEGRATIONS: &[&dyn Integration] = &[
    &waybar::Waybar,
    &rofi::Rofi,
    &hyprland::Hyprland,
    &kitty::Kitty,
    &dunst::Dunst,
    &mako::Mako,
    &wofi::Wofi,
    &hyprlock::Hyprlock,
    &gtk3::Gtk3,
    &gtk4::Gtk4,
    &qt::Qt,
    &zsh::Zsh,
    &neovim::Neovim,
    &yazi::Yazi,
    &btop::Btop,
];

/// Every integration, in installation order.
pub fn all() -> &'static [&'static dyn Integration] {
    INTEGRATIONS
}

/// Looks up an integration by id.
pub fn find(id: &str) -> Option<&'static dyn Integration> {
    let id = id.trim().to_lowercase();

    INTEGRATIONS
        .iter()
        .copied()
        .find(|integration| integration.id() == id)
}

/// Resolves the integrations selected by id, rejecting unknown ids.
pub fn select(ids: &[String]) -> Result<Vec<&'static dyn Integration>, String> {
    if ids.is_empty() {
        return Ok(all().to_vec());
    }

    let mut selected: Vec<&'static dyn Integration> = Vec::new();

    for id in ids {
        let integration = find(id).ok_or_else(|| {
            let known: Vec<&'static str> = INTEGRATIONS
                .iter()
                .map(|integration| integration.id())
                .collect();

            format!(
                "Unknown integration `{id}`. Known integrations: {}",
                known.join(", ")
            )
        })?;

        if !selected
            .iter()
            .any(|existing| existing.id() == integration.id())
        {
            selected.push(integration);
        }
    }

    Ok(selected)
}

/// Looks for an application on `PATH`.
pub fn detect(programs: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;

    for directory in std::env::split_paths(&path) {
        for program in programs {
            let candidate = directory.join(program);

            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[derive(Debug, Clone)]
pub struct IntegrationStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub programs: Vec<&'static str>,
    pub destination: &'static str,
    pub detected: Option<PathBuf>,
}

impl IntegrationStatus {
    pub const fn available(&self) -> bool {
        self.detected.is_some()
    }
}

/// Detection status for every integration.
pub fn status() -> Vec<IntegrationStatus> {
    INTEGRATIONS
        .iter()
        .map(|integration| IntegrationStatus {
            id: integration.id(),
            name: integration.name(),
            programs: integration.programs().to_vec(),
            destination: integration.relative_path(),
            detected: detect(integration.programs()),
        })
        .collect()
}

/// Runs the reload commands of `integration` when the application is present.
pub fn reload(integration: &dyn Integration) -> Result<Vec<String>, String> {
    if detect(integration.programs()).is_none() {
        return Ok(Vec::new());
    }

    let mut executed = Vec::new();

    for command in integration.reload_commands() {
        let Some((program, arguments)) = command.split_first() else {
            continue;
        };

        let status = Command::new(program)
            .args(arguments)
            .status()
            .map_err(|error| format!("{program}: {error}"))?;

        if !status.success() {
            return Err(format!(
                "`{}` exited with {}",
                command.join(" "),
                status.code().unwrap_or(-1)
            ));
        }

        executed.push(command.join(" "));
    }

    Ok(executed)
}

/// Rejects unbalanced braces in block based formats.
pub fn validate_braces(content: &str) -> Result<(), RenderError> {
    let mut depth: i32 = 0;

    for character in content.chars() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;

                if depth < 0 {
                    return Err(RenderError::Invalid("unbalanced closing brace".to_string()));
                }
            }
            _ => {}
        }
    }

    if depth != 0 {
        return Err(RenderError::Invalid("unbalanced opening brace".to_string()));
    }

    Ok(())
}

/// Rejects TOML that the application could not parse.
pub fn validate_toml(content: &str) -> Result<(), RenderError> {
    toml::from_str::<toml::Value>(content)
        .map(|_| ())
        .map_err(|error| RenderError::Invalid(format!("invalid TOML: {error}")))
}

/// Rejects empty output and output that is missing a required marker.
pub fn require(integration: &'static str, content: &str, needle: &str) -> Result<(), RenderError> {
    if content.trim().is_empty() {
        return Err(RenderError::Empty(integration));
    }

    if !content.contains(needle) {
        return Err(RenderError::Invalid(format!("missing `{needle}`")));
    }

    Ok(())
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
    fn every_integration_renders_and_validates() {
        let theme = theme();

        for integration in all() {
            let content = integration.render(&theme);

            integration
                .validate(&content)
                .unwrap_or_else(|error| panic!("{} failed validation: {error}", integration.id()));

            assert!(!content.trim().is_empty());
            assert!(
                content.contains("#0C0812") || content.contains("0C0812"),
                "{} should use the theme background",
                integration.id()
            );
        }
    }

    #[test]
    fn integration_ids_are_unique() {
        let mut ids: Vec<&str> = all().iter().map(|integration| integration.id()).collect();
        let count = ids.len();

        ids.sort_unstable();
        ids.dedup();

        assert_eq!(ids.len(), count);
    }

    #[test]
    fn selects_all_when_no_ids_are_given() {
        assert_eq!(select(&[]).unwrap().len(), all().len());
    }

    #[test]
    fn selects_named_integrations() {
        let selected = select(&["waybar".to_string(), "kitty".to_string()]).unwrap();

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].id(), "waybar");
        assert_eq!(selected[1].id(), "kitty");
    }

    #[test]
    fn rejects_unknown_integrations() {
        match select(&["nope".to_string()]) {
            Ok(_) => panic!("an unknown integration must be rejected"),
            Err(message) => assert!(message.contains("Unknown integration")),
        }
    }

    #[test]
    fn finds_integrations_by_id() {
        assert_eq!(
            find("WAYBAR").map(|integration| integration.id()),
            Some("waybar")
        );
        assert!(find("unknown").is_none());
    }

    #[test]
    fn detects_nothing_for_impossible_programs() {
        assert!(detect(&["my-linux-does-not-exist-xyz"]).is_none());
    }

    #[test]
    fn reports_status_for_every_integration() {
        let statuses = status();

        assert_eq!(statuses.len(), all().len());
        assert!(statuses.iter().all(|status| !status.programs.is_empty()));
    }

    #[test]
    fn rejects_invalid_content() {
        assert!(require("waybar", "", "x").is_err());
        assert!(require("waybar", "content", "missing-marker").is_err());
        assert!(require("waybar", "content", "content").is_ok());

        assert!(validate_braces("{ }").is_ok());
        assert!(validate_braces("{ ").is_err());
        assert!(validate_braces(" }").is_err());
    }
}
