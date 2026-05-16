#!/usr/bin/env bash
# Notarize a DMG via Apple notarytool. Conditional on env vars.
#
# Usage: notarize.sh path/to/VCM.dmg
#
# Required env vars (all three must be set):
#   APPLE_API_KEY_PATH        Path to App Store Connect API .p8 file
#   APPLE_API_KEY_ID          Key ID (10-char string)
#   APPLE_API_KEY_ISSUER_ID   Issuer UUID
#
# If any are missing, the DMG is left un-notarized and the script exits 0.

set -euo pipefail

DMG="${1:?usage: notarize.sh <VCM.dmg>}"

if [[ -z "${APPLE_API_KEY_PATH:-}" \
   || -z "${APPLE_API_KEY_ID:-}" \
   || -z "${APPLE_API_KEY_ISSUER_ID:-}" ]]; then
    echo "Skipping notarization — APPLE_API_KEY_PATH/ID/ISSUER_ID not all set"
    exit 0
fi

if [[ ! -f "$APPLE_API_KEY_PATH" ]]; then
    echo "APPLE_API_KEY_PATH does not exist: $APPLE_API_KEY_PATH" >&2
    exit 1
fi

echo "Submitting $DMG to notarytool"
xcrun notarytool submit "$DMG" \
    --key "$APPLE_API_KEY_PATH" \
    --key-id "$APPLE_API_KEY_ID" \
    --issuer "$APPLE_API_KEY_ISSUER_ID" \
    --wait

echo "Stapling ticket to $DMG"
xcrun stapler staple "$DMG"
xcrun stapler validate "$DMG"
