#!/bin/sh
# Puts Pixel Pet into your home folder and the applications menu. Run with --remove to take it out.
set -e
here="$(cd "$(dirname "$0")" && pwd)"
bin="$HOME/.local/bin"
apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
icons="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/256x256/apps"
if [ "$1" = "--remove" ]; then
    rm -f "$bin/pixel-pet" "$apps/pixel-pet.desktop" "$icons/pixel-pet.png" "${XDG_CONFIG_HOME:-$HOME/.config}/autostart/PixelPet.desktop"
    echo "Removed. Your settings are still in ${XDG_CONFIG_HOME:-$HOME/.config}/pixel-pet"
    exit 0
fi
mkdir -p "$bin" "$apps" "$icons"
cp "$here/pixel-pet" "$bin/pixel-pet"
chmod +x "$bin/pixel-pet"
cp "$here/pixel-pet.png" "$icons/pixel-pet.png"
cat > "$apps/pixel-pet.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Pixel Pet
Comment=A tiny pixel pet that lives on your desktop
Exec="$bin/pixel-pet"
Icon=pixel-pet
Categories=Utility;Amusement;
EOF
echo "Installed. Start it from your applications menu, or with: $bin/pixel-pet"
