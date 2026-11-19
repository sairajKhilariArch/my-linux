# Changelog

All notable changes to My Linux are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.27.0] - 2026-11-19

### Added

- Backup metadata sidecars in `backups/.meta/`: every original and previous
  backup records its timestamp, source theme, content digest, byte size and
  permissions. `backup list` now walks nested backup paths and reports the
  theme, age and verification state (`verified`, `legacy`, `CORRUPT`).
- Backup integrity verification: `restore` checks the original backup against
  its recorded digest before writing. A corrupted backup aborts the restore and
  leaves the installed configuration untouched. `doctor` reports corrupt and
  legacy backups.
- `backup prune [--keep <days>] [--dry-run]` removes `.previous` snapshots older
  than 30 days by default. Original backups are never pruned.
- Apply conflict handling: applying no longer overwrites generated files that
  were edited by hand. It stops with the conflicting paths and the CLI/GUI offer
  `--force` (or the “replace generated files I edited” checkbox) to snapshot and
  replace them deliberately.
- Waybar module overrides: `[overrides.waybar.modules.<module>]` sets
  `background`, `foreground` and `border` for individual Waybar modules. The
  renderer appends one CSS rule per module and the schema validates module names
  and colours.
- Notification per-application rules: `[overrides.dunst.modules.<app>]` and
  `[overrides.mako.modules.<app>]` generate app-specific Dunst sections and Mako
  `[app-name=...]` blocks with background, text and border colours.
- Firefox integration (`firefox/userChrome.css`): tabs, navigation bar, URL bar
  and toolbar are themed, with documented instructions for enabling
  `toolkit.legacyUserProfileCustomizations.stylesheets`.
- Profile dependencies: profiles accept an optional `dependencies` list. The
  CLI normalises and validates it, `profile set <id> dependencies ...` edits it,
  `profile info` shows it and profile application warns about missing programs
  on `PATH`.
- GUI profile editing: the profile manager has an edit form for name, theme,
  wallpaper, integrations and dependencies, plus a “Style” shortcut from the
  application manager to the per-integration editor.
- GUI wallpaper image preview decoded with the `image` crate, and a rendered
  desktop mock preview in the theme editor using the draft palette.
- Extension permissions: manifests declare `permissions = ["reload"]` before
  reload commands are accepted. Unknown permissions are rejected, `extensions
  info` displays them and the built-in SwayNC extension declares `reload`.
- `bump <id> <major|minor|patch>` raises a theme version, writes the user copy
  through the same validated export path and reports the new version.

### Changed

- Theme/module overrides now round-trip through `render_theme_toml`, so export,
  duplicate and version bumps keep module styling.
- The integration list grows to sixteen and the GUI/README counts follow.
- Workspace version 0.27.0; `PKGBUILD` `pkgver` follows it.

### Tests

- Theme engine: module overrides parse, inherit, export and reject invalid names
  and colours.
- Integrations: Waybar, Dunst and Mako emit per-module/per-app rules; Firefox
  renders and validates its browser stylesheet.
- Core: backup metadata and corruption detection, conflict refusal and forced
  replacement, retention pruning that keeps originals, and doctor backup health.
- Extensions: reload commands require the `reload` permission, unknown
  permissions are rejected and permissions survive manifest serialisation.
- Profiles and GUI: dependency normalisation and the sixteen-integration count.

### Limitations

- GUI changes were compiled and state-tested but not opened in a display session.
- Firefox, notification daemons, Waybar and Hyprland reloads were not executed in
  this environment.
- AUR publication, hosted marketplace listings and release artifacts remain
  external publishing steps; a code-level third-party integration plugin API is
  not included.

## [0.26.0] - 2026-11-19

### Added

- Lock screen indicators: the generated Hyprlock input field now sets
  `check_color` (secondary), `fail_color` and `fail_text` (error palette),
  `capslock_color` (primary), `numlock_color` (success) and `bothlock_color`
  (warning), with `swap_font_color`. Locks and authentication results are
  visible in the theme palette instead of Hyprlock defaults.
- The input field now follows the theme border width, corner radius and
  interface font instead of hard-coded values, and honours `hyprlock` token
  overrides.
- Fixed the CLI help block where the `create` description was printed after the
  `duplicate` usage line.

### Tests

- Hyprlock: the lock screen config carries the indicator colours, theme border
  width, radius and font.

### Changed

- Workspace version 0.26.0; `PKGBUILD` `pkgver` follows it.

## [0.25.0] - 2026-11-19

### Added

- Wallpaper changes refresh the Hyprland lock screen configuration. One core
  function, `refresh_lock_screen_config`, is used by `wallpaper apply`,
  `wallpaper restore` and the GUI wallpaper manager, so the shell and graphical
  paths cannot drift apart.
- The refresh only touches a file the installation owns and only while it is
  unmodified, rewrites it atomically and updates its recorded digest so `doctor`
  still reports the file as unmodified. Without an installed lock screen
  configuration nothing is created.

### Fixed

- Profile application records its wallpaper before the transaction, but now
  restores the previous current/previous wallpaper state when the transaction
  fails instead of leaving state behind for a configuration that was never
  installed.
- The GUI theme editor removes the written theme file when the saved theme does
  not load, instead of leaving a broken theme that `list` would report forever.
- A Hyprlock file edited by hand is left alone by a wallpaper refresh and the
  command reports why, rather than silently discarding the edit.

### Review of 0.22.0–0.24.0

- Simplified the wallpaper/lock-screen path: rendering, validation, digest
  bookkeeping and event logging live in one core function rather than in each
  caller; the new `WallpaperState::restore_state` replaces ad-hoc state rollback.
- Verified the override, shadow and render-context changes for failure safety:
  invalid overrides cannot be saved, previews remain read-only, and every
  generated file is validated before the atomic install.

### Tests

- CLI: a wallpaper apply changes the lock screen from the themed colour to the
  wallpaper path.
- Core: a modified lock screen is refused; failed profile application restores
  the wallpaper state; lock-screen refresh keeps the digest current.
- Assets: state restore clears current/previous without writing history.
- GUI: a theme that fails validation is removed after saving.

### Changed

- Workspace version 0.25.0; `PKGBUILD` `pkgver` follows it.

## [0.24.0] - 2026-11-19

### Added

- Lock screen wallpaper integration: integrations can render with runtime
  context (`RenderContext { wallpaper }`), and Hyprlock now uses the active
  wallpaper as its `background { path = … }` when one is recorded and still
  exists. Without a wallpaper it keeps the themed background colour, so the
  generated file stays valid either way.
- Profile application records the profile's wallpaper before the theme
  transaction, so switching a profile updates the lock screen in the same apply
  instead of waiting for the next one.

### Tests

- Hyprlock renders the wallpaper path when given one and falls back to the
  theme colour otherwise.
- Theme application writes the active wallpaper into the lock screen config.
- Profile application writes its wallpaper into the lock screen config and
  records it in state.

### Changed

- Workspace version 0.24.0; `PKGBUILD` `pkgver` follows it.

## [0.23.0] - 2026-11-19

### Added

- Shadow tokens in the theme schema: `metrics.shadow_range` (0–40 px, default
  12) and `metrics.shadow_opacity` (0–100 %, default 40). They are validated,
  exported, inherited and may be overridden per integration like the other
  design tokens.
- Hyprland renders the shadow range, computes the colour alpha from the token
  (so a theme can soften or disable shadows), emits `color_inactive`, and turns
  the shadow off when either token is zero.
- GUI theme editor exposes Shadow range and Shadow opacity next to border width
  and opacity, saving them through the same path.
- Cyberpunk demonstrates the tokens with `shadow_range = 16` and
  `shadow_opacity = 55`.

### Tests

- Loader: shadow defaults, per-application shadow overrides, export round-trip
  and rejection of out-of-range shadow values.
- Hyprland: renders the configured range and derived alpha, and disables the
  shadow at range zero.
- GUI: saved themes keep shadow tokens.

### Changed

- Workspace version 0.23.0; `PKGBUILD` `pkgver` follows it.

## [0.22.0] - 2026-11-19

### Added

- Application-specific theme overrides. Themes may declare
  `[overrides.<integration>]` with `font_family`, `mono_family`, `font_size`,
  `radius`, `spacing`, `border_width` and `opacity`. Integrations ask for
  `typography_for(id)` / `metrics_for(id)`, so only the named application uses
  the override and everything else keeps the shared tokens.
- Overrides inherit through `extends` field by field, are range-validated and
  round-trip through export. `info <theme>` prints them.
- `my-linux-theme duplicate <from> <to>` copies colours, typography, metrics and
  overrides into a new user theme; `create --from` now keeps the parent's tokens
  and overrides instead of resetting them.
- GUI theme editor: border width, opacity and a per-integration application
  override section (font size and corner radius, clearable), saved through the
  same `save_theme` path as every other edit.
- Cyberpunk now ships a Waybar override (font size 10, radius 4) and Futuristic
  a Kitty override (font size 13), so the feature is visible in bundled themes.

### Tests

- Theme loader: override parsing, token merging, inheritance and out-of-range
  rejection; rendering and round-trip of override sections.
- Waybar: renders its own overridden tokens.
- CLI: `duplicate` preserves overrides, `info` lists them, duplicate ids fail,
  and `create --from` keeps the parent palette with its overrides.
- GUI: `save_theme` persists overrides and reloads them through the registry.

### Changed

- Workspace version 0.22.0; `PKGBUILD` `pkgver` follows it.

## [0.21.0] - 2026-11-19

### Added

- Extension health checks: `ExtensionRegistry::diagnose` reports broken
  manifests, enabled extensions that no longer load, and whether their generated
  file was applied. `my-linux-theme doctor` and the GUI recovery screen include
  these checks, and `doctor` still exits non-zero when a check needs attention.
- GUI Repository screen: index location field (prefilled from
  `MY_LINUX_REPOSITORY`), load/refresh, package table with kind, version,
  description and availability, install action and an optional replace-existing
  toggle. Installation goes through the same validated repository code as the
  CLI.
- A repository unit test for `diagnose` and a GUI state test that loads
  `packages/community-index.toml` and installs its theme package.

### Changed

- Workspace version 0.21.0; `PKGBUILD` `pkgver` follows it.

## [0.20.0] - 2026-11-19

### Added

- `mako` integration: Mako notification configuration with `[colors]`, outline
  and per-urgency sections, theme border size/radius and interface font, plus a
  `makoctl reload` reload command. This gives Hyprland users a notification
  daemon alternative to Dunst.
- `wofi` integration: Wofi launcher stylesheet covering the window, input,
  entries, selection, scroll container, inner box and tooltips, using the theme
  colours, typography and geometry.
- Profile application now writes enabled extensions in the same transaction as
  the integrations and layout snippet (`apply_profile_with_extensions`), so a
  profile switch and its third-party styling roll back together. Covered by a
  new core test that applies and restores an extension file.
- 4 integration tests (rendered output and rejected incomplete output) and the
  apply/listing end-to-end tests now cover both applications.

### Changed

- Workspace version 0.20.0; `PKGBUILD` `pkgver` follows it. `optdepends` now
  mentions mako and wofi.

## [0.19.0] - 2026-11-19

### Added

- New `desktop/repository` crate: a community repository client for themes and
  extensions. An index is a TOML file (`[repository]` plus `[[package]]`
  entries) whose package sources are local paths, `file://` URLs or absolute
  paths; the index itself can be a local directory or a git URL.
- Index validation: schema version, non-empty name/description, valid package
  ids, known `kind` (`theme` | `extension`), unique ids, empty sources and empty
  fields are all rejected with actionable messages.
- Installation reuses the existing validators: themes are loaded through the
  theme registry and written as normalised user themes, extensions go through
  `ExtensionRegistry::import`, so a repository can never install content that
  the manual path would refuse. Existing installs require `--force`.
- `repository list|search|info|install|sync` with `--index <path|git-url>` or
  `MY_LINUX_REPOSITORY`; fetched indexes are cached under
  `$XDG_CACHE_HOME/my-linux/repository` (`MY_LINUX_REPOSITORY_CACHE` overrides
  it) and `git clone`/`git pull` is used for remote locations.
- `packages/community-index.toml`: a working example index for the built-in
  Futuristic theme and the SwayNC extension.
- 7 repository tests (index parsing, source resolution, install, reinstall,
  fetch, missing location) and 3 CLI end-to-end tests.

### Changed

- Workspace version 0.19.0; `PKGBUILD` `pkgver` follows it.
- Phase 11 now covers community themes and extensions through a repository
  index; only a hosted marketplace service remains out of scope.

## [0.18.0] - 2026-11-19

### Added

- Expanded theme schema: optional `[typography]` (`font_family`, `mono_family`,
  `font_size`) and `[metrics]` (`radius`, `spacing`, `border_width`, `opacity`)
  sections in `colors/palette.toml`, with defaults, parent inheritance and range
  validation (font size 6–48, radius 0–32, spacing 0–48, border 0–8, opacity
  20–100). Themes remain pure data and need no recompilation.
- Integrations read the tokens instead of hard-coded values: Waybar (font family,
  size, radius), Rofi (radius, spacing), Hyprland (rounding, border size, active
  opacity), Kitty (font family, size, padding), Dunst (font, corner radius) and
  Hyprlock (font family).
- `my-linux-theme info` prints typography and metrics; theme import and export
  preserve both sections, and a wallpaper-generated theme keeps the defaults.
- GUI theme editor gained interface/monospace font, font size, corner radius and
  spacing controls; saved themes round-trip all design tokens.
- GUI application manager gained an Extensions section: discovery, detection,
  applied state, enable/disable and removal of user extensions. The GUI applies
  enabled extensions in the same transaction as the CLI.
- Built-in Futuristic and Cyberpunk palettes now declare their design tokens.
- 4 theme-engine tests for token loading, inheritance and range rejection, and a
  GUI state test covering design-token round-trip and extension management.

### Changed

- Workspace version 0.18.0; `PKGBUILD` `pkgver` follows it.

## [0.17.0] - 2026-11-19

### Added

- New `desktop/extensions` crate: third-party application integrations defined
  entirely in data. An extension is a directory with an `extension.toml`
  manifest and a template rendered from the central theme palette.
- Manifest schema v1 with `id`, `name`, `version`, `description`, `format`,
  `destination`, `template`, `programs` and `reload` commands. Validation rejects
  schema mismatches, id/directory mismatches, empty metadata, invalid versions,
  unknown formats, absolute or `..` destinations, destinations inside the
  `state/` and `backups/` directories, and templates outside the extension.
- Template rendering for `{{colors.<name>}}` (all fourteen palette colours),
  `{{theme.id}}`, `{{theme.name}}`, `{{theme.version}}`, `{{theme.description}}`
  and `{{theme.author}}`. Unknown, empty or unterminated placeholders are errors.
- Output validation per declared format: TOML is parsed, CSS and RASI must have
  balanced braces, INI and text must not be empty.
- Discovery from a built-in directory and `$XDG_CONFIG_HOME/my-linux/extensions`
  (`MY_LINUX_USER_EXTENSIONS` overrides it), user extensions shadowing built-ins
  with the same id, and broken extensions reported without breaking the listing.
- Enabled state in `extensions.toml` next to the user root. Enabled extensions are
  rendered into the same transaction as the built-in integrations, so their files
  are journalled, backed up, rolled back and restored like any other generated
  file, and two extensions writing the same destination is refused.
- `extensions list|info|render|create|import|export|remove|enable|disable|status`
  and `extensions apply <theme>`. `apply` and `wallpaper palette --apply` now also
  write enabled extensions and report them.
- Built-in example extension `extensions/swaync` producing `swaync/style.css` with
  a `swaync-client --reload-css` reload command.
- The installer now installs built-in extensions into
  `<prefix>/share/my-linux/extensions`, records them in the manifest, preserves
  user-modified copies on upgrade and removes them on uninstall. `packaging/install.sh`,
  `packaging/build-release.sh` and the `PKGBUILD` ship the extension directory.
- 14 extension unit tests (manifest validation, rendering, registry discovery,
  enable state, import/export/remove, scaffolding) and 5 CLI end-to-end tests.

### Changed

- Workspace version 0.17.0; `PKGBUILD` `pkgver` follows the workspace version.
- Phase 11 (extension ecosystem) is complete apart from community hosting
  (theme repository and marketplace), which needs a server that is out of scope.

## [0.16.0] - 2026-11-19

### Added

- New `desktop/installer` crate with install, upgrade, verify and uninstall
  operations, a versioned manifest, host detection and a dependency report.
- `install [--prefix <dir>] [--source <dir>] [--force] [--dry-run]` copies both
  binaries, built-in themes, built-in profiles, a desktop entry and the icon into
  a prefix (default `~/.local`), then verifies every copied file.
- `install-manifest.toml` records the version, platform, install time and a
  digest for every installed file.
- `install-status [--prefix <dir>]` shows the installed version, verifies all
  files against their digests and reports whether an upgrade is available.
- `upgrade [--prefix <dir>] [--dry-run]` replaces binaries and only replaces
  built-in themes/profiles whose digest still matches the manifest; content the
  user edited is preserved and reported.
- `uninstall [--prefix <dir>] [--dry-run]` removes exactly the recorded program
  files and never touches user themes, user profiles, state or original
  configuration backups.
- Host detection from `/etc/os-release` plus architecture and session, and a
  runtime dependency report covering every integration, `hyprctl` and `pacman`.
- Installed binaries now resolve their shipped themes and profiles at runtime
  (`<prefix>/share/my-linux`, `MY_LINUX_DATA_DIR`, `MY_LINUX_THEMES_DIR`,
  `MY_LINUX_PROFILES_DIR`), so an installation no longer depends on the build
  tree.
- Packaging: `packaging/PKGBUILD`, `packaging/my-linux.desktop`,
  `packaging/my-linux.svg`, `packaging/install.sh` (build and install from a
  checkout) and `packaging/build-release.sh` (release tarball).
- 9 installer tests (install, upgrade preservation, uninstall, verification,
  dry run, refusal cases) and 2 CLI end-to-end tests.

### Changed

- The CLI gained the installation command group; the desktop entry is generated
  with the absolute GUI path so it works from any prefix.
- Phase 10 of the roadmap is complete.

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
