# MY LINUX
## Master Product Development Plan & Engineering Roadmap

**Project:** My Linux  
**Repository:** https://github.com/sairajKhilariArch/my-linux  
**Primary Platform:** Arch Linux  
**Desktop Environment:** Hyprland / Wayland  
**Core Language:** Rust  
**Theme Definition:** TOML  
**Initial Interface:** CLI  
**Future Interface:** Native graphical desktop application  
**Development Branch:** main  
**Project Type:** Independent Linux Desktop Customization Platform

**Status:** Core foundation implemented; application integration and platform development in progress.

---

# 1. PRODUCT VISION

We are building an independent Linux desktop customization platform that allows users to control the appearance, configuration and behavior of their Linux desktop from one central application.

This is not supposed to be another collection of CSS themes.

It should become a complete desktop customization system that manages:

- Desktop themes and color palettes.
- Window manager appearance.
- Status bars and application launchers.
- Terminal appearance.
- GTK and Qt application styling.
- Wallpapers and dynamic colors.
- Icons, cursors and fonts.
- Notification styling.
- Lock screen appearance.
- Application configuration profiles.
- Desktop layouts and presets.
- Installation, backup and restoration.
- Theme switching and application reloads.
- User-created themes and extensions.

The central concept:

**One platform. One configuration system. One theme source of truth. Complete desktop customization.**

A user should not need to manually edit ten different configuration files to customize their desktop.

For example:

```bash
my-linux-theme preview cyberpunk
my-linux-theme apply cyberpunk
my-linux-theme status
my-linux-theme restore
```

Eventually, users should also be able to perform these operations through a graphical application.

---

# 2. PRODUCT GOALS

## 2.1 Independence

The platform must operate independently of HyDE and other existing rice frameworks.

HyDE may remain installed during development, but our implementation must not depend on:

- HyDE theme management.
- HyDE internal scripts.
- HyDE-generated configuration logic.
- HyDE-specific directories.
- HyDE theme installation mechanisms.

We will build our own configuration management, rendering, theme management and application integration.

The final system must be usable on a clean Arch Linux installation without HyDE.

## 2.2 Complete desktop customization

The project should eventually control the complete visual experience of a supported Linux desktop.

This includes more than changing colors.

It should manage application configurations, appearance, layouts, wallpapers, typography, icons, desktop behavior and user profiles.

## 2.3 Safety

The platform must never destroy a user's existing configuration without a recoverable backup.

All installation operations should have:

- Path validation.
- Backup creation.
- Atomic file replacement.
- Error handling.
- Installation verification.
- Rollback.
- Recovery after interrupted operations.

## 2.4 Portability

The application should be installable on other Arch Linux systems.

We should eventually support:

- Standard Arch Linux.
- Arch Linux with Hyprland.
- Clean installations without HyDE.
- Other compatible Wayland environments where integrations permit.

Additional distributions may be considered later.

## 2.5 Performance

The core engine should be lightweight.

Rust should handle configuration loading, validation, rendering, installation and state management.

The application should avoid unnecessary background processes and excessive resource usage.

---

# 3. COMPLETE FEATURE SCOPE

The product will have the following major systems.

## 3.1 Theme Management System

The theme engine is the central component.

It must support:

- Built-in themes.
- User-created themes.
- Theme discovery.
- Theme metadata.
- Color palettes.
- Theme validation.
- Theme installation and removal.
- Theme import and export.
- Theme versioning.
- Theme inheritance.
- Theme preview.
- Theme switching.
- Theme profiles.

Initial themes:

1. Futuristic — Neon Horizon.
2. Minimal.
3. Cyberpunk.
4. Nature.

The palette should be the single source of truth.

Example:

```toml
[metadata]
name = "Cyberpunk"
version = "1.0.0"
author = "My Linux"

[colors]
background = "#0C0812"
surface = "#1E0F23"
surface_alt = "#30152F"
primary = "#FF235A"
secondary = "#00E6FF"
text = "#FFF0FA"
text_muted = "#B48CB4"
border = "#592B50"
success = "#39FF88"
warning = "#FFE14A"
error = "#FF235A"
```

The schema should eventually support additional properties:

- Typography.
- Spacing.
- Borders.
- Corner radius.
- Shadows.
- Opacity.
- Component states.
- Application-specific overrides.

A theme must be installable without modifying Rust source code.

## 3.2 Desktop Application Integrations

Each supported application will have its own integration module.

### Priority 1 — Core Hyprland desktop

| Component | Responsibilities |
|---|---|
| Hyprland | Window borders, active/inactive colors, decorations and supported settings |
| Waybar | Bar background, modules, text, icons and states |
| Rofi | Launcher, menus, dialogs and themes |
| Kitty | Terminal colors, fonts and appearance |
| Notifications | Notification styling |
| Lock screen | Lock screen appearance |

### Priority 2 — Application styling

| Component | Responsibilities |
|---|---|
| GTK | GTK application appearance |
| Qt | Qt application styling |
| File manager | Supported file manager configuration |
| Browser | Supported browser appearance settings |
| Shell | Zsh prompt and terminal environment |
| Editor | Supported editor appearance configuration |

### Priority 3 — Additional integrations

- Application launchers.
- Widgets.
- System monitors.
- Screenshots and screen recording tools.
- Clipboard managers.
- Notification extensions.
- Other Wayland applications.

Each integration must be independently installable and testable.

The platform must detect when an application is unavailable rather than failing the entire theme installation.

## 3.3 Wallpaper Management

Wallpaper support should become a first-class feature.

Planned capabilities:

- Wallpaper library.
- Wallpaper preview.
- Wallpaper selection.
- Wallpaper installation.
- Wallpaper switching.
- Per-profile wallpapers.
- Automatic wallpaper restoration.
- Wallpaper-to-palette extraction.
- Integration with supported wallpaper applications.

Future feature:

A user selects a wallpaper, and My Linux generates a matching color palette.

The generated palette should be previewed and approved before being applied.

## 3.4 Typography, Icons and Cursors

The platform should manage:

- Font families.
- Font sizes.
- Terminal fonts.
- Interface typography.
- Icon themes.
- Cursor themes.
- Cursor sizes.
- Application-specific font overrides.

These settings should be part of a desktop profile rather than independent manual operations.

## 3.5 Desktop Layout and Presets

We should support complete desktop presets.

Examples:

**Cyberpunk Developer**
- Dark neon palette.
- Matching Waybar.
- Matching Rofi.
- Terminal styling.
- Development-oriented layout.
- Matching wallpaper.

**Minimal Workspace**
- Clean colors.
- Minimal bar.
- Simple launcher.
- Reduced visual clutter.

**Nature**
- Nature-inspired palette.
- Matching wallpaper.
- Green and earth-tone styling.

**Gaming**
- Gaming-oriented appearance.
- Suitable layouts.
- Reduced visual distractions.

A preset should describe the desired desktop configuration and its dependencies.

## 3.6 Configuration Management

This is one of the most important systems.

It must handle:

- Configuration discovery.
- Path validation.
- Configuration templates.
- File installation.
- Backups.
- Restoration.
- Rollback.
- File permissions.
- Symbolic link handling.
- Existing configuration detection.
- Conflict resolution.
- Installation history.
- Interrupted operations.

The system should never blindly overwrite the user's original files.

For multiple application configurations, we should implement transaction-style application:

1. Prepare every generated configuration.
2. Validate all outputs.
3. Create required backups.
4. Install the configurations.
5. Verify the results.
6. Commit the new active state.
7. Roll back if a required operation fails.

A failed theme application should not leave half the desktop using one theme and half using another.

## 3.7 Application Reload Management

Generating configuration files does not automatically update running applications.

We need a dedicated reload system.

Examples:

- Waybar: controlled restart.
- Hyprland: supported reload mechanism.
- Kitty: supported configuration reload.
- Rofi: new configuration used on next launch.
- GTK: application/session-specific handling.
- Notifications: daemon-specific reload or restart.

Reload handlers must be application-specific.

The system must not restart unrelated desktop processes.

Users should be able to choose whether to reload immediately or apply changes later where supported.

## 3.8 Theme Profiles and Presets

Users should be able to save multiple complete desktop configurations.

Examples:

- Development.
- Gaming.
- Work.
- Minimal.
- Night.
- Custom.

A profile should contain references to:

- Theme.
- Wallpaper.
- Application configurations.
- Fonts.
- Icons.
- Cursors.
- Desktop layout.
- Enabled integrations.

Switching profiles should apply the complete configuration safely.

## 3.9 User Theme Creation

Users should be able to create custom themes without Rust programming knowledge.

The future system should provide:

- Theme templates.
- TOML validation.
- Color editor.
- Preview.
- Import/export.
- Duplicate theme.
- Theme metadata editing.
- Application-specific overrides.

The GUI should eventually include a visual theme editor.

## 3.10 Plugin and Extension Architecture

New integrations should be added without rewriting the entire application.

For example, a developer should be able to add a new application renderer through a defined integration interface.

The architecture should separate:

- Theme data.
- Application integration.
- Configuration rendering.
- Installation.
- Reloading.

Third-party extensions may be considered after the core architecture is stable.

---

# 4. TECHNICAL ARCHITECTURE

The project should follow a modular Rust workspace.

Proposed architecture:

```text
my-linux/
│
├── Cargo.toml
├── Cargo.lock
├── README.md
│
├── desktop/
│   ├── core/
│   ├── theme-engine/
│   ├── config-manager/
│   ├── components/
│   ├── integrations/
│   │   ├── waybar/
│   │   ├── rofi/
│   │   ├── hyprland/
│   │   ├── kitty/
│   │   ├── gtk/
│   │   ├── qt/
│   │   ├── notifications/
│   │   └── lockscreen/
│   │
│   ├── wallpaper-manager/
│   ├── profile-manager/
│   ├── reload-manager/
│   ├── state-manager/
│   ├── cli/
│   └── gui/
│
├── themes/
│   ├── futuristic/
│   ├── minimal/
│   ├── cyberpunk/
│   └── nature/
│
├── profiles/
├── configs/
├── wallpapers/
├── packages/
├── scripts/
├── system/
├── docs/
└── tests/
```

This is the target architecture, not a requirement to create every directory immediately.

Existing modules should be retained and evolved rather than unnecessarily rewritten.

## Module responsibilities

**Theme Engine**

Loads, validates and manages theme definitions.

**Desktop Core**

Coordinates desktop customization operations.

**Config Manager**

Handles safe file operations, installation and recovery.

**Integrations**

Provide application-specific rendering, installation and reload logic.

**Wallpaper Manager**

Manages wallpaper assets and wallpaper integrations.

**Profile Manager**

Manages complete desktop profiles and presets.

**Reload Manager**

Controls application-specific reload operations.

**State Manager**

Stores active theme, profile and installation information.

**CLI**

Provides terminal access to all platform operations.

**GUI**

Provides a graphical interface using the same core functionality.

---

# 5. CONFIGURATION AND DATA DESIGN

We should separate application files, user data, generated configurations and state.

Use the XDG directory standards where appropriate.

Proposed locations:

```text
~/.config/my-linux/
    settings.toml
    profiles/
    integrations.toml

~/.local/share/my-linux/
    themes/
    wallpapers/
    generated/
    templates/

~/.local/state/my-linux/
    active-theme.json
    active-profile.json
    installations/
    history/
    logs/

~/.cache/my-linux/
    previews/
    temporary/
```

System-wide installation should use appropriate system locations and permissions.

The exact directory structure must be finalized before implementing production installation.

Generated files must be distinguishable from user-edited files.

The application should preserve user changes and detect conflicts instead of silently replacing them.

---

# 6. COMMAND-LINE INTERFACE

The CLI is the first complete user interface.

Planned commands:

## Theme commands

```bash
my-linux-theme list
my-linux-theme preview cyberpunk
my-linux-theme info cyberpunk
my-linux-theme create my-theme
my-linux-theme import theme.zip
my-linux-theme export cyberpunk
my-linux-theme remove my-theme
```

## Application commands

```bash
my-linux-theme render cyberpunk
my-linux-theme apply cyberpunk
my-linux-theme restore
my-linux-theme current
my-linux-theme status
```

## Profile commands

```bash
my-linux-theme profile list
my-linux-theme profile create development
my-linux-theme profile apply development
my-linux-theme profile export development
```

## Wallpaper commands

```bash
my-linux-theme wallpaper list
my-linux-theme wallpaper preview
my-linux-theme wallpaper apply
```

## Recovery commands

```bash
my-linux-theme backup list
my-linux-theme rollback
my-linux-theme history
my-linux-theme doctor
```

Commands should have predictable behavior, clear help output and useful error messages.

We should implement only the commands backed by real functionality. Do not expose unfinished commands as if they work.

---

# 7. GRAPHICAL APPLICATION

The graphical application is a major long-term goal.

It must use the same Rust core rather than implementing a second theme engine.

## Main interface

The GUI should eventually include:

### Dashboard

- Active theme.
- Active profile.
- Wallpaper.
- Supported application status.
- Quick theme switching.
- Recent activity.

### Theme Gallery

- Theme cards.
- Color previews.
- Theme details.
- Apply and preview actions.
- Installed themes.

### Theme Editor

- Color editing.
- Typography.
- Component styling.
- Application overrides.
- Live preview.

### Applications

- List supported applications.
- Detect installed applications.
- Enable or disable integrations.
- Show configuration status.
- Apply application-specific settings.

### Wallpapers

- Wallpaper gallery.
- Preview.
- Apply.
- Extract palette.

### Profiles

- Create profiles.
- Save desktop configurations.
- Switch profiles.
- Import/export profiles.

### Recovery

- Backup history.
- Restore.
- Rollback.
- Configuration conflict detection.

The GUI should be lightweight and consistent with the desktop's visual design.

The technology choice should be finalized after the Rust core and CLI stabilize.

---

# 8. INSTALLATION AND DISTRIBUTION

The final platform should be easy to install on another Arch Linux system.

Planned distribution:

1. Development build.
2. Release binary.
3. Arch Linux package.
4. PKGBUILD.
5. AUR package.
6. Installation script.
7. Upgrade mechanism.
8. Uninstallation and recovery.

The installer should:

- Detect dependencies.
- Detect supported desktop components.
- Avoid overwriting existing files.
- Set up application directories.
- Install built-in themes.
- Provide a clean uninstall process.

Updates must preserve user-created themes, profiles and settings.

---

# 9. SECURITY AND RELIABILITY

Safety must be built into the architecture.

Required safeguards:

- Validate all user-supplied paths.
- Handle symbolic links safely.
- Avoid unsafe shell execution.
- Use atomic file operations where supported.
- Preserve permissions when required.
- Never silently replace original backups.
- Validate generated configurations.
- Keep an installation journal.
- Support recovery after interruption.
- Handle permission errors clearly.
- Verify application reload results.
- Prevent partial multi-application installations.

The system should not require root privileges for normal per-user desktop customization.

Use elevated privileges only for operations that genuinely require them.

---

# 10. TESTING STRATEGY

Every module needs unit and integration tests.

## Theme engine

- Valid TOML.
- Invalid TOML.
- Valid colors.
- Invalid colors.
- Unknown themes.
- Missing metadata.
- Custom themes.

## Configuration manager

- Backup creation.
- Backup collision.
- Installation.
- Atomic replacement.
- Restoration.
- Missing files.
- Permissions.
- Symbolic links.
- Interrupted operations.
- Concurrent operations.

## Application integrations

- Valid generated CSS.
- Valid RASI.
- Valid Hyprland snippets.
- Application-specific validation.
- Missing applications.
- Reload failures.

## Full platform

- Apply a complete theme.
- Restore the original configuration.
- Switch between themes.
- Switch between profiles.
- Recover from failed installation.
- Preserve user changes.

Required quality checks:

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

No phase should be considered complete merely because the code compiles.

Each phase needs its own acceptance tests.

---

# 11. UPDATED DEVELOPMENT ROADMAP

The roadmap is ordered by dependencies. We should not start the GUI or package distribution before the core system is reliable.

## Phase 1 — Finish the current foundation

**Current phase: In progress**

- [x] Rust workspace.
- [x] Theme engine.
- [x] TOML palette loading.
- [x] Four initial themes.
- [x] Waybar CSS renderer.
- [x] ANSI color preview.
- [x] CLI list, preview, render and export.
- [x] Configuration backup.
- [x] Safe content installation.
- [x] Atomic restoration function.
- [x] Desktop Core integration.
- [x] Initial test installation.
- [x] 23 workspace tests passing.
- [x] Cyberpunk test installation verified.
- [ ] Add CLI restore command.
- [ ] Verify restoration manually.
- [ ] Test repeated apply operations.
- [ ] Add CLI integration tests.
- [ ] Add installation failure tests.
- [ ] Improve configuration path and permission handling.

**Acceptance:** The complete CLI workflow must work inside a test directory without modifying the real desktop.

## Phase 2 — Production-ready configuration management

- [ ] Define configuration destination mapping.
- [ ] Design backup naming and retention.
- [ ] Preserve file permissions.
- [ ] Handle symlinks safely.
- [ ] Detect user modifications.
- [ ] Add installation journal.
- [ ] Implement transaction-style application.
- [ ] Implement rollback after failure.
- [ ] Add recovery and diagnostic commands.

**Acceptance:** The system can safely install, restore and recover multiple configurations.

## Phase 3 — Complete theme engine

- [ ] Dynamic theme discovery.
- [ ] Theme schema versioning.
- [ ] Theme metadata validation.
- [ ] User theme directory.
- [ ] Custom theme creation.
- [ ] Import and export.
- [ ] Theme removal.
- [ ] Theme inheritance.
- [ ] Palette generation.
- [ ] Theme profiles integration.

**Acceptance:** New themes can be added and managed without changing the Rust source code.

## Phase 4 — Complete core Hyprland integrations

Implementation order:

1. Waybar.
2. Rofi.
3. Hyprland.
4. Kitty.
5. Notifications.
6. Lock screen.

For each integration:

- [ ] Configuration template.
- [ ] Renderer.
- [ ] Output validation.
- [ ] Safe installation.
- [ ] Backup.
- [ ] Restore.
- [ ] Reload handler.
- [ ] Integration tests.

**Acceptance:** One theme applies consistently across the core desktop.

## Phase 5 — Extended application integrations

- [ ] GTK.
- [ ] Qt.
- [ ] File manager.
- [ ] Browser styling where supported.
- [ ] Zsh.
- [ ] Editor integrations.
- [ ] System monitors.
- [ ] Additional applications.

**Acceptance:** Users can enable individual application integrations without requiring every application to be installed.

## Phase 6 — Wallpapers and desktop assets

- [ ] Wallpaper manager.
- [ ] Wallpaper library.
- [ ] Wallpaper preview.
- [ ] Wallpaper profiles.
- [ ] Palette extraction.
- [ ] Font management.
- [ ] Icon themes.
- [ ] Cursor themes.
- [ ] Asset validation.

**Acceptance:** A desktop profile can control colors and associated visual assets.

## Phase 7 — Profiles and complete desktop presets

- [ ] Profile schema.
- [ ] Profile creation.
- [ ] Profile discovery.
- [ ] Profile switching.
- [ ] Profile import/export.
- [ ] Development preset.
- [ ] Gaming preset.
- [ ] Minimal preset.
- [ ] Custom presets.
- [ ] Profile rollback.

**Acceptance:** Users can switch complete desktop configurations with one command.

## Phase 8 — State, monitoring and diagnostics

- [ ] Active theme tracking.
- [ ] Active profile tracking.
- [ ] Application integration status.
- [ ] Configuration modification detection.
- [ ] Installation history.
- [ ] Backup history.
- [ ] System diagnostics.
- [ ] `doctor` command.
- [ ] Structured logging.

**Acceptance:** The application knows what is installed, what is active and what needs attention.

## Phase 9 — Graphical user interface

- [ ] GUI architecture.
- [ ] Dashboard.
- [ ] Theme gallery.
- [ ] Theme preview.
- [ ] Theme editor.
- [ ] Application manager.
- [ ] Wallpaper manager.
- [ ] Profile manager.
- [ ] Backup and recovery UI.
- [ ] Settings.
- [ ] Light and dark interface.

**Acceptance:** The full platform can be operated without using the terminal.

## Phase 10 — Packaging and release

- [ ] Release builds.
- [ ] Arch PKGBUILD.
- [ ] AUR packaging.
- [ ] Installer.
- [ ] Dependency detection.
- [ ] Upgrade handling.
- [ ] Uninstaller.
- [ ] Documentation.
- [ ] Screenshots and demonstrations.
- [ ] Release testing.

**Acceptance:** Another Arch Linux user can install, configure, update and uninstall the platform safely.

## Phase 11 — Advanced ecosystem

Long-term features:

- Community theme repository.
- Theme marketplace.
- Plugin ecosystem.
- Dynamic wallpaper palettes.
- Automatic theme generation.
- More desktop environment support.
- Advanced desktop layouts.
- Additional distribution support.

These features must not delay completion of the core platform.

---

# 12. IMMEDIATE NEXT DEVELOPMENT TASKS

Our actual next tasks are deliberately smaller than the entire roadmap.

1. Add the CLI `restore` command.
2. Verify the original test CSS is restored.
3. Test applying Cyberpunk repeatedly.
4. Test switching from Cyberpunk to Nature.
5. Verify that the first original backup is preserved.
6. Add automated CLI integration tests.
7. Improve backup and installation edge cases.
8. Finalize the configuration manager API.
9. Build the first complete Waybar integration.
10. Start the Rofi renderer.

We must complete these tasks before moving to real desktop installation.

---

# 13. PROJECT COMPLETION CRITERIA

The project is considered mature when a new Arch Linux user can:

1. Install My Linux.
2. Discover available themes.
3. Preview themes.
4. Create custom themes.
5. Manage desktop applications.
6. Apply a complete theme.
7. Customize wallpapers, fonts, icons and cursors.
8. Create desktop profiles.
9. Switch between profiles.
10. Reload supported applications.
11. See the active configuration status.
12. Back up and restore configurations.
13. Recover from failed operations.
14. Update the platform without losing personal data.
15. Use either the CLI or GUI.

All of this must work independently of HyDE.

---

# 14. NON-NEGOTIABLE DEVELOPMENT RULES

1. Build functionality, not just documentation.
2. Do not rewrite working modules without a technical reason.
3. Inspect existing code before modifying it.
4. Preserve existing project history and assets.
5. Do not modify HyDE internals.
6. Test installation in isolated directories first.
7. Never overwrite user backups silently.
8. Keep the Rust workspace modular.
9. Avoid unnecessary dependencies.
10. Implement one complete feature at a time.
11. Add tests for every important operation.
12. Update progress based on actual implementation, not assumptions.
13. Do not claim features are complete before verifying them.
14. Keep the project usable from the CLI even after the GUI is introduced.

---

# 15. FINAL PRODUCT DIRECTION

My Linux should evolve into an independent Linux desktop customization platform, not remain a simple theme switcher.

The Rust core will manage themes, configurations, profiles, installations and recovery.

The application integrations will translate central theme data into the configuration formats used by the desktop.

The CLI will provide complete control first.

The graphical interface will eventually make the platform accessible to users who do not want to use terminal commands.

The system should be modular enough to grow, safe enough to manage real desktop configurations and portable enough to work on other Arch Linux installations.

**Our ultimate goal: Build our own independent Arch Linux desktop customization ecosystem, with complete control over the desktop appearance and configuration.**

The first priority remains finishing and hardening the existing foundation. We will expand only after each layer is tested and reliable.