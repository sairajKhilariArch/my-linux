# My Linux

My Linux is an independent Arch Linux desktop customisation platform. A single
TOML palette is the source of truth; the Rust core renders it into the
configuration formats that each desktop component understands.

The project does not depend on HyDE or any other rice framework.

**Status:** Phases 1–3 are complete for the Waybar integration: theme engine,
configuration management and theme management (discovery, creation, import,
export, removal, inheritance). The CLI installs into an isolated *installation
root* that you choose, so it never touches your real desktop configuration yet.
Real desktop paths, the remaining integrations, profiles, wallpapers and the GUI
are not implemented — see [`docs/to-do.md`](docs/to-do.md) for the roadmap and
[`CHANGELOG.md`](CHANGELOG.md) for release history.

## What works today

| Area | State |
|---|---|
| Theme loading, validation and discovery from any theme root | Implemented |
| Theme schema versioning and metadata validation | Implemented |
| Theme inheritance (`extends = "parent"`) | Implemented |
| Palette generation from a seed colour | Implemented |
| `create`, `import`, `export`, `remove` for user themes | Implemented |
| Palette preview and theme details (`preview`, `info`, `render`) | Implemented |
| Waybar CSS renderer | Implemented |
| Transactional apply with backup, verification and rollback | Implemented |
| First-run original backup, retained forever | Implemented |
| `restore`, `rollback`, journal recovery | Implemented |
| Modification detection (unmodified / hand-edited / missing) | Implemented |
| `status`, `history`, `backup list`, `doctor` | Implemented |
| Path validation, permission preservation, symlink-safe writes | Implemented |
| Rofi, Hyprland, Kitty, notifications, lockscreen integrations | Not implemented |
| Profiles, wallpapers, fonts, icons, GUI, packaging | Not implemented |

## Build and verify

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The suite contains unit tests for every crate plus end-to-end CLI tests in
`desktop/cli/tests/cli.rs`. Each CLI test runs in its own temporary root and
user theme directory, so the real desktop configuration is never modified.

## Command line

```bash
cargo build --workspace
./target/debug/my-linux-theme list
./target/debug/my-linux-theme info cyberpunk
./target/debug/my-linux-theme preview cyberpunk
./target/debug/my-linux-theme render cyberpunk
./target/debug/my-linux-theme create my-theme --from cyberpunk
./target/debug/my-linux-theme create forest --base '#6EBE5A'
./target/debug/my-linux-theme import ./theme.toml --as imported
./target/debug/my-linux-theme export my-theme ./my-theme.toml
./target/debug/my-linux-theme apply my-theme
./target/debug/my-linux-theme status
./target/debug/my-linux-theme rollback
./target/debug/my-linux-theme restore
./target/debug/my-linux-theme history
./target/debug/my-linux-theme backup list
./target/debug/my-linux-theme doctor
./target/debug/my-linux-theme remove my-theme
```

Every command accepts `--root <directory>` to pick the installation root. The
default is `$MY_LINUX_ROOT` if set, otherwise `target/theme-test`.

## Themes

A theme is a TOML palette. Built-in themes live in `themes/`, user themes in
`$MY_LINUX_USER_THEMES` (default `$XDG_DATA_HOME/my-linux/themes`,
usually `~/.local/share/my-linux/themes`). User themes are searched first, so
they can override a built-in theme with the same id.

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

Layout on disk, both accepted:

```text
themes/<id>/colors/palette.toml      # directory theme
themes/<id>.toml                     # single-file theme
```

Validation rules:

- `id` uses lowercase letters, digits, `-` or `_`, and must match its directory.
- `name`, `version` (`MAJOR.MINOR.PATCH`) and `description` are required.
- `schema` must be `1`; a newer schema is rejected with a clear error.
- All eleven colours and three states are required after inheritance is applied.
- `extends` must name an existing theme; cycles are rejected.
- Invalid themes are listed by `list` and reported by `doctor` instead of
  silently disappearing.

## Installation root layout

```text
<root>/
├── waybar.css                        # generated configuration
├── backups/
│   ├── waybar.css.original           # first original configuration, never overwritten
│   └── waybar.css.previous           # state before the last operation (used by rollback)
└── state/
    ├── active-theme                  # theme currently installed
    ├── installed.tsv                 # installed files and their digests
    ├── journal.tsv                   # present only while an operation is in flight
    ├── previous-theme                # theme to return to on rollback
    └── history.log                   # apply / restore / rollback / recover history
```

## Safety guarantees

- Every apply is a transaction: generate, validate, journal, back up, install
  atomically, then commit the new state.
- A failure at any step rolls the installation back; a failed apply never leaves
  a half-themed desktop.
- An interrupted operation is detected through the journal and reverted by the
  next command that touches the installation.
- The first original configuration is backed up once and never replaced.
- Replacing a configuration keeps its file permissions.
- Symlinked destinations are written through; the link is preserved.
- Files changed by hand are detected and reported.
- `restore` and `rollback` refuse to run when the backup they need is missing,
  and say which file is missing.
- `remove` only ever deletes inside the user theme directory; built-in themes
  cannot be removed.

## Repository layout

```text
desktop/
├── core/
│   ├── src/lib.rs          # registry wiring, installation and restore entry points
│   └── src/transaction.rs  # journal, transaction, rollback, history, diagnostics
├── theme-engine/           # discovery, schema validation, inheritance, generation
├── config-manager/         # safe file operations: backup, atomic install, restore, digests
├── components/             # shared desktop components (placeholder)
└── cli/                    # the my-linux-theme binary and its integration tests
themes/                     # built-in theme definitions
configs/                    # reference configuration templates
docs/                       # product roadmap
```
