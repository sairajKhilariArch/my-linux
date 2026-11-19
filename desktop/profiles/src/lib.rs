//! Desktop profiles.
//!
//! A profile is the complete desktop configuration: a theme reference, a
//! wallpaper, the integrations to install, font and icon settings and the
//! window-manager layout. Profiles live in TOML files just like themes, so a
//! user can create one without recompiling My Linux.

pub mod profile;
pub mod registry;

pub use profile::{
    Profile, ProfileAssets, ProfileError, ProfileLayout, ProfileMetadata, SCHEMA_VERSION,
};
pub use registry::{ProfileRegistry, ProfileSource, ProfileSummary};
