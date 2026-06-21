#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUT_DIR="$ROOT/release/safari-extension"
PROJECT="$OUT_DIR/StarleeSafari/Starlee Safari/Starlee Safari.xcodeproj"
DERIVED_DATA="$OUT_DIR/DerivedData"
BUILT_APP="$DERIVED_DATA/Build/Products/Release/Starlee Safari.app"
APP_DEST="${STARLEE_APP_DIR:-$HOME/Applications}/Starlee Safari.app"
EXTENSION_ID="com.starlee.capture.safari.Extension"

if ! xcrun --find safari-web-extension-converter >/dev/null 2>&1; then
  cat >&2 <<EOF
Skipping Safari extension install because safari-web-extension-converter was not found.
Install full Xcode, open it once, then select it:
  sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
EOF
  exit 0
fi

"$ROOT/scripts/package-safari-extension.sh" >/dev/null

xcodebuild \
  -project "$PROJECT" \
  -scheme "Starlee Safari" \
  -configuration Release \
  -derivedDataPath "$DERIVED_DATA" \
  build >/dev/null

mkdir -p "$(dirname "$APP_DEST")"
pkill -f "$APP_DEST/Contents/MacOS/Starlee Safari" >/dev/null 2>&1 || true
rm -rf "$APP_DEST"
ditto "$BUILT_APP" "$APP_DEST"

pluginkit -r "$BUILT_APP/Contents/PlugIns/Starlee Safari Extension.appex" >/dev/null 2>&1 || true
pluginkit -e use -i "$EXTENSION_ID" >/dev/null 2>&1 || true
open "$APP_DEST" >/dev/null 2>&1 || true

printf 'Installed Starlee Safari extension app to %s\n' "$APP_DEST"
pluginkit -m -A -D -i "$EXTENSION_ID" >/dev/null
