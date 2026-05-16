#!/usr/bin/env bash
# Build assets/icon/AppIcon.icns from assets/icon/AppIcon-1024.png.
# If AppIcon-1024.png is missing, generate a placeholder first.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICON_DIR="$REPO_ROOT/assets/icon"
SOURCE_PNG="$ICON_DIR/AppIcon-1024.png"
ICONSET_DIR="$ICON_DIR/AppIcon.iconset"
OUT_ICNS="$ICON_DIR/AppIcon.icns"

if [[ ! -f "$SOURCE_PNG" ]]; then
    echo "AppIcon-1024.png missing — generating placeholder"
    "$REPO_ROOT/scripts/make-placeholder-icon.sh"
fi

rm -rf "$ICONSET_DIR"
mkdir -p "$ICONSET_DIR"

declare -a SIZES=(
    "16:icon_16x16.png"
    "32:icon_16x16@2x.png"
    "32:icon_32x32.png"
    "64:icon_32x32@2x.png"
    "128:icon_128x128.png"
    "256:icon_128x128@2x.png"
    "256:icon_256x256.png"
    "512:icon_256x256@2x.png"
    "512:icon_512x512.png"
    "1024:icon_512x512@2x.png"
)

for entry in "${SIZES[@]}"; do
    px="${entry%%:*}"
    name="${entry##*:}"
    sips -z "$px" "$px" "$SOURCE_PNG" --out "$ICONSET_DIR/$name" >/dev/null
done

iconutil -c icns "$ICONSET_DIR" -o "$OUT_ICNS"
rm -rf "$ICONSET_DIR"

echo "wrote $OUT_ICNS"
