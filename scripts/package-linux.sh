#!/bin/sh
# Builds a Linux release that runs on any distribution, and packs it.
# Usage: sh scripts/package-linux.sh        (needs: rustup target add x86_64-unknown-linux-musl)
set -e
cd "$(dirname "$0")/.."
target="${1:-x86_64-unknown-linux-musl}"
version=$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
cargo build --release --target "$target"
name="PixelPet-$version-linux-${target%%-*}"
stage="target/package/$name"
rm -rf "$stage"
mkdir -p "$stage"
cp "${CARGO_TARGET_DIR:-target}/$target/release/pixel-pet" "$stage/"
strip "$stage/pixel-pet" 2>/dev/null || true
cp assets/linux/install.sh assets/linux/README.txt "$stage/"
cp assets/pixel-pet.png "$stage/pixel-pet.png"
cp NOTICE.md LICENSE LICENSE-ART.md assets/fonts/FiraMono-OFL.txt "$stage/"
chmod +x "$stage/pixel-pet" "$stage/install.sh"
tar -C target/package -czf "target/package/$name.tar.gz" "$name"
echo "OK: target/package/$name.tar.gz"
sha256sum "target/package/$name.tar.gz"
