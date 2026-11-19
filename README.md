# My Linux

My Linux is an independent Arch Linux desktop customisation platform. One TOML
palette is the source of truth; the Rust core renders it into the configuration
format of every supported desktop application.

The project does not depend on HyDE or any other rice framework.

**Status:** Phases 1–4 are complete: foundation, production configuration
management, the dynamic theme engine, and the core Hyprland integrations
(Waybar, Rofi, Hyprland, Kitty, notifications, lock screen). The CLI writes into
an isolated *installation root* that you choose, so it never touches your live
`~/.config` yet. Profiles, wallpapers, fonts/icons, the GUI and packaging are not
implemented — see [`docs/to-do.md`](docs/to-do.md) and
[`CHANGELOG.md`](CHANGELOG.md).

## What works today

| Area | State |
|---|---|
| Theme discovery, validation, schema versioning, inheritance | Implemented |
| Theme creation, import, export, removal, palette generation | Implemented |
| Waybar, Rofi, Hyprland, Kitty, Dunst, Hyprlock rendering | Implemented |
| GTK 3/4, Qt, Zsh, Neovim, Yazi, Btop rendering | Implemented |
| Generated-output validation per application | Implemented |
| Application detection and per-application reload commands | Implemented |
| Multi-file transactional apply with backup and rollback | Implemented |
| First-run original backup, retained forever | Implemented |
| `restore`, `rollback`, journal recovery | Implemented |
| Modification detection and refusal to delete hand-edited files | Implemented |
| `status`, `history`, `backup list`, `doctor`, `integrations status` | Implemented |
| Path validation, permission preservation, symlink-safe writes | Implemented |
| Browser styling | Not implemented (no stable format) |
| Profiles, wallpapers, fonts/icons/cursors, GUI, packaging | Not implemented |

## Build and verify

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Unit tests cover every crate; `desktop/cli/tests/cli.rs` runs the binary end to
end in temporary roots and user theme directories.

## Command line

```bash
cargo build --workspace

# Themes
./target/debug/my-linux-theme list
./target/debug/my-linux-theme info cyberpunk
./target/debug/my-linux-theme preview cyberpunk
./target/debug/my-linux-theme render cyberpunk --integration rofi
./target/debug/my-linux-theme create my-theme --from cyberpunk
./target/debug/my-linux-theme import ./theme.toml --as imported
./target/debug/my-linux-theme export my-theme ./my-theme.toml
./target/debug/my-linux-theme remove my-theme

# Desktop
./target/debug/my-linux-theme apply cyberpunk
./target/debug/my-linux-theme apply nature --integration kitty
./target/debug/my-linux-theme apply cyberpunk --reload
./target/debug/my-linux-theme integrations list
./target/debug/my-linux-theme status
./target/debug/my-linux-theme rollback
./target/debug/my-linux-theme restore

# Recovery
./target/debug/my-linux-theme history
./target/debug/my-linux-theme backup list
./target/debug/my-linux-theme doctor
```

`--root <directory>` picks the installation root (default `$MY_LINUX_ROOT`,
otherwise `target/theme-test`). `--integration <id>` limits an apply to the named
integrations. `--reload` reloads running applications that are detected; without
it no process is touched.

## Integrations

| Id | Application | Generated file | Reload |
|---|---|---|---|
| `waybar` | Waybar | `waybar.css` | `killall -SIGUSR2 waybar` |
| `rofi` | Rofi | `rofi/theme.rasi` | applied on next launch |
| `hyprland` | Hyprland | `hyprland/my-linux.conf` | `hyprctl reload` |
| `kitty` | Kitty | `kitty/my-linux.conf` | `pkill -USR1 -x kitty` |
| `dunst` | Notifications | `dunst/dunstrc` | `dunstctl reload` |
| `hyprlock` | Lock screen | `hyprlock/hyprlock.conf` | applied on next lock |
| `gtk3` | GTK 3 | `gtk-3.0/my-linux.css` | applied on next start |
| `gtk4` | GTK 4 | `gtk-4.0/my-linux.css` | applied on next start |
| `qt` | Qt (qt6ct/qt5ct) | `qt6ct/colors/my-linux.conf` | applied on next start |
| `zsh` | Zsh prompt | `zsh/my-linux.zsh` | new shell |
| `neovim` | Neovim | `nvim/lua/my-linux.lua` | applied on next start |
| `yazi` | Yazi file manager | `yazi/theme.toml` | applied on next start |
| `btop` | Btop system monitor | `btop/themes/my-linux.theme` | applied on next start |

Each integration renders from the same palette, validates its own output and is
installed independently. Every file is a snippet meant to be imported by the
user's own configuration — My Linux never rewrites those files:

```conf
# ~/.config/hypr/hyprland.conf
source = ~/.config/hypr/my-linux.conf

# ~/.config/kitty/kitty.conf
include ~/.config/kitty/my-linux.conf
```

```css
/* ~/.config/gtk-3.0/gtk.css and ~/.config/gtk-4.0/gtk.css */
@import url("my-linux.css");
```

```zsh
# ~/.zshrc
source ~/.config/zsh/my-linux.zsh
```

```lua
-- ~/.config/nvim/init.lua
require("my-linux")
```

Qt needs `QT_QPA_PLATFORMTHEME=qt6ct` and the generated scheme selected in
qt6ct. Btop needs `color_theme = "my-linux"` in `btop.conf`.

An application that is not installed simply never reads its generated file;
detection only decides whether a reload is attempted.

## Themes

A theme is a TOML palette. Built-in themes live in `themes/`, user themes in
`$MY_LINUX_USER_THEMES` (default `$XDG_DATA_HOME/my-linux/themes`,
usually `~/.local/share/my-linux/themes`). User themes are searched first.

```toml
[theme]
schema = 1
name = "Cyberpunk"
id = "cyberpunk"
version = "1.0.0"
description = "High contrast neon interface"
author = "My Linux"
# extends = "nature"          # optional: inherit and override

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

[colors.states]
active = "#FF235A"
hover = "#00E6FF"
selected = "#451B48"
```

Accepted layouts: `themes/<id>/colors/palette.toml` or `themes/<id>.toml`.

Validation rules:

- `id` uses lowercase letters, digits, `-` or `_`, and must match its directory.
- `name`, `version` (`MAJOR.MINOR.PATCH`) and `description` are required.
- `schema` must be `1`; a newer schema is rejected with a clear error.
- All eleven colours and three states are required after inheritance.
- `extends` must name an existing theme; cycles are rejected.
- Invalid themes are listed by `list` and reported by `doctor`.

## Installation root layout

```text
<root>/
├── waybar.css
├── rofi/theme.rasi
├── hyprland/my-linux.conf
├── kitty/my-linux.conf
├── dunst/dunstrc
├── hyprlock/hyprlock.conf
├── backups/<file>.original            # first original configuration, never overwritten
├── backups/<file>.previous            # state before the last operation (rollback)
└── state/
    ├── active-theme
    ├── installed.tsv                  # installed files and their digests
    ├── journal.tsv                    # present only while an operation is in flight
    ├── previous-theme
    └── history.log
```

## Safety guarantees

- Every apply is one transaction across all selected integrations: render,
  validate, journal, back up, install atomically, commit — or roll back.
- A failed or interrupted operation never leaves a half-themed desktop; the
  journal is replayed on the next command.
- The first original configuration of each file is backed up once and never
  replaced. Files that My Linux generated are never mistaken for user originals.
- Replacing a configuration keeps its file permissions; symlinked destinations
  are written through and the link is preserved.
- `restore` puts originals back, removes generated files that never replaced a
  user file, and refuses to delete a file you edited by hand.
- `rollback` and `restore` refuse to run when there is nothing to restore.
- Reloads only happen with `--reload`, only for detected applications, and a
  failed reload never breaks the installation.

## Repository layout

```text
desktop/
├── core/            # registry wiring, transactions, rollback, history, diagnostics
├── theme-engine/    # discovery, schema validation, inheritance, generation
├── config-manager/  # atomic install, backups, restore, digests
├── integrations/    # one renderer per application (thirteen integrations)
├── components/      # shared desktop components (placeholder)
└── cli/             # the my-linux-theme binary and its integration tests
themes/              # built-in theme definitions
configs/             # reference configuration templates
docs/                # product roadmap
```
