#!/usr/bin/env bash
set -euo pipefail

# Bundle a macOS .app for Typeswift

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
APP_NAME="Typeswift"
APP_ID="com.typeswift.app"
BINARY_NAME="typeswift"
ICON_PATH="$ROOT_DIR/icons/Typeswift.icns"
SWIFT_BUILD_DIR="$ROOT_DIR/VoicySwift/.build/release"
DYLIB_NAME="libTypeswiftSwift.dylib"

# Derive version from Cargo.toml
VERSION=$(awk -F ' *= *' '/^version/ {gsub(/"/, "", $2); print $2; exit}' "$ROOT_DIR/Cargo.toml")
if [[ -z "$VERSION" ]]; then VERSION="0.1.0"; fi

APP_ROOT="$DIST_DIR/${APP_NAME}.app"
CONTENTS="$APP_ROOT/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"
FRAMEWORKS="$CONTENTS/Frameworks"

if [[ "${NO_BUILD:-0}" == "1" ]]; then
  echo "==> Skipping builds (NO_BUILD=1)"
else
  if [[ -f "$SWIFT_BUILD_DIR/$DYLIB_NAME" ]]; then
    echo "==> Swift dylib already built, skipping"
  else
    echo "==> Building Swift bridge (release)"
    (
      cd "$ROOT_DIR/VoicySwift"
      swift build -c release --product TypeswiftSwift
    )
  fi
  if [[ -x "$ROOT_DIR/target/release/$BINARY_NAME" ]]; then
    echo "==> Rust binary already built, skipping"
  else
    echo "==> Building Rust binary (release)"
    cargo build --release
  fi
fi

echo "==> Creating bundle layout at $APP_ROOT"
rm -rf "$APP_ROOT"
mkdir -p "$MACOS" "$RESOURCES" "$FRAMEWORKS"

echo "==> Copying binary and resources"
install -m 0755 "$ROOT_DIR/target/release/$BINARY_NAME" "$MACOS/$BINARY_NAME"
if [[ -f "$ICON_PATH" ]]; then
  install -m 0644 "$ICON_PATH" "$RESOURCES/Typeswift.icns"
fi
for asset in menubar.png menubar_recording.png logo.png; do
  [[ -f "$ROOT_DIR/$asset" ]] && install -m 0644 "$ROOT_DIR/$asset" "$RESOURCES/$asset"
done

echo "==> Staging Swift dylib"
install -m 0644 "$SWIFT_BUILD_DIR/$DYLIB_NAME" "$FRAMEWORKS/$DYLIB_NAME"

INFO_PLIST="$CONTENTS/Info.plist"
echo "==> Writing Info.plist"
cat > "$INFO_PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>${BINARY_NAME}</string>
  <key>CFBundleIconFile</key>
  <string>Typeswift</string>
  <key>CFBundleIdentifier</key>
  <string>${APP_ID}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSMicrophoneUsageDescription</key>
  <string>Typeswift needs microphone access to transcribe speech.</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

# Ad-hoc sign to avoid runtime warnings and enable loading local dylibs
echo "==> Ad-hoc signing app and embedded dylibs"
codesign --force --timestamp=none --sign - "$FRAMEWORKS/$DYLIB_NAME" || true
codesign --force --deep --timestamp=none --sign - "$APP_ROOT" || true

echo "==> Done. App bundle at: $APP_ROOT"
echo "Open with: open '$APP_ROOT'"
