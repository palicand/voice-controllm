#!/usr/bin/env bash
# Symlinks vcmctl into the user's PATH.
# Called by the VCM menubar app via osascript.
set -euo pipefail

BUNDLE_VCMCTL="${1:?usage: $0 /path/to/bundled/vcmctl}"

if [[ ! -x "$BUNDLE_VCMCTL" ]]; then
    echo "Error: vcmctl not found or not executable at $BUNDLE_VCMCTL" >&2
    exit 1
fi

# Prefer /usr/local/bin if writable; otherwise ~/.local/bin.
for TARGET_DIR in /usr/local/bin "$HOME/.local/bin"; do
    if [[ -d "$TARGET_DIR" && -w "$TARGET_DIR" ]] || mkdir -p "$TARGET_DIR" 2>/dev/null; then
        ln -sf "$BUNDLE_VCMCTL" "$TARGET_DIR/vcmctl"
        echo "Installed: $TARGET_DIR/vcmctl -> $BUNDLE_VCMCTL"
        exit 0
    fi
done

echo "Error: could not create /usr/local/bin or ~/.local/bin" >&2
exit 2
