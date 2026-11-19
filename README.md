# My Linux

My Linux is an independent Arch Linux desktop customisation platform. A single
TOML palette is the source of truth; the Rust core renders it into the
configuration formats that each desktop component understands.

The project does not depend on HyDE or any other rice framework.

**Status:** Phase 1 (foundation) and Phase 2 (configuration management) are
complete for the Waybar integration. The CLI installs into an isolated
*installation root* that you choose, so it never touches your real desktop
configuration yet. Real desktop paths, the remaining integrations, profiles,
wallpapers and the GUI are not implemented — see
[`docs/to-do.md`](docs/to-do.md) for the roadmap and
[`CHANGELOG.md`](CHANGELOG.md) for release history.

## What works today

| Area | State |
|---|---|
| Theme loading and validation from `themes/<name>/colors/palette.toml` | Implemented |
| Palette preview and theme details (`preview`, `info`, `render`) | Implemented |
| Waybar CSS renderer | Implemented |
| Transactional apply with backup, verification and rollback | Implemented |
| First-run original backup, retained forever | Implemented |
| `restore` back to the original configuration | Implemented |
| `rollback` to the state before the last apply | Implemented |
| Installation journal and recovery from an interrupted operation | Implemented |
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
./target/debug/my-linux-theme rollback
./target/debug/my-linux-theme restore
./target/debug/my-linux-theme history
./target/debug/my-linux-theme backup list
./target/debug/my-linux-theme doctor
./target/debug/my-linux-theme export cyberpunk
```

Every command accepts `--root <directory>` to pick the installation root. The
default is `$MY_LINUX_ROOT` if set, otherwise `target/theme-test`.

### Installation root layout

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

### Safety guarantees

- Every apply is a transaction: generate, validate, journal, back up, install
  atomically, then commit the new state.
- A failure at any step rolls the installation back; a failed apply never leaves
  a half-themed desktop.
- An interrupted operation is detected through the journal and reverted by the
  next command that touches the installation.
- The first original configuration is backed up once and never replaced.
- Replacing a configuration keeps its file permissions.
- Symlinked destinations are written through; the link is preserved.
- Files changed by hand are detected and reported instead of being silently
  overwritten without a backup.
- `restore` and `rollback` refuse to run when the backup they need is missing,
  and say which file is missing.

## Repository layout

```text
desktop/
├── core/
│   ├── src/lib.rs          # theme loading, installation and restore entry points
│   └── src/transaction.rs  # journal, transaction, rollback, history, diagnostics
├── theme-engine/           # TOML palette parsing and colour validation
├── config-manager/         # safe file operations: backup, atomic install, restore, digests
├── components/             # shared desktop components (placeholder)
└── cli/                    # the my-linux-theme binary and its integration tests
themes/                     # built-in theme definitions
configs/                    # reference configuration templates
docs/                       # product roadmap
```
