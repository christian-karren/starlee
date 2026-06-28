#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=$(node -p "require('$ROOT/sensor/package.json').version")
OUT_DIR="$ROOT/release/chrome-extension"
STAGE="$OUT_DIR/starlee-capture"
ZIP="$OUT_DIR/starlee-capture-${VERSION}.zip"

cd "$ROOT/sensor"
npm run build

node - "$ROOT" "$VERSION" <<'NODE'
const fs = require("fs");
const [root, expectedVersion] = process.argv.slice(2);
const sourceManifest = JSON.parse(fs.readFileSync(`${root}/sensor/extension/manifest.json`, "utf8"));
const builtManifest = JSON.parse(fs.readFileSync(`${root}/sensor/dist/extension/manifest.json`, "utf8"));
function fail(message) {
  console.error(message);
  process.exit(1);
}
if (sourceManifest.version !== expectedVersion) {
  fail(`source manifest version ${sourceManifest.version} does not match sensor/package.json ${expectedVersion}`);
}
if (builtManifest.version !== expectedVersion) {
  fail(`built manifest version ${builtManifest.version} does not match sensor/package.json ${expectedVersion}`);
}
if (builtManifest.manifest_version !== 3) {
  fail(`built manifest_version must be 3, got ${builtManifest.manifest_version}`);
}
if (builtManifest.background?.service_worker !== "background.js") {
  fail("built manifest must use background.js as the service worker");
}
for (const required of ["manifest.json", "background.js", "content.js", "options.html", "options.js", "build-info.json"]) {
  if (!fs.existsSync(`${root}/sensor/dist/extension/${required}`)) {
    fail(`built extension is missing ${required}`);
  }
}
NODE

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
