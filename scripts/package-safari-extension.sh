#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=$(node -p "require('$ROOT/sensor/package.json').version")
OUT_DIR="$ROOT/release/safari-extension"
STAGE="$OUT_DIR/starlee-safari-web-extension"
ZIP="$OUT_DIR/starlee-safari-web-extension-${VERSION}.zip"
PROJECT_DIR="$OUT_DIR/StarleeSafari"
PROJECT_EXTENSION_DIR="$OUT_DIR/extension"
APP_ICON_SOURCE="$ROOT/assets/brand/starlee_desktop_application_icon.png"
CONVERTER=${SAFARI_WEB_EXTENSION_CONVERTER:-}

require_file() {
  if [ ! -f "$1" ]; then
    printf 'required file missing: %s\n' "$1" >&2
    exit 1
  fi
}

require_dir() {
  if [ ! -d "$1" ]; then
    printf 'required directory missing: %s\n' "$1" >&2
    exit 1
  fi
}

cd "$ROOT/sensor"
npm run build

rm -rf "$STAGE"
mkdir -p "$OUT_DIR" "$STAGE"
rm -f "$ZIP"
cp -R "$ROOT/sensor/dist/extension/." "$STAGE/"
find "$STAGE" -name '*.map' -delete
rm -f "$STAGE/starlee-config.json"
node - "$STAGE/manifest.json" <<'NODE'
const fs = require("node:fs");
const manifestPath = process.argv[2];
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
if (manifest.background && Object.prototype.hasOwnProperty.call(manifest.background, "type")) {
  delete manifest.background.type;
}
fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
NODE

(
  cd "$STAGE"
  LC_ALL=C find . -type f | sort | zip -X -q "$ZIP" -@
)

printf '%s\n' "$ZIP"
"$ROOT/scripts/inspect-safari-extension-package.sh" "$ZIP" >/dev/null

LOCAL_CONFIG="${STARLEE_SAFARI_LOCAL_CONFIG:-$HOME/Starlee/sensor-extension/starlee-config.json}"
if [ -f "$LOCAL_CONFIG" ]; then
  cp "$LOCAL_CONFIG" "$STAGE/starlee-config.json"
  printf 'Copied local Safari development config into staged source: %s\n' "$LOCAL_CONFIG" >&2
fi

if [ -z "$CONVERTER" ]; then
  CONVERTER=$(xcrun --find safari-web-extension-converter 2>/dev/null || true)
elif ! command -v "$CONVERTER" >/dev/null 2>&1; then
  cat >&2 <<EOF
Safari Web Extension source package is ready, but the configured converter was not found:
  SAFARI_WEB_EXTENSION_CONVERTER=$CONVERTER

Set SAFARI_WEB_EXTENSION_CONVERTER to Apple's safari-web-extension-converter, or select full Xcode:
  sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
EOF
  exit 1
fi

if [ -z "$CONVERTER" ]; then
  cat >&2 <<EOF
Safari Web Extension source package is ready.

The Safari Xcode wrapper was not generated because Apple's
safari-web-extension-converter is not available from xcrun.

Install full Xcode, open it once, then select it:
  sudo xcode-select -s /Applications/Xcode.app/Contents/Developer

Then rerun:
  ./scripts/package-safari-extension.sh

To make converter absence fail CI, set:
  STARLEE_REQUIRE_SAFARI_CONVERTER=1
EOF
  if [ "${STARLEE_REQUIRE_SAFARI_CONVERTER:-0}" = "1" ]; then
    exit 1
  fi
  exit 0
fi

CONVERTER_TMP="/private/tmp/starlee-safari-converter-${USER:-local}"
rm -rf "$CONVERTER_TMP"
mkdir -p "$CONVERTER_TMP"
trap 'rm -rf "$CONVERTER_TMP"' EXIT INT TERM
CONVERTER_STAGE="$CONVERTER_TMP/extension"
CONVERTER_PROJECT="$CONVERTER_TMP/StarleeSafari"
mkdir -p "$CONVERTER_STAGE"
cp -R "$STAGE/." "$CONVERTER_STAGE/"

rm -rf "$PROJECT_DIR" "$PROJECT_EXTENSION_DIR"
"$CONVERTER" "$CONVERTER_STAGE" \
  --macos-only \
  --project-location "$CONVERTER_PROJECT" \
  --app-name "Starlee Safari" \
  --bundle-identifier "com.starlee.capture.safari" \
  --no-open

require_dir "$CONVERTER_PROJECT"

cp -R "$CONVERTER_PROJECT" "$PROJECT_DIR"
cp -R "$CONVERTER_STAGE" "$PROJECT_EXTENSION_DIR"

if [ -f "$APP_ICON_SOURCE" ]; then
  APP_ICONSET=$(find "$PROJECT_DIR" -path '*/Assets.xcassets/AppIcon.appiconset' -type d -print -quit)
  if [ -n "$APP_ICONSET" ]; then
    sips --resampleHeightWidth 16 16 "$APP_ICON_SOURCE" --out "$APP_ICONSET/mac-icon-16@1x.png" >/dev/null
    sips --resampleHeightWidth 32 32 "$APP_ICON_SOURCE" --out "$APP_ICONSET/mac-icon-16@2x.png" >/dev/null
    sips --resampleHeightWidth 32 32 "$APP_ICON_SOURCE" --out "$APP_ICONSET/mac-icon-32@1x.png" >/dev/null
    sips --resampleHeightWidth 64 64 "$APP_ICON_SOURCE" --out "$APP_ICONSET/mac-icon-32@2x.png" >/dev/null
    sips --resampleHeightWidth 128 128 "$APP_ICON_SOURCE" --out "$APP_ICONSET/mac-icon-128@1x.png" >/dev/null
    sips --resampleHeightWidth 256 256 "$APP_ICON_SOURCE" --out "$APP_ICONSET/mac-icon-128@2x.png" >/dev/null
    sips --resampleHeightWidth 256 256 "$APP_ICON_SOURCE" --out "$APP_ICONSET/mac-icon-256@1x.png" >/dev/null
    sips --resampleHeightWidth 512 512 "$APP_ICON_SOURCE" --out "$APP_ICONSET/mac-icon-256@2x.png" >/dev/null
    sips --resampleHeightWidth 512 512 "$APP_ICON_SOURCE" --out "$APP_ICONSET/mac-icon-512@1x.png" >/dev/null
    sips --resampleHeightWidth 1024 1024 "$APP_ICON_SOURCE" --out "$APP_ICONSET/mac-icon-512@2x.png" >/dev/null
  fi
  RESOURCE_ICON=$(find "$PROJECT_DIR" -path '*/Resources/Icon.png' -type f -print -quit)
  if [ -n "$RESOURCE_ICON" ]; then
    sips --resampleHeightWidth 128 128 "$APP_ICON_SOURCE" --out "$RESOURCE_ICON" >/dev/null
  fi
fi

PROJECT_FILE=$(find "$PROJECT_DIR" -name project.pbxproj -print -quit)
if [ -n "$PROJECT_FILE" ]; then
  perl -0pi -e 's/PRODUCT_BUNDLE_IDENTIFIER = "?com\.starlee\.capture\.Starlee-Safari"?;/PRODUCT_BUNDLE_IDENTIFIER = com.starlee.capture.safari;/g' "$PROJECT_FILE"
fi

require_file "$PROJECT_DIR/Starlee Safari/Starlee Safari.xcodeproj/project.pbxproj"
require_dir "$PROJECT_EXTENSION_DIR"

if ! grep -R 'PRODUCT_BUNDLE_IDENTIFIER = com.starlee.capture.safari;' "$PROJECT_DIR" >/dev/null 2>&1; then
  printf 'generated Safari project does not contain expected app bundle identifier com.starlee.capture.safari\n' >&2
  exit 1
fi

if ! grep -R 'com.starlee.capture.safari.Extension' "$PROJECT_DIR" >/dev/null 2>&1; then
  printf 'generated Safari project does not contain expected extension identifier com.starlee.capture.safari.Extension\n' >&2
  exit 1
fi

printf '%s\n' "$PROJECT_DIR"
