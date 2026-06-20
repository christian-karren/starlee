#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  printf 'usage: %s /path/to/starlee-capture.zip\n' "$0" >&2
  exit 2
fi

ZIP="$1"
if [ ! -f "$ZIP" ]; then
  printf 'package not found: %s\n' "$ZIP" >&2
  exit 2
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

unzip -qq "$ZIP" -d "$TMP"

test -f "$TMP/manifest.json"
test -f "$TMP/background.js"
test -f "$TMP/content.js"
test -f "$TMP/options.html"
test -f "$TMP/options.js"
test -f "$TMP/assets/icon-16.png"
test -f "$TMP/assets/icon-32.png"
test -f "$TMP/assets/icon-48.png"
test -f "$TMP/assets/icon-128.png"

if find "$TMP" \( -name 'config.json' -o -name 'starlee-config.json' -o -name '*.db' -o -name '*.db-shm' -o -name '*.db-wal' -o -name '*.gguf' -o -path '*/vault/*' -o -path '*/models/*' -o -path '*/node_modules/*' -o -name '*.map' \) | grep .; then
  printf 'forbidden local data or build artifact found in package\n' >&2
  exit 1
fi

if find "$TMP" \( -name '*.svg' -o -name '*.webp' \) | grep .; then
  printf 'unsupported manifest icon format found in package\n' >&2
  exit 1
fi

if grep -R -E 'Bearer [0-9a-fA-F]{32,}|captureToken"[[:space:]]*:[[:space:]]*"[0-9a-fA-F]{32,}' "$TMP"; then
  printf 'possible capture token found in package\n' >&2
  exit 1
fi

if grep -R -E "fetch\\([[:space:]]*[\`'\"]https?://" "$TMP" | grep -v '127\.0\.0\.1'; then
  printf 'unexpected remote fetch destination found in package\n' >&2
  exit 1
fi

printf 'Chrome extension package inspection passed: %s\n' "$ZIP"
