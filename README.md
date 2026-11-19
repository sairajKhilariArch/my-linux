# My Linux

My Linux is an independent Arch Linux desktop customisation platform. One TOML
palette is the source of truth; the Rust core renders it into the configuration
format of every supported desktop application.

The project does not depend on HyDE or any other rice framework.

**Status:** Phases 1–11 are complete: foundation, production configuration
management, the dynamic theme engine, the core Hyprland integrations, extended
integrations, wallpapers and assets, desktop profiles, state/diagnostics, a
native GUI, installation/packaging, and the extension ecosystem. Everything
writes into an isolated *installation root* that you choose, so it never
modifies your live `~/.config`. See [`docs/to-do.md`](docs/to-do.md) and
[`CHANGELOG.md`](CHANGELOG.md).

## What works today

| Area | State |
|---|---|
| Theme discovery, validation, schema versioning, inheritance | Implemented |
| Typography and geometry tokens (font, size, radius, spacing, border, opacity) | Implemented |
| Theme creation, import, export, removal, palette generation | Implemented |
| Waybar, Rofi, Wofi, Hyprland, Kitty, Dunst, Mako, Hyprlock rendering | Implemented |
| GTK 3/4, Qt, Zsh, Neovim, Yazi, Btop rendering | Implemented |
| Generated-output validation per application | Implemented |
| Application detection and per-application reload commands | Implemented |
| Multi-file transactional apply with backup and rollback | Implemented |
| First-run original backup, retained forever | Implemented |
| `restore`, `rollback`, journal recovery | Implemented |
| Modification detection and refusal to delete hand-edited files | Implemented |
| `status`, `current`, `history`, `backup list`, `doctor`, `integrations status` | Implemented |
| Structured event log, failure records, last-action reporting | Implemented |
| Path validation, permission preservation, symlink-safe writes | Implemented |
| Browser styling | Not implemented (no stable format) |
| Wallpaper library, preview, switching, history and restore | Implemented |
| Palette extraction from a PNG/JPEG wallpaper | Implemented |
| Font, icon-theme and cursor-theme discovery and settings | Implemented |
| Desktop profiles, presets, per-profile wallpapers, profile switching | Implemented |
| Native GUI: dashboard, gallery, editor, applications, wallpapers, profiles, recovery | Implemented |
| Installation, upgrade, uninstall and verification | Implemented |
| User extensions: manifest, template, discovery, import/export, enable state | Implemented |
| Community repository: index, search, install of themes and extensions | Implemented |
| Hosted marketplace service | Not implemented (needs a server) |
| Arch PKGBUILD, desktop entry, icon and release tarballs | Implemented |

## Build and verify

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Unit tests cover every crate; `desktop/cli/tests/cli.rs` runs the binary end to
end in temporary roots and user theme directories.

## Installation

```bash
# Build and install into ~/.local (both binaries, themes, profiles, desktop entry)
./packaging/install.sh

# Or build a release tarball and install from it
./packaging/build-release.sh
./target/release/my-linux-theme install --source dist/my-linux-<version>-<arch>
```

`install` records every file with a digest in
`~/.local/share/my-linux/install-manifest.toml`, then verifies it. `install-status`
rechecks the files and reports whether an upgrade is available; `upgrade` replaces
the binaries and replaces built-in themes/profiles only when they are unmodified
(anything you edited is preserved and reported); `uninstall` removes the recorded
program files and keeps user themes, user profiles, state and original
configuration backups.

All three accept `--prefix <dir>` and `--dry-run`; `install` and `upgrade` also
accept `--source <release directory>` and `--force`. Installed binaries find their
themes and profiles in `<prefix>/share/my-linux` at runtime.

Arch packaging is in `packaging/PKGBUILD`; it installs the same binaries,
`/usr/share/my-linux/{themes,profiles}`, the desktop entry and the icon.

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
./target/debug/my-linux-theme current
./target/debug/my-linux-theme rollback
./target/debug/my-linux-theme restore

# Recovery and diagnostics
./target/debug/my-linux-theme history
./target/debug/my-linux-theme logs --limit 10
./target/debug/my-linux-theme backup list
./target/debug/my-linux-theme doctor

# Wallpapers
./target/debug/my-linux-theme wallpaper list
./target/debug/my-linux-theme wallpaper preview neon --width 48
./target/debug/my-linux-theme wallpaper apply neon
./target/debug/my-linux-theme wallpaper palette neon --save --name neon-dream
./target/debug/my-linux-theme wallpaper restore

# Assets
./target/debug/my-linux-theme assets fonts
./target/debug/my-linux-theme assets status
./target/debug/my-linux-theme assets set --icon-theme Papirus --cursor-theme Bibata --cursor-size 28
./target/debug/my-linux-theme assets install ./Inter-Regular.ttf

# Profiles
./target/debug/my-linux-theme profile list
./target/debug/my-linux-theme profile info cyberpunk-developer
./target/debug/my-linux-theme profile preview nature
./target/debug/my-linux-theme profile create my-desktop --from nature
./target/debug/my-linux-theme profile set my-desktop rounding 4
./target/debug/my-linux-theme profile apply my-desktop
./target/debug/my-linux-theme profile restore
./target/debug/my-linux-theme profile export my-desktop ./my-desktop.toml
./target/debug/my-linux-theme profile remove my-desktop

# Repository
./target/debug/my-linux-theme repository list --index packages/community-index.toml
./target/debug/my-linux-theme repository search neon --index packages/community-index.toml
./target/debug/my-linux-theme repository install swaync --index packages/community-index.toml

# Extensions
./target/debug/my-linux-theme extensions list
./target/debug/my-linux-theme extensions info swaync
./target/debug/my-linux-theme extensions enable swaync
./target/debug/my-linux-theme extensions render swaync --theme cyberpunk
./target/debug/my-linux-theme extensions create my-widget --format css --destination my-widget/style.css
./target/debug/my-linux-theme extensions export my-widget ./packages
./target/debug/my-linux-theme extensions import ./packages/my-widget --as my-widget-copy
./target/debug/my-linux-theme extensions disable swaync
./target/debug/my-linux-theme extensions remove my-widget
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
| `mako` | Notifications | `mako/config` | `makoctl reload` |
| `wofi` | Wofi launcher | `wofi/style.css` | applied on next launch |
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

## Graphical application

`desktop/gui` is a native eframe/egui application built on the same core as the
CLI. It provides a dashboard, theme gallery, theme editor, application manager,
wallpaper manager with palette extraction, profile manager and a recovery screen,
with light and dark modes. It never renders a theme itself and never writes
outside the installation root.

```bash
cargo run -p my-linux-gui -- --root /tmp/my-linux-gui
```

The root defaults to `$MY_LINUX_ROOT` and then to `$XDG_STATE_HOME/my-linux`. The
GUI compiles and its state layer is tested against isolated roots; opening the
window itself requires a Wayland or X11 session.

## State, logs and diagnostics

`status` summarises the installation, `current` answers "what is active right
now" (theme, profile, wallpaper, assets, file health, last action), and `doctor`
checks the whole installation and exits non-zero when something needs attention.

Every operation is appended to `state/events.jsonl` as one JSON object per line:

```json
{"timestamp":1763560000,"level":"info","action":"apply","detail":"theme=cyberpunk files=13 backups=0"}
```

Failures are recorded with `"level":"error"` (for example `apply-failed` or
`profile-failed`), so a failed command can always be reconstructed later.
`logs` prints the tail of that file, `history` shows the installation history
and `backup list` shows every backup with its size, write time and digest.

`integrations status` reports, for every integration, whether the application is
detected and what happened to its generated file (unmodified, modified, missing
or not installed).

## Wallpapers and assets

Wallpapers come from `$MY_LINUX_WALLPAPERS` (colon separated) or, by default,
`~/.local/share/my-linux/wallpapers` and `~/Pictures/Wallpapers`. PNG and JPEG
files are read; `wallpaper preview` draws them with ANSI truecolor.

`wallpaper apply` copies the file into the library, records it in
`state/wallpaper` and `state/wallpaper-history.log`, then switches the desktop
with `swww` or `hyprpaper` when one is detected. Without a wallpaper daemon the
choice is recorded and nothing on the desktop changes; `wallpaper restore`
returns to the previous wallpaper.

`wallpaper palette` analyses the image, prints the dominant colours and a
proposed palette, and does nothing else unless `--save` (write a user theme) or
`--apply` (write and apply it) is passed — the active theme is never replaced
silently. The derived theme is a normal theme: `list`, `preview`, `export` and
`apply` treat it like any other.

`assets` discovers fonts, icon themes and cursor themes in the XDG and
`/usr/share` directories, copies installed assets into the My Linux library,
and stores the chosen font, icon theme, cursor theme and sizes in
`state/assets.conf`. Cursor settings are also written to
`hyprland/assets.conf`, a snippet you source from your own Hyprland
configuration; `--reload` additionally runs `hyprctl setcursor`.

## Profiles

A profile is a complete desktop: theme, wallpaper, the integrations to install,
font and icon settings and the window-manager layout. Built-in presets live in
`profiles/`, user profiles in `$MY_LINUX_USER_PROFILES` (default
`$XDG_DATA_HOME/my-linux/profiles`), and user profiles shadow built-ins.

```toml
[profile]
schema = 1
id = "cyberpunk-developer"
name = "Cyberpunk Developer"
version = "1.0.0"
description = "Neon developer desktop."
theme = "cyberpunk"
wallpaper = "/home/me/Wallpapers/neon.png"   # optional; path or library name
integrations = ["waybar", "rofi", "hyprland", "kitty"]  # empty = everything

[assets]
font = "JetBrainsMono Nerd Font"
font_size = 11
icon_theme = "Papirus-Dark"
cursor_theme = "Bibata-Modern-Ice"
cursor_size = 24

[layout]
gaps_in = 5
gaps_out = 12
border_size = 2
rounding = 10
active_opacity = 1.0
inactive_opacity = 0.95
blur = true
```

`profile apply <id>` is the single switching command: it renders the theme for
the profile's integrations, writes `hyprland/layout.conf`, stores the asset
settings and records the wallpaper. The layout file is installed inside the
theme transaction, so `rollback` and `restore` handle it like any other
generated file. The active profile is kept in `state/active-profile` and the one
before it in `state/profile-previous`, which `profile restore` re-applies.

Built-in presets: `cyberpunk-developer`, `minimal-workspace`, `nature` and
`gaming`. `profile create` starts from a theme or copies an existing profile;
`profile set` edits one field at a time; `profile import` and `profile export`
move profiles between machines.

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

## Extensions

An extension adds a third-party application integration without changing the
Rust core. An extension is a directory with a manifest and a template:

```text
~/.config/my-linux/extensions/my-widget/
├── extension.toml
└── template.css
```

```toml
[extension]
schema = 1
id = "my-widget"
name = "My Widget"
version = "1.0.0"
description = "Styling for my widget daemon"
format = "css"                 # toml | css | rasi | ini | text
destination = "my-widget/style.css"
template = "template.css"
programs = ["my-widget-daemon"]
reload = [["my-widget-daemon", "--reload"]]
```

A theme may also declare design tokens used by every renderer:

```toml
[typography]
font_family = "Inter"
mono_family = "JetBrainsMono Nerd Font"
font_size = 11

[metrics]
radius = 8
spacing = 8
border_width = 2
opacity = 100
```

The template is rendered from the central palette: `{{colors.background}}`,
`{{colors.primary}}`, `{{colors.active}}`, `{{theme.name}}`, `{{theme.id}}` and
the rest of the theme colours. Unknown placeholders, unsafe destinations (`..`,
absolute paths, the `state/` and `backups/` directories) and output that does
not parse in the declared format are rejected before anything is written.

`extensions enable <id>` records the extension in
`~/.config/my-linux/extensions.toml`; every later `apply` renders it into the
same transaction as the built-in integrations, so it is backed up, rolled back
and restored like any other generated file. `swaync` ships as a built-in
example. Extensions are ordinary content: `extensions create` writes a starter
template, and `extensions import`/`export` move a directory between machines.

## Community repository

A repository is a TOML index and the content it references. It can live in a git
repository, on a shared path or in a local folder:

```toml
[repository]
schema = 1
name = "My Linux Community"

[[package]]
id = "aurora"
kind = "theme"            # theme | extension
name = "Aurora"
version = "1.0.0"
description = "A cool blue theme"
source = "themes/aurora"  # relative to the index, or absolute
```

```bash
my-linux-theme repository list --index packages/community-index.toml
my-linux-theme repository install aurora --index https://github.com/you/themes
my-linux-theme repository sync https://github.com/you/themes
```

Remote indexes are cloned into `$XDG_CACHE_HOME/my-linux/repository` with `git`
and updated with `git pull --ff-only`; a local path is used in place. Installing
never bypasses validation: themes load through the theme registry, extensions
through the extension registry, and an existing installation needs `--force`.
`--index` can be replaced by `MY_LINUX_REPOSITORY`.

## Installation root layout

```text
<root>/
├── waybar.css
├── rofi/theme.rasi
├── hyprland/my-linux.conf
├── hyprland/layout.conf               # generated from the applied profile
├── hyprland/assets.conf               # generated from the asset settings
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
    ├── history.log
    ├── wallpaper                     # active wallpaper path
    ├── wallpaper-previous
    ├── wallpaper-history.log
    ├── active-profile
    ├── profile-previous
    ├── profile-history.log
    ├── events.jsonl                  # structured event log
    └── assets.conf
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
├── assets/          # wallpaper library, palette extraction, fonts/icons/cursors
├── profiles/        # profile schema, registry and profile application
├── extensions/      # extension manifest, template rendering and registry
├── repository/      # community index client for themes and extensions
├── installer/       # install, upgrade, verify, uninstall and the manifest
├── gui/             # native graphical application (eframe)
├── components/      # shared desktop components (placeholder)
└── cli/             # the my-linux-theme binary and its integration tests
packaging/           # PKGBUILD, desktop entry, icon, install and release scripts
themes/              # built-in theme definitions
extensions/          # built-in example extensions
configs/             # reference configuration templates
docs/                # product roadmap
```
