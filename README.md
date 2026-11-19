# My Linux

My Linux is an independent Arch Linux desktop customisation platform. A single
TOML palette is the source of truth; the Rust core renders it into the
configuration formats that each desktop component understands.

The project does not depend on HyDE or any other rice framework.

**Status:** Phase 1 (foundation) is complete for the Waybar integration. The CLI
installs into an isolated *installation root* that you choose, so it never
touches your real desktop configuration yet. Real desktop paths, the remaining
integrations, profiles, wallpapers and the GUI are not implemented — see
[`docs/to-do.md`](docs/to-do.md) for the roadmap and
[`CHANGELOG.md`](CHANGELOG.md) for release history.

## What works today

| Area | State |
|---|---|
| Theme loading and validation from `themes/<name>/colors/palette.toml` | Implemented |
| Palette preview and theme details (`preview`, `info`, `render`) | Implemented |
| Waybar CSS renderer | Implemented |
| Atomic installation with first-run backup and `restore` | Implemented |
| Permission preservation and symlink-safe writes | Implemented |
| Path validation (rejects `..` traversal and empty paths) | Implemented |
| Active theme reporting (`status`) | Implemented |
| Rofi, Hyprland, Kitty, notifications, lockscreen integrations | Not implemented |
| Profiles, wallpapers, fonts, icons, GUI, packaging | Not implemented |

## Build and verify

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The test suite contains unit tests for every crate plus end-to-end CLI tests in
`desktop/cli/tests/cli.rs`. Each CLI test runs in its own temporary root, so the
real desktop configuration is never modified.

## Command line

```bash
cargo build --workspace
./target/debug/my-linux-theme list
./target/debug/my-linux-theme info cyberpunk
./target/debug/my-linux-theme preview cyberpunk
./target/debug/my-linux-theme render cyberpunk
./target/debug/my-linux-theme apply cyberpunk
./target/debug/my-linux-theme status
./target/debug/my-linux-theme restore
./target/debug/my-linux-theme export cyberpunk
```

Every command accepts `--root <directory>` to pick the installation root. The
default is `$MY_LINUX_ROOT` if set, otherwise `target/theme-test`.

### Installation root layout

```text
<root>/
├── waybar.css                        # generated configuration
├── backups/
│   └── waybar.css.original           # first original configuration, never overwritten
└── state/
    └── active-theme                  # theme currently installed, removed by restore
```

### Safety guarantees

- A configuration is written to a unique temporary file and published with an
  atomic rename.
- The first original configuration is backed up once and never replaced.
- Replacing a configuration keeps its file permissions.
- Symlinked destinations are written through; the link is preserved.
- A failed installation leaves the previous configuration untouched and removes
  its temporary file.
- `restore` refuses to run when no backup exists and says so explicitly.

## Repository layout

```text
desktop/
├── core/             # coordinates theme loading, installation and restore
├── theme-engine/     # TOML palette parsing and colour validation
├── config-manager/   # safe file operations: backup, atomic install, restore
├── components/       # shared desktop components (placeholder)
└── cli/              # the my-linux-theme binary and its integration tests
themes/               # built-in theme definitions
configs/              # reference configuration templates
docs/                 # product roadmap
```
