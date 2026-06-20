#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=$(node -p "require('$ROOT/sensor/package.json').version")
OUT_DIR="$ROOT/release/safari-extension"
STAGE="$OUT_DIR/starlee-safari-web-extension"
ZIP="$OUT_DIR/starlee-safari-web-extension-${VERSION}.zip"
PROJECT_DIR="$OUT_DIR/StarleeSafari"
CONVERTER=${SAFARI_WEB_EXTENSION_CONVERTER:-}

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

if [ -z "$CONVERTER" ]; then
  CONVERTER=$(xcrun --find safari-web-extension-converter 2>/dev/null || true)
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

rm -rf "$PROJECT_DIR"
"$CONVERTER" "$STAGE" \
  --macos-only \
  --project-location "$PROJECT_DIR" \
  --app-name "Starlee Safari" \
  --bundle-identifier "com.starlee.capture.safari" \
  --no-open

printf '%s\n' "$PROJECT_DIR"
