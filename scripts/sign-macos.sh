#!/bin/sh
# Signs Pixel Pet.app with a Developer ID, has Apple check it (notarisation) and packs it, so that it
# opens on any Mac without a warning. Runs on a Mac with the Xcode command line tools.
#
#   sh sign-macos.sh "Pixel Pet.app" [out.zip]
#
# SIGN_ID         the certificate; found by itself if there is exactly one Developer ID in the keychain
# NOTARY_PROFILE  a profile kept by:  xcrun notarytool store-credentials   (asks for what it needs)
#   or, as a build machine has it:  NOTARY_KEY (a .p8 file), NOTARY_KEY_ID and NOTARY_ISSUER
#
# Without a certificate the app is signed for this Mac only ("ad hoc").
set -e
app="$1"
[ -d "$app/Contents" ] || { echo "usage: sh sign-macos.sh \"Pixel Pet.app\" [out.zip]" >&2; exit 2; }
out="${2:-$(dirname "$app")/PixelPet-macos.zip}"
# a download leaves marks that must not be signed
xattr -cr "$app"
chmod +x "$app/Contents/MacOS/pixel-pet"

if [ -z "$SIGN_ID" ]; then
    ids=$(security find-identity -v -p codesigning 2>/dev/null | sed -n 's/.*"\(Developer ID Application: .*\)"/\1/p' | sort -u)
    n=$(printf '%s' "$ids" | grep -c . || true)
    if [ "$n" = 1 ]; then
        SIGN_ID="$ids"
    elif [ "$n" -gt 1 ]; then
        echo "There are several certificates. Say which one, as SIGN_ID:" >&2
        printf '%s\n' "$ids" >&2
        exit 2
    fi
fi

pack() {
    rm -f "$out"
    ditto -c -k --keepParent "$app" "$out"
}

if [ -z "$SIGN_ID" ]; then
    echo "No Developer ID certificate here: signed for this Mac only."
    codesign --force --sign - "$app"
    pack
    echo "OK (not for giving away): $out"
    exit 0
fi

echo "Signing as: $SIGN_ID"
# hardened runtime and a time stamp: Apple checks for both
codesign --force --options runtime --timestamp --sign "$SIGN_ID" "$app"
codesign --verify --strict --verbose=2 "$app"
pack

if [ -n "$NOTARY_PROFILE" ]; then
    set -- --keychain-profile "$NOTARY_PROFILE"
elif [ -n "$NOTARY_KEY" ] && [ -n "$NOTARY_KEY_ID" ] && [ -n "$NOTARY_ISSUER" ]; then
    set -- --key "$NOTARY_KEY" --key-id "$NOTARY_KEY_ID" --issuer "$NOTARY_ISSUER"
else
    echo "Signed, but not notarised: macOS still warns about it. Set NOTARY_PROFILE to finish."
    exit 0
fi
echo "Asking Apple to check it. This takes a few minutes."
answer=$(xcrun notarytool submit "$out" "$@" --wait 2>&1) || true
printf '%s\n' "$answer"
case "$answer" in
    *"status: Accepted"*) ;;
    *)
        id=$(printf '%s\n' "$answer" | sed -n 's/^ *id: *//p' | head -1)
        [ -n "$id" ] && xcrun notarytool log "$id" "$@" || true
        echo "Apple did not accept it." >&2
        exit 1
        ;;
esac
# the answer goes into the app, so that it opens without the internet too
xcrun stapler staple "$app"
xcrun stapler validate "$app"
spctl --assess --type execute --verbose=2 "$app"
pack
echo "OK, signed and notarised: $out"
shasum -a 256 "$out"
