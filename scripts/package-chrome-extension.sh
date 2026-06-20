#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=$(node -p "require('$ROOT/sensor/package.json').version")
OUT_DIR="$ROOT/release/chrome-extension"
STAGE="$OUT_DIR/starlee-capture"
ZIP="$OUT_DIR/starlee-capture-${VERSION}.zip"

cd "$ROOT/sensor"
npm run build

rm -rf "$STAGE"
mkdir -p "$OUT_DIR" "$STAGE"
rm -f "$ZIP"
cp -R "$ROOT/sensor/dist/extension/." "$STAGE/"
find "$STAGE" -name '*.map' -delete
rm -f "$STAGE/starlee-config.json"

(
  cd "$STAGE"
  LC_ALL=C find . -type f | sort | zip -X -q "$ZIP" -@
)

printf '%s\n' "$ZIP"
