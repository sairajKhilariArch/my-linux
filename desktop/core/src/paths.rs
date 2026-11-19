//! Where My Linux looks for the data it ships with.
//!
//! An installed binary lives in `<prefix>/bin`, with themes and profiles in
//! `<prefix>/share/my-linux`. A development build lives in `target/debug` with
//! the repository next to it. Both are supported, in this order:
//!
//! 1. an explicit environment override,
//! 2. `<executable>/../share/my-linux`,
//! 3. the repository the binary was built from,
//! 4. `themes` or `profiles` relative to the working directory.

use std::path::PathBuf;

/// Data directory of an installed copy, if this looks like one.
pub fn install_data_dir() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("MY_LINUX_DATA_DIR") {
        return Some(PathBuf::from(directory));
    }

    let executable = std::env::current_exe().ok()?;
    let directory = executable.parent()?.join("../share/my-linux");

    if directory.is_dir() {
        Some(directory)
    } else {
        None
    }
}

fn resolve(variable: &str, name: &str) -> PathBuf {
    if let Some(directory) = std::env::var_os(variable) {
        return PathBuf::from(directory);
    }

    if let Some(data) = install_data_dir() {
        let directory = data.join(name);

        if directory.is_dir() {
            return directory;
        }
    }

    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(name);

    if repository.is_dir() {
        return repository;
    }

    PathBuf::from(name)
}

/// Directory holding the built-in themes.
pub fn builtin_themes_dir() -> PathBuf {
    resolve("MY_LINUX_THEMES_DIR", "themes")
}

/// Directory holding the built-in desktop profiles.
pub fn builtin_profiles_dir() -> PathBuf {
    resolve("MY_LINUX_PROFILES_DIR", "profiles")
}

/// Directory holding the built-in extensions.
pub fn builtin_extensions_dir() -> PathBuf {
    resolve("MY_LINUX_EXTENSIONS_DIR", "extensions")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_built_in_themes_in_a_development_tree() {
        let directory = builtin_themes_dir();

        assert!(directory.is_dir(), "{} should exist", directory.display());
        assert!(directory.join("cyberpunk").is_dir());
    }

    #[test]
    fn finds_the_built_in_profiles() {
        let directory = builtin_profiles_dir();

        assert!(directory.is_dir(), "{} should exist", directory.display());
    }

    #[test]
    fn finds_the_built_in_extensions() {
        let directory = builtin_extensions_dir();

        assert!(directory.is_dir(), "{} should exist", directory.display());
        assert!(directory.join("swaync/extension.toml").is_file());
    }
}
