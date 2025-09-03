#!/usr/bin/env bash
set -euo pipefail

# Typeswift rebuild helper
# Usage:
#   ./tools/rebuild.sh            # full: regen icons if needed, then bundle (builds Swift+Rust)
#   ./tools/rebuild.sh full       # same as default
#   ./tools/rebuild.sh fast       # fast: cargo build, then re-bundle without building Swift
#   ./tools/rebuild.sh icons      # regenerate .icns from logo.png only

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

MODE="${1:-full}"

maybe_make_icons() {
  local png="$ROOT_DIR/logo.png"
  local icns="$ROOT_DIR/icons/Typeswift.icns"
  if [[ ! -f "$icns" ]]; then
    echo "[icons] No .icns found, generating from logo.png"
    ./tools/makeicons.sh
    return
  fi
  if [[ "$png" -nt "$icns" ]]; then
    echo "[icons] logo.png is newer than .icns, regenerating"
    ./tools/makeicons.sh
  else
    echo "[icons] Up-to-date (.icns)"
  fi
}

case "$MODE" in
  icons)
    ./tools/makeicons.sh
    ;;
  fast)
    maybe_make_icons
    echo "[build] cargo build --release"
    cargo build --release
    echo "[bundle] NO_BUILD=1 ./tools/bundle_macos.sh"
    NO_BUILD=1 ./tools/bundle_macos.sh
    ;;
  full|*)
    maybe_make_icons
    echo "[bundle] ./tools/bundle_macos.sh"
    ./tools/bundle_macos.sh
    ;;
esac

echo "[done] App at: $ROOT_DIR/dist/Typeswift.app"

