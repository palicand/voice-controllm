#!/usr/bin/env bash
# Generate AppIcon.icns from a 1024x1024 source PNG.
# Usage: scripts/build-icns.sh path/to/source.png
set -euo pipefail

SOURCE="${1:?usage: $0 source-1024.png}"
REPO_ROOT="$(git rev-parse --show-toplevel)"
OUT_DIR="$REPO_ROOT/assets/icon"
ICONSET="$OUT_DIR/AppIcon.iconset"
ICNS="$OUT_DIR/AppIcon.icns"

mkdir -p "$ICONSET"

# Apple's required iconset: logical-size@scale -> pixel dimensions
# logical  scale  pixels
#   16       1x     16
#   16       2x     32
#   32       1x     32
#   32       2x     64
#  128       1x    128
#  128       2x    256
#  256       1x    256
#  256       2x    512
#  512       1x    512
#  512       2x   1024
declare -a logical=(16  16  32  32  128  128  256  256  512  512)
declare -a pixels=(16   32  32  64  128  256  256  512  512  1024)
declare -a names=(
    "icon_16x16.png"
    "icon_16x16@2x.png"
    "icon_32x32.png"
    "icon_32x32@2x.png"
    "icon_128x128.png"
    "icon_128x128@2x.png"
    "icon_256x256.png"
    "icon_256x256@2x.png"
    "icon_512x512.png"
    "icon_512x512@2x.png"
)

for i in "${!names[@]}"; do
    px="${pixels[$i]}"
    sips -z "$px" "$px" "$SOURCE" --out "$ICONSET/${names[$i]}" >/dev/null
done

iconutil -c icns -o "$ICNS" "$ICONSET"
rm -rf "$ICONSET"
echo "Generated $ICNS"
