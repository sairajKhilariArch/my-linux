#!/usr/bin/env bash
# Builds a release tarball that `my-linux-theme install --source <dir>` accepts.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(grep -m1 '^version' "$root/Cargo.toml" | cut -d'"' -f2)"
arch="$(uname -m)"
name="my-linux-$version-$arch"
dist="$root/dist"
stage="$dist/$name"

cd "$root"

echo "Building My Linux $version ($arch)"
cargo build --release --locked -p my-linux-theme -p my-linux-gui

rm -rf "$stage"
install -dm755 "$stage/bin" "$stage/share/my-linux" "$stage/share/applications"
install -Dm755 target/release/my-linux-theme "$stage/bin/my-linux-theme"
install -Dm755 target/release/my-linux-gui "$stage/bin/my-linux-gui"

cp -r themes "$stage/share/my-linux/themes"
cp -r profiles "$stage/share/my-linux/profiles"
cp -r extensions "$stage/share/my-linux/extensions"
install -Dm644 packaging/my-linux.desktop "$stage/share/applications/my-linux.desktop"

install -Dm644 README.md "$stage/README.md"
install -Dm644 CHANGELOG.md "$stage/CHANGELOG.md"

tar -C "$dist" -czf "$dist/$name.tar.gz" "$name"

echo
echo "Release: $dist/$name.tar.gz"
echo "Install with:"
echo "  tar -xzf $dist/$name.tar.gz -C /tmp"
echo "  ./target/release/my-linux-theme install --source /tmp/$name"
