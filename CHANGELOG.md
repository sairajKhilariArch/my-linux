# Changelog

All notable changes to My Linux are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-11-19

### Added

- `ConfigError::InvalidPath` and `ConfigError::MissingBackup`, so invalid
  targets and missing backups report a specific, actionable message instead of
  a bare I/O error.
- Path validation that rejects empty paths and any `..` traversal before a
  file is written or backed up.
- Symbolic link handling: installing onto a symlinked destination writes
  through to the link target and leaves the link itself intact.
- Permission preservation: replacing a configuration keeps the existing file
  mode, and a backup keeps the mode of the file it copied.
- Failure tests for invalid destinations, missing sources, installs that must
  not touch existing content, missing backups, permissions and symlinks.

### Changed

- `install_file` now delegates to `install_content`, so installing from a
  source file, installing generated content and restoring a backup all share
  one atomic write implementation.
- The workspace version lives once in `[workspace.package]`; member crates
  inherit it, so a release bumps a single line.
