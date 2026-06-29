#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=$(node -p "require('$ROOT/sensor/package.json').version")
OUT_DIR="$ROOT/release/firefox-extension"
STAGE="$OUT_DIR/starlee-firefox-extension"
ZIP="$OUT_DIR/starlee-firefox-extension-${VERSION}.zip"

cd "$ROOT/sensor"
node scripts/build.mjs --target firefox

rm -rf "$STAGE"
mkdir -p "$OUT_DIR" "$STAGE"
rm -f "$ZIP"
cp -R "$ROOT/sensor/dist/firefox-extension/." "$STAGE/"
find "$STAGE" -name '*.map' -delete
rm -f "$STAGE/starlee-config.json"

(
  cd "$STAGE"
  LC_ALL=C find . -type f | sort | zip -X -q "$ZIP" -@
)

LOCAL_CONFIG="${STARLEE_FIREFOX_LOCAL_CONFIG:-$HOME/Starlee/sensor-extension/starlee-config.json}"
if [ -f "$LOCAL_CONFIG" ]; then
  cp "$LOCAL_CONFIG" "$STAGE/starlee-config.json"
fi

printf '%s\n' "$ZIP"
