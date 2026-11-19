//! Host detection: distribution, architecture, session and runtime tools.

use std::fs;
use std::path::PathBuf;

/// Finds an executable on `PATH`.
pub fn find_program(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;

    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

#[derive(Debug, Clone)]
pub struct HostEnvironment {
    pub os_id: String,
    pub os_name: String,
    pub architecture: String,
    pub session: Option<String>,
    /// True for Arch and Arch-derived systems (or any system with pacman).
    pub arch_based: bool,
}

impl HostEnvironment {
    pub fn summary(&self) -> String {
        format!("{} ({}, {})", self.os_name, self.os_id, self.architecture)
    }
}

/// Reads `/etc/os-release`, the architecture and the current session.
pub fn detect_environment() -> HostEnvironment {
    let release = fs::read_to_string("/etc/os-release").unwrap_or_default();
    let value = |key: &str| -> Option<String> {
        release.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;

            if name.trim() != key {
                return None;
            }

            Some(value.trim().trim_matches('"').to_string())
        })
    };

    let os_id = value("ID")
        .unwrap_or_else(|| "unknown".to_string())
        .to_lowercase();
    let os_name = value("PRETTY_NAME").unwrap_or_else(|| os_id.clone());
    let architecture = std::env::consts::ARCH.to_string();
    let session = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .or_else(|| std::env::var("WAYLAND_DISPLAY").ok())
        .or_else(|| std::env::var("DISPLAY").ok());
    let arch_based = matches!(
        os_id.as_str(),
        "arch" | "archarm" | "manjaro" | "endeavouros" | "cachyos" | "garuda"
    ) || find_program("pacman").is_some();

    HostEnvironment {
        os_id,
        os_name,
        architecture,
        session,
        arch_based,
    }
}

/// One runtime tool that integrations or the platform can use.
#[derive(Debug, Clone)]
pub struct DependencyCheck {
    pub name: &'static str,
    pub programs: Vec<&'static str>,
    pub detected: Option<PathBuf>,
    pub required: bool,
    pub purpose: &'static str,
}

impl DependencyCheck {
    pub fn missing(&self) -> bool {
        self.detected.is_none()
    }

    pub fn description(&self) -> String {
        match &self.detected {
            Some(path) => format!("{}: found ({})", self.name, path.display()),
            None => format!(
                "{}: not found (looked for {})",
                self.name,
                self.programs.join(", ")
            ),
        }
    }
}

/// Every supported application plus the platform tools, with detection state.
pub fn dependency_report() -> Vec<DependencyCheck> {
    let mut checks: Vec<DependencyCheck> = desktop_core::integrations::status()
        .into_iter()
        .map(|status| DependencyCheck {
            name: status.id,
            programs: status.programs,
            detected: status.detected,
            required: false,
            purpose: "application integration",
        })
        .collect();

    for (name, programs, purpose) in [
        (
            "hyprctl",
            vec!["hyprctl"],
            "Hyprland reload and monitor control",
        ),
        ("pacman", vec!["pacman"], "Arch package management"),
    ] {
        let detected = programs.iter().find_map(|program| find_program(program));

        checks.push(DependencyCheck {
            name,
            programs,
            detected,
            required: false,
            purpose,
        });
    }

    checks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_host() {
        let environment = detect_environment();

        assert!(!environment.os_id.is_empty());
        assert!(!environment.os_name.is_empty());
        assert!(!environment.architecture.is_empty());
        assert!(environment.summary().contains(&environment.architecture));
    }

    #[test]
    fn reports_every_integration() {
        let report = dependency_report();

        assert!(report.len() >= 15);
        assert!(report.iter().any(|check| check.name == "waybar"));
        assert!(report.iter().any(|check| check.name == "hyprctl"));
        assert!(report.iter().all(|check| !check.description().is_empty()));
    }
}
