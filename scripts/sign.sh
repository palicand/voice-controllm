#!/usr/bin/env bash
# Code-sign VCM.app.
#
# Usage: sign.sh path/to/VCM.app
#
# With APPLE_DEVELOPER_ID set: full Developer ID + hardened runtime + timestamp.
# Without: ad-hoc signature so the app runs on the developer's own machine.

set -euo pipefail

APP="${1:?usage: sign.sh <VCM.app>}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENTITLEMENTS="$REPO_ROOT/scripts/entitlements.plist"

# Sign nested binaries first (inside-out).
for nested in vcmd vcmctl; do
    bin="$APP/Contents/MacOS/$nested"
    if [[ -f "$bin" ]]; then
        if [[ -n "${APPLE_DEVELOPER_ID:-}" ]]; then
            codesign --force --options runtime --timestamp \
                --entitlements "$ENTITLEMENTS" \
                --sign "$APPLE_DEVELOPER_ID" \
                "$bin"
        else
            codesign --force --sign - --entitlements "$ENTITLEMENTS" "$bin"
        fi
    fi
done

if [[ -n "${APPLE_DEVELOPER_ID:-}" ]]; then
    codesign --force --options runtime --timestamp \
        --entitlements "$ENTITLEMENTS" \
        --sign "$APPLE_DEVELOPER_ID" \
        "$APP"
    echo "Signed with Developer ID: $APPLE_DEVELOPER_ID"
else
    codesign --force --sign - --entitlements "$ENTITLEMENTS" "$APP"
    echo "Ad-hoc signed (local dev). For distribution, set APPLE_DEVELOPER_ID."
fi
