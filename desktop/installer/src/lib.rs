//! Installation, upgrade, verification and uninstallation.
//!
//! The installer is deliberately conservative: it only writes inside the chosen
//! prefix, records every file it installs in a manifest, replaces built-in
//! themes and profiles only when they are unmodified, and never touches the
//! user's My Linux state, user themes, profiles or original configuration
//! backups.

pub mod environment;
pub mod layout;
pub mod manifest;
pub mod operations;

pub use environment::{
    DependencyCheck, HostEnvironment, dependency_report, detect_environment, find_program,
};
pub use layout::{InstallLayout, default_prefix, state_dir};
pub use manifest::{Manifest, ManifestFile};
pub use operations::{
    InstallError, InstallOptions, InstallReport, InstallStatus, UninstallReport, VerifyCheck,
    VerifyReport, desktop_entry, install, install_status, uninstall, upgrade, verify,
};

/// Version of the running binaries.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// File name of the desktop entry installed into `share/applications`.
pub const DESKTOP_FILE_NAME: &str = "my-linux.desktop";
