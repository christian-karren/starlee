#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUT_DIR="$ROOT/release/safari-extension"
PROJECT="$OUT_DIR/StarleeSafari/Starlee Safari/Starlee Safari.xcodeproj"
DERIVED_DATA="$OUT_DIR/DerivedData"
BUILT_APP="$DERIVED_DATA/Build/Products/Release/Starlee Safari.app"
APP_DEST="${STARLEE_APP_DIR:-$HOME/Applications}/Starlee Safari.app"
EXTENSION_ID="com.starlee.capture.safari.Extension"

require_path() {
  if [ ! -e "$1" ]; then
    printf 'required Safari install artifact missing: %s\n' "$1" >&2
    exit 1
  fi
}

require_safari_converter() {
  if xcrun --find safari-web-extension-converter >/dev/null 2>&1; then
    return 0
  fi
  cat >&2 <<EOF
Cannot install the Safari extension because safari-web-extension-converter was not found.
Install full Xcode, open it once, then select it:
  sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
EOF
  exit 1
}

build_safari_app() {
  STARLEE_REQUIRE_SAFARI_CONVERTER=1 "$ROOT/scripts/package-safari-extension.sh" >/dev/null
  require_path "$PROJECT"
  xcodebuild \
    -project "$PROJECT" \
    -scheme "Starlee Safari" \
    -configuration Release \
    -derivedDataPath "$DERIVED_DATA" \
    build >/dev/null
  require_path "$BUILT_APP"
  require_path "$BUILT_APP/Contents/PlugIns/Starlee Safari Extension.appex"
}

install_safari_app() {
  mkdir -p "$(dirname "$APP_DEST")"
  pkill -f "$APP_DEST/Contents/MacOS/Starlee Safari" >/dev/null 2>&1 || true
  rm -rf "$APP_DEST"
  ditto "$BUILT_APP" "$APP_DEST"
}

register_safari_extension() {
  pluginkit -r "$BUILT_APP/Contents/PlugIns/Starlee Safari Extension.appex" >/dev/null 2>&1 || true
  pluginkit -e use -i "$EXTENSION_ID" >/dev/null 2>&1 || true
  open "$APP_DEST" >/dev/null 2>&1 || true
  pluginkit -m -A -D -i "$EXTENSION_ID"
}

require_safari_converter
build_safari_app
install_safari_app
register_safari_extension >/dev/null

printf 'Installed Starlee Safari extension app to %s\n' "$APP_DEST"
printf 'Registered Safari extension identifier: %s\n' "$EXTENSION_ID"
printf 'Enable Starlee in Safari Settings > Extensions, then grant site access for pages you want to save.\n'
