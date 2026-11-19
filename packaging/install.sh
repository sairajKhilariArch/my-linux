#!/usr/bin/env bash
# Builds and installs My Linux from a source checkout.
#
#   ./packaging/install.sh                 install into ~/.local
#   ./packaging/install.sh --prefix DIR    install elsewhere
#   ./packaging/install.sh --dry-run       show what would be installed
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
prefix="${MY_LINUX_PREFIX:-$HOME/.local}"
dry_run=0
extra=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      prefix="$2"
      shift 2
      ;;
    --prefix=*)
      prefix="${1#--prefix=}"
      shift
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    --help|-h)
      sed -n '2,7p' "$0"
      exit 0
      ;;
    *)
      extra+=("$1")
      shift
      ;;
  esac
done

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo was not found. Install the Rust toolchain first (pacman -S rust)." >&2
  exit 1
fi

if [[ -r /etc/os-release ]]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  echo "Host: ${PRETTY_NAME:-unknown}"
fi

echo "Building My Linux from $root"
cargo build --release --locked --workspace --manifest-path "$root/Cargo.toml"

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

install -dm755 "$stage/bin" "$stage/share/my-linux"
install -m755 "$root/target/release/my-linux-theme" "$stage/bin/my-linux-theme"
install -m755 "$root/target/release/my-linux-gui" "$stage/bin/my-linux-gui"
cp -r "$root/themes" "$stage/share/my-linux/themes"
cp -r "$root/profiles" "$stage/share/my-linux/profiles"
cp -r "$root/extensions" "$stage/share/my-linux/extensions"

args=(install --prefix "$prefix" --source "$stage")
if [[ $dry_run -eq 1 ]]; then
  args+=(--dry-run)
fi

"$root/target/release/my-linux-theme" "${args[@]}" "${extra[@]}"

echo
echo "Add $prefix/bin to PATH if it is not there already."
