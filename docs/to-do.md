
# My Linux Theme Manager

## Project Vision, Master Specification & Engineering Roadmap

**Project:** My Linux
**Platform:** Arch Linux
**Desktop Environment:** Hyprland (Wayland)
**Primary Language:** Rust
**Configuration Format:** TOML
**Repository:** https://github.com/sairajKhilariArch/my-linux
**Current Development Branch:** `main`
**Project Status:** Active Development — Foundation and Initial Theme Installation

---

# 1. Project Vision

We are building an independent, Rust-powered Linux desktop customization and theme management system.

The goal is to create a system that allows users to customize their entire Linux desktop from one central theme engine.

Instead of manually changing colors in different configuration files, users should be able to select one theme, and our system should automatically generate the correct configurations for supported desktop applications.

For example:

```bash
my-linux-theme apply cyberpunk
```

This command should eventually change the appearance of the supported desktop components to the Cyberpunk theme.

The system should be:

- Independent of HyDE.
- Built primarily in Rust.
- Modular and extensible.
- Safe when modifying configuration files.
- Portable across Arch Linux installations.
- Easy to configure and use.
- Capable of supporting multiple desktop applications.
- Designed for future graphical user interface integration.

**Our central idea: One theme, one palette, multiple applications.**

---

# 2. The Problem We Are Solving

Linux desktop customization often involves multiple independent configuration files.

For example:

| Application           | Configuration                     |
| --------------------- | --------------------------------- |
| Hyprland              | Hyprland configuration            |
| Waybar                | CSS and JSONC                     |
| Rofi                  | RASI                              |
| GTK                   | GTK CSS and settings              |
| Qt                    | Qt configuration                  |
| Terminal              | Terminal color configuration      |
| Notifications         | Notification daemon configuration |
| Application launchers | Application-specific styling      |

If a user wants a Cyberpunk appearance, they might have to manually change colors in all these places.

This creates several problems:

1. The same color must be entered multiple times.
2. Applications can end up with inconsistent colors.
3. Changing themes becomes repetitive.
4. Existing configurations can accidentally be overwritten.
5. Restoring the original appearance can be difficult.
6. Different applications use different configuration formats.

We want to solve these problems through a centralized theme management system.

---

# 3. What We Are Building

The system will contain several independent modules.

## 3.1 Theme Engine

This is the heart of the project.

Its responsibility is to load, validate, manage and provide theme information.

Themes will be stored in TOML files.

Example:

```toml
[metadata]
name = "Cyberpunk"
author = "My Linux"
description = "Neon-inspired dark theme"

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

The palette will be the single source of truth.

This means applications should receive their colors from the same theme instead of maintaining separate color definitions.

### Initial themes

We currently have four themes:

- Futuristic — Neon Horizon
- Minimal
- Cyberpunk
- Nature

Future themes should be installable without changing the theme engine's core logic.

## 3.2 Configuration Renderer

Different applications use different configuration formats.

The renderer converts the centralized theme palette into application-specific configurations.

Planned renderers:

| Application         | Output                             |
| ------------------- | ---------------------------------- |
| Waybar              | CSS                                |
| Rofi                | RASI                               |
| Hyprland            | Configuration snippets             |
| GTK                 | CSS/settings                       |
| Qt                  | Supported Qt theme configuration   |
| Kitty               | Terminal configuration             |
| Notification daemon | Application-specific configuration |

Each renderer should be independent.

For example:

```text
Cyberpunk palette
       |
       v
   Theme Engine
       |
       +------> Waybar CSS
       |
       +------> Rofi RASI
       |
       +------> Hyprland config
       |
       +------> GTK styling
       |
       +------> Terminal colors
```

The renderer must generate valid configuration files without manually modifying the original theme palette.

## 3.3 Configuration Manager

This module is responsible for safely installing, backing up and restoring configuration files.

Its main responsibilities:

- Validate paths.
- Create backups.
- Avoid overwriting existing backups.
- Install generated configurations.
- Restore previous configurations.
- Handle file errors.
- Prevent partially written configuration files.
- Handle concurrent operations safely.

An important design decision is that configuration installation should use temporary files and atomic replacement wherever supported.

We have already implemented the foundation of this module.

## 3.4 Desktop Core

This module connects the theme engine and configuration manager.

Its responsibility is to coordinate operations.

For example:

```text
User selects Cyberpunk
          |
          v
      Desktop Core
          |
          v
       Load Theme
          |
          v
    Generate Waybar CSS
          |
          v
    Create Backup
          |
          v
    Install CSS
```

The desktop core should eventually manage multiple application configurations through a common interface.

## 3.5 Command-Line Interface

The CLI will be the primary interface during early development.

The intended command structure is:

```bash
my-linux-theme list
my-linux-theme preview cyberpunk
my-linux-theme render cyberpunk
my-linux-theme export cyberpunk
my-linux-theme apply cyberpunk
my-linux-theme restore
my-linux-theme current
my-linux-theme status
```

The exact behavior of commands such as `current` and `status` will be implemented later.

### Command responsibilities

| Command     | Responsibility                           |
| ----------- | ---------------------------------------- |
| `list`    | Show available themes                    |
| `preview` | Display theme colors                     |
| `render`  | Generate configuration output            |
| `export`  | Save generated files                     |
| `apply`   | Install a theme                          |
| `restore` | Restore previous configuration           |
| `current` | Show active theme                        |
| `status`  | Show installation and application status |

The CLI should also provide useful error messages and help information.

---

# 4. Project Architecture

Our current Rust workspace is organized into separate packages.

```text
my-linux/
│
├── Cargo.toml
├── Cargo.lock
├── README.md
│
├── desktop/
│   │
│   ├── core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── render.rs
│   │
│   ├── theme-engine/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── loader.rs
│   │
│   ├── config-manager/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── components/
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   └── cli/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
│
├── themes/
│   ├── futuristic/
│   ├── minimal/
│   ├── cyberpunk/
│   └── nature/
│
├── configs/
│   ├── waybar/
│   └── rofi/
│
├── packages/
│
├── docs/
├── scripts/
└── system/
```

### Responsibilities of each package

**theme-engine**

Loads and validates theme definitions and converts colors into usable Rust values.

**desktop-core**

Coordinates theme loading, rendering and installation operations.

**config-manager**

Handles safe file operations, backup, installation and restoration.

**components**

Will eventually contain reusable desktop components and shared integration logic.

**cli**

Provides the user-facing terminal commands.

**themes**

Contains all theme definitions and generated theme assets.

**configs**

Contains reference and application configuration templates.

**docs**

Contains architecture documents, guides and development documentation.

---

# 5. Development Principles

These rules will guide the entire project.

## 5.1 Independence from HyDE

HyDE must not be a dependency of our theme engine.

We can continue using HyDE temporarily on the development desktop, but our application must own its own theme logic.

We must not depend on:

```text
~/.local/lib/hyde
~/.config/hyde
```

We must also avoid blindly modifying generated HyDE configuration files.

Our system should eventually work on a clean Arch Linux installation without HyDE.

## 5.2 One source of truth

The theme palette is the central source of color information.

We should not hardcode different color palettes into individual renderers.

## 5.3 Safe configuration management

Never blindly overwrite user configurations.

Before installation:

1. Check the destination.
2. Determine whether it already exists.
3. Create a backup when necessary.
4. Install atomically.
5. Verify the operation.
6. Preserve the original backup.

The user should always have a recovery mechanism.

## 5.4 Modular architecture

Each application integration should be separate.

If we add a new application, we should not have to rewrite the theme engine.

## 5.5 Test before using real configurations

All new installation and restoration functionality must first be tested inside a temporary project directory.

Only after successful testing should we consider installing into actual desktop configuration directories.

---

# 6. Current Development Progress

The following work has already been completed or verified during development.

## 6.1 Theme Engine — Implemented

- TOML theme loading.
- Theme metadata parsing.
- Hexadecimal color parsing.
- Invalid color validation.
- Named theme loading.
- Four theme definitions.
- Theme color conversion to hexadecimal.

## 6.2 Rendering — Initial Implementation

- Waybar CSS renderer.
- Generated CSS files.
- Theme-based CSS color values.
- Cyberpunk rendering tests.

More application renderers are still required.

## 6.3 Configuration Manager — Foundation Implemented

- Path validation.
- Backup creation.
- Protection against overwriting existing backups.
- Temporary-file-based installation.
- Atomic replacement.
- Direct content installation.
- Concurrent backup protection.
- Restoration using atomic installation.

The restore function has been updated to read the backup and reuse `install_content()`.

## 6.4 Desktop Core — Initial Implementation

- Theme loading integration.
- Waybar CSS generation.
- Waybar installation.
- Automatic original-file backup.

## 6.5 CLI — Initial Implementation

Currently implemented commands:

```bash
my-linux-theme list
my-linux-theme preview cyberpunk
my-linux-theme render cyberpunk
my-linux-theme export cyberpunk
my-linux-theme apply cyberpunk
```

The `apply` command currently installs into:

```text
target/theme-test/
```

It does not modify the actual Waybar configuration.

The `restore` operation exists in the configuration manager, but the CLI `restore` command is still pending implementation and verification.

## 6.6 Testing Results

The latest complete workspace test run produced:

| Package            | Tests passed |
| ------------------ | -----------: |
| config-manager     |            9 |
| desktop-components |            1 |
| desktop-core       |            4 |
| theme-engine       |            9 |
| CLI                |            0 |
| **Total**    | **23** |

**Latest result: 23 passed, 0 failed.**

The Cyberpunk test installation was also verified manually.

Generated file:

```text
target/theme-test/waybar.css
```

Backup:

```text
target/theme-test/backups/waybar.css.original
```

The generated CSS contained the expected Cyberpunk colors, and the backup contained:

```css
/* Original test Waybar configuration */
```

---

# 7. Engineering Roadmap

We will develop the project in stages.

## Phase 1 — Complete the foundation

**Status: In progress**

Tasks:

- [X] Create Rust workspace.
- [X] Implement theme engine.
- [X] Load TOML palettes.
- [X] Create four initial themes.
- [X] Implement Waybar CSS renderer.
- [X] Implement configuration backup.
- [X] Implement safe installation.
- [X] Add ANSI terminal color preview.
- [X] Implement initial CLI.
- [X] Implement atomic restoration function.
- [X] Test the workspace.
- [X] Test Cyberpunk installation in a temporary directory.
- [ ] Add CLI `restore` command.
- [ ] Verify restoration manually.
- [ ] Test repeated theme applications.
- [ ] Improve error handling and validation.
- [ ] Add CLI integration tests.

**Completion criteria:** The complete preview, render, export, apply and restore workflow must work without touching real desktop configurations.

## Phase 2 — Build a proper theme system

Tasks:

- Improve palette validation.
- Add theme discovery instead of hardcoded theme lists.
- Support user-created themes.
- Support theme installation and removal.
- Support theme metadata and versioning.
- Add theme inheritance if useful.
- Add configuration schema validation.
- Introduce theme state management.

**Completion criteria:** Users can add, remove and manage themes without modifying Rust source code.

## Phase 3 — Multiple application integrations

We will add renderers in a controlled order.

1. Waybar.
2. Rofi.
3. Hyprland.
4. Kitty.
5. GTK.
6. Qt.
7. Notification daemon.
8. Additional applications.

Each integration needs:

- A renderer.
- A configuration template where appropriate.
- Installation support.
- Backup support.
- Restoration support.
- Tests.

**Completion criteria:** A single theme selection produces consistent configurations for all supported applications.

## Phase 4 — Application lifecycle and reload management

Generating configuration files is not enough.

Applications may need to reload or restart before their new appearance becomes visible.

We will implement application-specific reload handlers.

Examples:

| Application | Reload strategy                            |
| ----------- | ------------------------------------------ |
| Waybar      | Controlled restart                         |
| Hyprland    | Supported reload mechanism                 |
| Rofi        | Use generated configuration on next launch |
| Kitty       | Supported configuration reload             |
| GTK         | Application/session-specific handling      |

We must avoid restarting unrelated desktop processes.

**Completion criteria:** Applying a theme updates supported applications without requiring a complete desktop restart whenever possible.

## Phase 5 — Theme state and recovery

We need to know which theme is currently installed.

Planned features:

- Store active theme information.
- Track generated configuration files.
- Track backup locations.
- Detect missing or modified configurations.
- Provide status information.
- Support rollback.
- Handle interrupted installations.

Possible future commands:

```bash
my-linux-theme current
my-linux-theme status
my-linux-theme rollback
```

**Completion criteria:** The system can reliably identify its active theme and recover from failed operations.

## Phase 6 — Graphical User Interface

After the CLI and core engine are stable, we can build a graphical application.

Possible features:

- Theme gallery.
- Color previews.
- Live theme preview.
- Theme selection.
- Apply and restore buttons.
- Application integration settings.
- Backup management.
- Light and dark interface.
- Custom theme editor.

The GUI should communicate with the same Rust core.

We should not create a second independent theme engine for the GUI.

**Completion criteria:** The user can manage the entire desktop theme through a graphical interface or the CLI.

## Phase 7 — Packaging and Distribution

We want the project to work on other Arch Linux systems.

Planned work:

- Build release binaries.
- Create an Arch package.
- Prepare AUR packaging.
- Create an installation script.
- Manage application dependencies.
- Support clean installation and upgrades.
- Provide uninstall and recovery functionality.
- Document supported desktop environments.

**Completion criteria:** Another Arch Linux user can install and use the project without manually building its internal modules.

---

# 8. Future Features

These are longer-term possibilities, not current commitments.

### Theme marketplace or community repository

Allow users to share custom themes.

### Wallpaper integration

Automatically synchronize theme palettes with selected wallpapers.

### Dynamic color extraction

Extract colors from an image and generate a compatible desktop palette.

### Theme profiles

Allow users to maintain different configurations for different situations.

Examples:

- Work
- Gaming
- Development
- Minimal
- Night mode

### Plugin architecture

Allow new application integrations to be added independently.

### Cross-desktop support

Potentially support other Wayland desktop environments and window managers in the future.

---

# 9. Testing and Quality Requirements

Every major feature must have tests.

Our testing strategy should include:

**Unit tests**

Test individual functions such as color parsing, TOML loading, CSS rendering and path validation.

**Integration tests**

Test complete operations such as loading a theme, rendering CSS and installing the output.

**Failure tests**

Test missing files, invalid palettes, permission errors, existing backups and failed installation.

**Recovery tests**

Test restoration after installation and failure scenarios.

**Platform tests**

Verify behavior on supported Linux filesystems and desktop configurations.

We should also run:

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

before major releases.

---

# 10. Security and Safety Requirements

This project modifies desktop configuration files, so safety is a core requirement.

The implementation must:

- Never use arbitrary shell commands for simple file operations.
- Validate paths before modifying files.
- Avoid following unexpected symbolic links where security matters.
- Never overwrite a user's original backup silently.
- Avoid modifying HyDE internals.
- Use atomic writes where supported.
- Preserve file permissions where required.
- Provide understandable error messages.
- Keep a recovery path.
- Never apply untested configurations automatically.

We must also consider rollback when an installation modifies several application configurations and one operation fails.

A future multi-application installation should ideally behave like a transaction: either all required changes succeed, or the system restores the previous state.

---

# 11. Immediate Development Plan

Our next development tasks are:

| Order | Task                                  | Status  |
| ----- | ------------------------------------- | ------- |
| 1     | Implement CLI restore command         | Pending |
| 2     | Test restoration of original CSS      | Pending |
| 3     | Test repeated theme applications      | Pending |
| 4     | Add CLI integration tests             | Pending |
| 5     | Implement theme discovery             | Pending |
| 6     | Improve configuration manager         | Pending |
| 7     | Add Rofi renderer                     | Pending |
| 8     | Add Hyprland renderer                 | Pending |
| 9     | Add active-theme state                | Pending |
| 10    | Implement application reload handlers | Pending |

We should complete each task and verify it before moving to the next one.

---

# 12. Definition of the Finished Project

The long-term goal is a complete desktop theme management system.

A user should be able to install the project and run:

```bash
my-linux-theme list
```

Choose a theme:

```bash
my-linux-theme preview cyberpunk
```

Apply it:

```bash
my-linux-theme apply cyberpunk
```

Check the result:

```bash
my-linux-theme status
```

Restore the previous configuration:

```bash
my-linux-theme restore
```

Behind the scenes, the application should manage all supported application configurations, backups, installation, state and reload operations.

The user should not need to manually edit multiple CSS, RASI, TOML or Hyprland configuration files.

---

# 13. Final Project Direction

We are not simply creating a collection of Linux themes.

We are creating a modular Linux desktop customization platform.

Rust will provide the core engine, configuration management, safety and application integration.

TOML will provide readable theme definitions.

The CLI will provide the first interface.

A graphical application can be built on top of the same core later.

Our most important priorities are:

1. Correct architecture.
2. Safe configuration management.
3. Reliable theme generation.
4. Independent application integrations.
5. Recovery and rollback.
6. Portability.
7. A polished user experience.

**Development philosophy: Build the foundation correctly, test every operation in isolation, and only then connect it to the real desktop.**

The current development environment must remain safe. HyDE should not be modified or removed until our independent theme management system is sufficiently complete and tested.
