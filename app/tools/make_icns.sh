#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
ICONSET_DIR="$ROOT_DIR/icons/Typeswift.iconset"
ICNS_OUT="$ROOT_DIR/icons/Typeswift.icns"
SRC_PNG="$ROOT_DIR/logo.png"

if [[ ! -f "$SRC_PNG" ]]; then
  echo "Error: $SRC_PNG not found. Place your base logo.png at repo root." >&2
  exit 1
fi

mkdir -p "$ICONSET_DIR"
rm -f "$ICONSET_DIR"/*.png "$ICNS_OUT" || true

# Generate required icon sizes
sips -z 16 16   "$SRC_PNG" --out "$ICONSET_DIR/icon_16x16.png" >/dev/null
sips -z 32 32   "$SRC_PNG" --out "$ICONSET_DIR/icon_16x16@2x.png" >/dev/null
sips -z 32 32   "$SRC_PNG" --out "$ICONSET_DIR/icon_32x32.png" >/dev/null
sips -z 64 64   "$SRC_PNG" --out "$ICONSET_DIR/icon_32x32@2x.png" >/dev/null
sips -z 128 128 "$SRC_PNG" --out "$ICONSET_DIR/icon_128x128.png" >/dev/null
sips -z 256 256 "$SRC_PNG" --out "$ICONSET_DIR/icon_128x128@2x.png" >/dev/null
sips -z 256 256 "$SRC_PNG" --out "$ICONSET_DIR/icon_256x256.png" >/dev/null
sips -z 512 512 "$SRC_PNG" --out "$ICONSET_DIR/icon_256x256@2x.png" >/dev/null
sips -z 512 512 "$SRC_PNG" --out "$ICONSET_DIR/icon_512x512.png" >/dev/null
sips -z 1024 1024 "$SRC_PNG" --out "$ICONSET_DIR/icon_512x512@2x.png" >/dev/null

# Build icns
iconutil -c icns "$ICONSET_DIR" -o "$ICNS_OUT"
echo "Built $ICNS_OUT"

