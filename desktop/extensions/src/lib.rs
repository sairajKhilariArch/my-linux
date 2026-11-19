//! User extensions: third-party application integrations described in TOML.
//!
//! An extension is a directory with an `extension.toml` manifest and a template
//! that is rendered from the central theme palette. Extensions are discovered
//! from a built-in directory and a user directory, so a developer can add an
//! application integration without changing or recompiling the Rust core.
//!
//! Rendering only produces the file content. Installation, backup, rollback and
//! restoration go through the same transaction as the built-in integrations.

pub mod manifest;
pub mod registry;
pub mod template;

pub use manifest::{Extension, ExtensionFormat, ExtensionMetadata, SCHEMA_VERSION};
pub use registry::{
    BrokenExtension, ExtensionDiagnostic, ExtensionDiscovery, ExtensionError, ExtensionRegistry,
    ExtensionSource, ExtensionSummary, GeneratedExtension, Settings, user_extensions_dir,
};
pub use template::render;

/// Version of the extension schema understood by this build.
pub const EXTENSION_SCHEMA_VERSION: u32 = SCHEMA_VERSION;
