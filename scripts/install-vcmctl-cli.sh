#!/usr/bin/env bash
# Symlinks vcmctl into ~/.local/bin.
# Called by the VCM menubar app.
set -euo pipefail

BUNDLE_VCMCTL="${1:?usage: $0 /path/to/bundled/vcmctl}"

if [[ ! -x "$BUNDLE_VCMCTL" ]]; then
    echo "Error: vcmctl not found or not executable at $BUNDLE_VCMCTL" >&2
    exit 1
fi

TARGET_DIR="$HOME/.local/bin"
mkdir -p "$TARGET_DIR"
ln -sf "$BUNDLE_VCMCTL" "$TARGET_DIR/vcmctl"
echo "$TARGET_DIR/vcmctl"
