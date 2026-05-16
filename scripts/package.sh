#!/usr/bin/env bash
# Build VCM.app and DMG.
#
# Outputs into dist/:
#   - VCM.app           (signed if APPLE_DEVELOPER_ID is set, ad-hoc otherwise)
#   - VCM-<ver>.dmg     (signed/notarized if creds present)
#   - VCM-<ver>.dmg.sha256
#
# Env vars consumed:
#   APPLE_DEVELOPER_ID         e.g. "Developer ID Application: Name (TEAMID)"
#                              When unset → ad-hoc sign (codesign -s -).
#   APPLE_API_KEY_PATH         Path to .p8 file (notarization).
#   APPLE_API_KEY_ID
#   APPLE_API_KEY_ISSUER_ID
#                              When all three are set → notarize + staple.
#   VCM_TARGET                 Cargo target triple. Default: aarch64-apple-darwin.
#   VCM_SKIP_BUILD             Set to skip `cargo build` (for re-packaging).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

TARGET="${VCM_TARGET:-aarch64-apple-darwin}"
DIST="$REPO_ROOT/dist"
APP="$DIST/VCM.app"
CONTENTS="$APP/Contents"
MACOS_DIR="$CONTENTS/MacOS"
RESOURCES_DIR="$CONTENTS/Resources"

VERSION=$(awk -F\" '/^version[[:space:]]*=/ {print $2; exit}' Cargo.toml)
if [[ -z "$VERSION" ]]; then
    echo "Could not determine version from Cargo.toml" >&2
    exit 1
fi

echo "==> Building VCM v$VERSION for $TARGET"

if [[ -z "${VCM_SKIP_BUILD:-}" ]]; then
    rustup target add "$TARGET" >/dev/null 2>&1 || true
    cargo build --release --target "$TARGET"
fi

BIN_DIR="$REPO_ROOT/target/$TARGET/release"
for bin in vcm vcmd vcmctl; do
    if [[ ! -f "$BIN_DIR/$bin" ]]; then
        echo "Missing binary: $BIN_DIR/$bin" >&2
        exit 1
    fi
done

echo "==> Building icon"
"$REPO_ROOT/scripts/build-icns.sh"

echo "==> Assembling VCM.app"
rm -rf "$APP"
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

for bin in vcm vcmd vcmctl; do
    cp "$BIN_DIR/$bin" "$MACOS_DIR/$bin"
    strip "$MACOS_DIR/$bin"
done

cp "$REPO_ROOT/assets/icon/AppIcon.icns" "$RESOURCES_DIR/AppIcon.icns"

sed "s/__VERSION__/$VERSION/g" \
    "$REPO_ROOT/scripts/Info.plist.template" \
    > "$CONTENTS/Info.plist"

echo "==> Signing"
"$REPO_ROOT/scripts/sign.sh" "$APP"

echo "==> Verifying signature"
codesign --verify --deep --strict --verbose=2 "$APP"

echo "==> Building DMG"
DMG_NAME="VCM-${VERSION}-aarch64.dmg"
DMG_PATH="$DIST/$DMG_NAME"
STAGING="$DIST/dmg-staging"

rm -rf "$STAGING" "$DMG_PATH"
mkdir -p "$STAGING"
cp -R "$APP" "$STAGING/VCM.app"
ln -s /Applications "$STAGING/Applications"

hdiutil create \
    -volname "VCM $VERSION" \
    -srcfolder "$STAGING" \
    -ov \
    -format UDZO \
    "$DMG_PATH"

rm -rf "$STAGING"

if [[ -n "${APPLE_DEVELOPER_ID:-}" ]]; then
    codesign --force --sign "$APPLE_DEVELOPER_ID" --timestamp "$DMG_PATH"
fi

echo "==> Notarizing (if credentials available)"
"$REPO_ROOT/scripts/notarize.sh" "$DMG_PATH"

echo "==> Computing checksum"
( cd "$DIST" && shasum -a 256 "$DMG_NAME" > "$DMG_NAME.sha256" )

echo
echo "Done."
echo "  $APP"
echo "  $DMG_PATH"
echo "  $DMG_PATH.sha256"
