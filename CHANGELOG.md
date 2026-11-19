# Changelog

All notable changes to My Linux are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.15.0] - 2026-11-19

### Added

- New `desktop/gui` crate: a native `my-linux-gui` binary built on eframe/egui
  that drives the existing `desktop-core` crates. The GUI has no theme engine of
  its own and writes only inside the selected installation root.
- Dashboard: active theme, active profile, installed-file count, detected
  integrations, backups, wallpaper, asset summary and the six most recent events,
  plus one-click theme switching.
- Theme gallery: palette cards, apply, palette inspector, base-theme editor,
  export, import and removal of user themes.
- Theme editor: start from any theme, edit nine colours with live previews, save
  a custom theme through the normal registry validation, or save and apply it.
- Application manager: detection result and generated-file state per integration,
  per-integration include checkboxes, an "apply selected integrations" action and
  the list of files changed outside My Linux.
- Wallpaper manager: library listing, palette extraction preview, save as theme,
  save and apply, and wallpaper history.
- Profile manager: apply, export, create, import, and restore the previous
  profile in one click.
- Recovery screen: doctor output, rollback, restore originals, interrupted
  transaction recovery, installed-file state, backups, installation history and
  the event log.
- Appearance: light and dark modes, and a `--root` option that defaults to
  `$MY_LINUX_ROOT` and then to the XDG state directory.
- Five GUI state tests covering theme/integration discovery, applying and
  restoring themes, custom theme saving and removal, profile create/apply, and
  the diagnostics screen.

### Changed

- The workspace now includes `desktop/gui`; `my-linux-theme` and `my-linux-gui`
  share one core.
- Phase 9 of the roadmap is complete. The GUI window itself needs a Wayland or
  X11 session: it was compiled, its `--help` path was run, and its state layer was
  tested end to end, but the window was not displayed in this environment.

## [0.14.0] - 2026-11-19

### Added

- Structured event log: every apply, restore, rollback, profile application,
  wallpaper change and asset change is recorded as one JSON object per line in
  `state/events.jsonl`, including failed operations.
- `logs [--limit <n>]` prints the event log with timestamps and levels.
- `current` reports the active theme, active profile, wallpaper, font/icon/cursor
  settings, installed-file health and the last recorded action.
- `doctor` now also checks the active profile against the profile registry, the
  recorded wallpaper, the asset settings, the backup count and the event log.
- `backup list` shows when each backup was written in addition to its size and
  digest.
- 3 event-log unit tests and 2 CLI end-to-end tests for the diagnostics.

### Changed

- `integrations status` is now a real per-integration report: detection result
  plus the state of the generated file (unmodified, modified, missing or not
  installed) for every integration.
- `status` reports how many integrations are configured and how many themes are
  available, and shows the last recorded action.
- `diagnose` takes an optional profile registry so it can validate the active
  profile.
- Phase 8 of the roadmap is complete.

## [0.13.0] - 2026-11-19

### Added

- New `desktop-profiles` crate: TOML profile schema holding the theme
  reference, wallpaper, integration list, font/icon/cursor settings and the
  Hyprland layout, with validation and a registry whose user profiles shadow
  built-in presets.
- Four built-in presets in `profiles/`: Cyberpunk Developer, Minimal Workspace,
  Nature and Gaming.
- `profile list`, `info`, `preview`, `create`, `set`, `duplicate`, `remove`,
  `import`, `export`, `apply`, `restore` and `current`.
- `profile apply` configures a whole desktop with one command: the theme across
  the profile's integrations, a generated `hyprland/layout.conf` snippet, the
  font/icon/cursor settings and the profile wallpaper.
- The layout snippet is installed inside the theme transaction through the new
  `apply_theme_with_extra`, so rollback and restore treat it like any other
  generated file.
- `state/active-profile` and `state/profile-previous` track the active profile;
  `status` and `profile current` report it, and `profile restore` re-applies the
  previously active one.
- Validation happens before the first write: schema, ids, version, integration
  ids, opacities and non-zero font/cursor sizes.
- 9 profile tests, 4 core profile-application tests and 2 CLI end-to-end tests.

### Changed

- `restore` also clears the recorded active profile.
- Phase 7 of the roadmap is complete.

## [0.12.0] - 2026-11-19

### Added

- New `desktop-assets` crate: wallpaper library, recorded wallpaper state and
  history, ANSI truecolor preview, palette extraction, and font/icon/cursor
  discovery with an asset library and settings.
- `wallpaper list`, `preview`, `apply`, `current`, `history`, `restore` and
  `palette` commands.
- `wallpaper palette` analyses a PNG or JPEG wallpaper, prints the dominant
  colours and a proposed palette, and derives the theme through the existing
  palette generator. Nothing is saved or applied without `--save` / `--apply`,
  so the active theme is never replaced silently.
- `wallpaper apply` records the choice, copies the file into the library and
  switches the desktop with `swww` or `hyprpaper` when one is detected; without
  a wallpaper daemon the choice is recorded and the desktop is left untouched.
- `assets fonts|icons|cursors` discovery, `assets status`, `assets set`,
  `assets reset`, `assets install`, `assets installed` and `assets remove`.
- `assets set` writes `state/assets.conf` plus a `hyprland/assets.conf` snippet
  with `XCURSOR_THEME`, `XCURSOR_SIZE` and `cursor.size`; `--reload` also calls
  `hyprctl setcursor` for a running Hyprland session.
- 15 unit tests for the assets crate and 2 CLI end-to-end tests for wallpapers
  and asset settings.

### Changed

- `image` with only the PNG and JPEG codecs is the first new dependency;
  extracting a palette needs real pixel data and no lighter option exists.
- Phase 6 of the roadmap is complete. Per-profile wallpapers move to Phase 7
  together with profiles.

## [0.11.0] - 2026-11-19

### Added

- Seven extended integrations, each enableable on its own with
  `--integration <id>`:
  - `gtk3` and `gtk4` — stylesheets defining the palette colours and the
    window, headerbar, button, entry, selection, menu, tooltip and scrollbar
    states.
  - `qt` — qt6ct/qt5ct colour scheme with `active`, `inactive` and `disabled`
    palettes covering all 21 `QPalette` roles.
  - `zsh` — prompt, completion listing colours and autosuggestion styling,
    sourced from `.zshrc`.
  - `neovim` — Lua colour scheme module with editor, UI and diagnostic
    highlights.
  - `yazi` — file manager theme with `mgr`, `status`, `tabs` and `filetype`
    sections.
  - `btop` — system monitor theme with all 40 colour keys.
- `validate_toml` helper rejects generated TOML that cannot be parsed.
- Unit tests for every new renderer together with its output validation, and a
  CLI test that applies and renders the extended integrations end to end.

### Changed

- `integrations list` and `integrations status` now report thirteen
  integrations instead of six.
- Phase 5 of the roadmap is complete for the applications listed above; browser
  styling is intentionally not implemented because no stable configuration
  format exists.

## [0.10.0] - 2026-11-19

### Added

- `integrations list` shows every supported application, its destination file
  and its reload behaviour.
- `integrations status` reports which of those applications are installed.
- `apply --integration <id>` (repeatable) applies only the named integrations.
- `apply --reload` reloads detected applications after a successful apply; no
  process is touched without the flag.
- `render <theme> --integration <id>` prints any generated application
  configuration, not just Waybar CSS.
- CLI integration tests for the integration listing, multi-file apply,
  single-integration apply, per-integration render, unknown-integration errors,
  reload reporting and restore of generated files.

### Changed
- `apply` reports every installed file and states whether a reload was skipped.
- `restore` removes the obsolete rollback snapshots and reports honestly when
  no original configuration was replaced.
- Phase 4 of the roadmap is marked complete and the README documents the
  integration table, snippet layout and safety guarantees.

## [0.9.0] - 2026-11-19

### Added

- `desktop-integrations` crate with an `Integration` trait covering id, name,
  destination path, application detection, rendering, output validation and
  reload commands.
- Waybar integration: complete CSS with bar, workspace, module, tooltip, menu
  and urgency styling derived from the theme palette.
- Rofi integration: RASI theme with window, input bar, list view, element and
  selection states.
- Hyprland integration: a dedicated snippet with active/inactive borders,
  rounding, shadow, blur and background colour. The user's `hyprland.conf` is
  never touched; the snippet is meant to be sourced from it.
- Kitty integration: terminal colours, 16-colour palette, selection, cursor,
  tab colours and font settings.
- Notifications (Dunst) integration: global style plus low, normal and critical
  urgency sections.
- Lock screen (Hyprlock) integration: background, clock labels and input field.
- Integration detection through `PATH`, a status report per integration, and
  per-integration reload commands that are only executed when the application
  is present and a reload is explicitly requested.
- Multi-file transaction: `apply` now installs every integration in one
  transaction and `restore`/`rollback` handle all generated files.

### Fixed

- A generated configuration can no longer be mistaken for a user original on a
  second apply: files recorded as installed are never backed up as originals.
- `restore` removes generated files that never replaced a user file instead of
  failing, and refuses to delete a file the user edited by hand.

## [0.8.0] - 2026-11-19

### Added

- `create <id>` builds a user theme, either from another theme
  (`--from <theme>`) or from a seed colour (`--base '#RRGGBB'`), and writes it
  to the user theme directory.
- `import <path> [--as <id>]` imports a theme directory or a single `.toml`
  file, validating it before anything is written.
- `remove <id>` deletes a user theme. It refuses built-in themes and only
  deletes inside the user theme directory.
- `MY_LINUX_USER_THEMES` selects the user theme directory (default
  `$XDG_DATA_HOME/my-linux/themes`), and user themes take precedence over
  built-in themes.
- `list` shows the id, name, version and source of every discovered theme, plus
  every invalid theme file with its error.
- `info` reports schema, author, inheritance parent and the theme path.

### Changed
- `export` now writes a portable theme `.toml` file that `import` accepts,
  instead of duplicating the CSS that `render` already prints.
- Applying, previewing, rendering and inspecting accept any discovered theme
  id, including user themes and inherited themes.
- CLI tests cover user theme creation, generation from a colour, import and
  removal, inheritance of built-in colours and rejection of invalid ids.
- Phase 3 of the roadmap is marked complete and the README documents the theme
  schema, inheritance and both theme layouts.

## [0.7.0] - 2026-11-19

### Added

- Dynamic theme discovery: `Registry` searches one or more theme roots in
  order, lists valid themes with their source and reports broken theme files
  instead of hiding them. Directory themes (`<id>/colors/palette.toml`) and
  single-file themes (`<id>.toml`) are both supported.
- Theme schema versioning: `schema` in `[theme]`, `SCHEMA_VERSION` and a
  dedicated `UnsupportedSchema` error for themes written for a newer build.
- Theme metadata validation: id syntax, id/directory agreement, non-empty name
  and description, and `MAJOR.MINOR.PATCH` versions, with specific errors.
- Theme inheritance: `extends = "parent"` merges the parent palette and only
  overrides the colours the child defines, with cycle detection.
- Palette generation from a single seed colour (`generate_palette`), using HSL
  normalisation so unusual seeds still produce a readable dark theme.
- Theme serialisation (`render_theme_toml`, `render_from_parts`) so a theme can
  be exported and re-imported without loss.
- `Color::from_hex`, `Color::mix`, `lighten` and `darken`.

### Changed

- Removed the hard-coded `Theme` enum and its duplicate palettes. Palettes now
  come only from TOML, so a theme can be added or changed without touching Rust
  source code.
- `LoadedTheme` reports the path it was loaded from.
- Diagnostics list every discovered theme and every invalid theme file.

## [0.6.0] - 2026-11-19

### Added

- `rollback` command: reverts to the state before the most recent apply and
  restores the previous theme, keeping installed-file tracking intact.
- `history` command: lists every apply, restore, rollback and recovery with a
  readable UTC timestamp and the files involved.
- `backup list` command: lists the retained backups with size and digest.
- `doctor` command: reports the installation root, theme loading, active theme,
  an interrupted operation, modified files and the original backup. It exits
  with a non-zero status when a check fails.
- `status` now reports each installed file as unmodified, hand-edited or
  missing.

### Changed

- `apply` runs as a transaction and prints the installed files and the backups
  it created; `restore` also runs through the transaction and records history.
- Phase 2 of the roadmap is marked complete and the README documents the
  transaction, the state layout and the recovery commands.

## [0.5.0] - 2026-11-19

### Added

- Transaction-style application in `desktop-core::transaction`: generate,
  validate, journal, back up, install, commit — or roll every file back on
  failure, so a failed apply never leaves a half-themed desktop.
- Installation journal (`state/journal.tsv`): written before the first file is
  touched and cleared on commit. An interrupted operation is replayed and
  reverted by the next command that touches the installation.
- Backup retention scheme: `backups/<file>.original` keeps the first original
  configuration forever, `backups/<file>.previous` holds the state before the
  latest operation and powers `rollback`.
- Installed-state tracking (`state/installed.tsv`) with per-file digests, and
  modification detection that distinguishes unmodified, hand-edited and missing
  configurations.
- Installation history (`state/history.log`) with apply, restore, rollback and
  recovery entries, plus `format_timestamp` for readable UTC timestamps.
- `diagnose()`: installation root, theme loading, active theme, interrupted
  journal, modified files and original backup in one report.
- Configuration destination mapping (`destination`, `original_backup`,
  `previous_backup`) so integrations share one layout definition.
- `digest`, `digest_file`, `snapshot_file` and `remove_file_if_exists` in the
  configuration manager.

### Changed

- `apply` and `restore` now run through the transaction instead of writing a
  single file directly, and both record their outcome in the history.
- Applying a theme validates the generated CSS before anything is written.

## [0.4.0] - 2026-11-19

### Added

- `restore` command: restores the original Waybar configuration from its
  backup, clears the recorded active theme and keeps the backup for later use.
- `status` command: reports the installation root, the active theme and whether
  the generated configuration and the original backup are present.
- `info` command: prints theme metadata and the complete 14-colour palette.
- `--root <directory>` (and `--root=<directory>`) on every command, plus the
  `MY_LINUX_ROOT` environment variable, so the whole workflow can run against
  an isolated directory.
- `export <theme> [file]` accepts an explicit output path.
- Help output generated by `my-linux-theme --help`.
- End-to-end CLI tests covering listing, preview, render, apply, repeated
  apply, theme switching, restore, restore without a backup, status, export and
  error handling. The suite runs in temporary roots.
- A real `README.md` documenting the commands, the installation root layout and
  the safety guarantees.

### Changed
- `apply` no longer fabricates a fake original configuration. When no previous
  configuration exists it installs the theme and reports that there is nothing
  to back up.
- Unknown commands fail with a non-zero exit status and print the help text.
- Phase 1 of the roadmap is marked complete; `docs/to-do.md` reflects the
  verification that actually ran.

## [0.3.0] - 2026-11-19

### Added

- `WaybarPaths` / `waybar_paths(root)`, a single definition of where the
  Waybar configuration and its first-run backup live inside an installation
  root (`waybar.css` and `backups/waybar.css.original`).
- `restore_waybar_theme(destination, backup)`, the supported restore path for
  the Waybar integration. It fails with `MissingBackup` instead of silently
  doing nothing.
- Active theme tracking: `write_active_theme`, `read_active_theme` and
  `clear_active_theme` record the applied theme in `state/active-theme` under
  the installation root and remove it after a restore.
- Core tests for path mapping, repeated installs, switching themes, the full
  install/restore cycle and the active-theme round trip.

### Changed
- `install_waybar_theme` documents that the original backup is created once,
  so repeated applications never overwrite the user's first configuration.
- `Theme` is re-exported from `desktop-core`, so the CLI no longer needs to
  depend on the theme engine directly.

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
