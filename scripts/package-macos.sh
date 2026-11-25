#!/bin/sh
# Builds Pixel Pet.app for Apple and Intel Macs in one, and packs it. Has to run on a Mac.
# Usage: sh scripts/package-macos.sh   (see sign-macos.sh for signing it with a Developer ID)
set -e
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
export MACOSX_DEPLOYMENT_TARGET=11.0
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
out="target/package"
app="$out/Pixel Pet.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
lipo -create -output "$app/Contents/MacOS/pixel-pet" target/aarch64-apple-darwin/release/pixel-pet target/x86_64-apple-darwin/release/pixel-pet
strip "$app/Contents/MacOS/pixel-pet"
set -- 16 32 128 256
icons="$out/icon.iconset"
rm -rf "$icons"
mkdir -p "$icons"
for s in "$@"; do
    sips -z "$s" "$s" assets/pixel-pet.png --out "$icons/icon_${s}x${s}.png" >/dev/null
done
iconutil -c icns -o "$app/Contents/Resources/pixel-pet.icns" "$icons"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Pixel Pet</string>
<key>CFBundleIdentifier</key><string>io.github.pixel-pet</string>
<key>CFBundleExecutable</key><string>pixel-pet</string>
<key>CFBundleIconFile</key><string>pixel-pet</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>LSMinimumSystemVersion</key><string>11.0</string>
<key>LSUIElement</key><true/>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
EOF
cp NOTICE.md LICENSE LICENSE-ART.md assets/fonts/FiraMono-OFL.txt "$app/Contents/Resources/"
sh scripts/sign-macos.sh "$app" "$out/PixelPet-$version-macos.zip"
