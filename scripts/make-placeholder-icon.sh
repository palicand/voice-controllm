#!/usr/bin/env bash
# Generate a placeholder 1024x1024 AppIcon-1024.png. Used until a real icon is
# produced (see docs/plans/icon-prompt.md).
#
# Output: assets/icon/AppIcon-1024.png

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$REPO_ROOT/assets/icon/AppIcon-1024.png"
mkdir -p "$(dirname "$OUT")"

PY=$(command -v python3 || true)
if [[ -z "$PY" ]]; then
    echo "python3 not found — cannot generate placeholder icon" >&2
    exit 1
fi

"$PY" - "$OUT" <<'PYEOF'
import struct, zlib, sys

size = 1024
out = sys.argv[1]

start = (0x1A, 0x23, 0x7E)
end = (0x4A, 0x14, 0x8C)
radius = int(size * 0.22)


def in_squircle(x, y):
    dx = max(abs(x - size / 2) - (size / 2 - radius), 0)
    dy = max(abs(y - size / 2) - (size / 2 - radius), 0)
    return (dx ** 5 + dy ** 5) ** 0.2 < radius


def lerp(a, b, t):
    return int(a + (b - a) * t)


pixels = bytearray()
mic_cx, mic_cy = size / 2, size / 2 - size * 0.04
mic_body_w = size * 0.18
mic_body_h = size * 0.30
mic_stem_h = size * 0.12
mic_base_w = size * 0.28
mic_base_h = size * 0.04

for y in range(size):
    row = bytearray()
    t_y = y / (size - 1)
    for x in range(size):
        t = (x / (size - 1) + t_y) / 2
        r = lerp(start[0], end[0], t)
        g = lerp(start[1], end[1], t)
        b = lerp(start[2], end[2], t)
        a = 255 if in_squircle(x, y) else 0

        if (
            abs(x - mic_cx) < mic_body_w / 2
            and abs(y - mic_cy) < mic_body_h / 2
            and a > 0
        ):
            r = g = b = 255

        if (
            abs(x - mic_cx) < size * 0.02
            and mic_cy + mic_body_h / 2 < y < mic_cy + mic_body_h / 2 + mic_stem_h
            and a > 0
        ):
            r = g = b = 255

        if (
            abs(x - mic_cx) < mic_base_w / 2
            and abs(y - (mic_cy + mic_body_h / 2 + mic_stem_h)) < mic_base_h / 2
            and a > 0
        ):
            r = g = b = 255

        row += bytes((r, g, b, a))
    pixels += b"\x00" + row


def chunk(tag, data):
    return (
        struct.pack(">I", len(data))
        + tag
        + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )


with open(out, "wb") as f:
    f.write(b"\x89PNG\r\n\x1a\n")
    f.write(chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)))
    f.write(chunk(b"IDAT", zlib.compress(bytes(pixels), 9)))
    f.write(chunk(b"IEND", b""))

print(f"wrote {out}")
PYEOF
