#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  printf 'usage: %s /path/to/starlee-safari-web-extension.zip\n' "$0" >&2
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

require_file() {
  if [ ! -f "$TMP/$1" ]; then
    printf 'required Safari package file missing: %s\n' "$1" >&2
    exit 1
  fi
}

require_file manifest.json
require_file background.js
require_file content.js
require_file options.html
require_file options.js
require_file build-info.json
require_file assets/icon-16.png
require_file assets/icon-32.png
require_file assets/icon-48.png
require_file assets/icon-128.png

if find "$TMP" \( -name 'config.json' -o -name 'starlee-config.json' -o -name '*.db' -o -name '*.db-shm' -o -name '*.db-wal' -o -name '*.gguf' -o -path '*/vault/*' -o -path '*/models/*' -o -path '*/node_modules/*' -o -name '*.map' \) | grep .; then
  printf 'forbidden local data or build artifact found in Safari package\n' >&2
  exit 1
fi

if find "$TMP" \( -name '*.svg' -o -name '*.webp' \) | grep .; then
  printf 'unsupported manifest icon format found in Safari package\n' >&2
  exit 1
fi

if grep -R -E 'Bearer [0-9a-fA-F]{32,}|captureToken"[[:space:]]*:[[:space:]]*"[0-9a-fA-F]{32,}' "$TMP"; then
  printf 'possible capture token found in Safari package\n' >&2
  exit 1
fi

if grep -R -E "fetch\\([[:space:]]*[\`'\"]https?://" "$TMP" | grep -v -E '127\.0\.0\.1|https://www\.youtube\.com/|https://youtube\.com/|https://m\.youtube\.com/|https://music\.youtube\.com/'; then
  printf 'unexpected remote fetch destination found in Safari package\n' >&2
  exit 1
fi

node - "$TMP" "$ZIP" <<'NODE'
const fs = require("fs");
const [tmp, zip] = process.argv.slice(2);
const manifest = JSON.parse(fs.readFileSync(`${tmp}/manifest.json`, "utf8"));
const build = JSON.parse(fs.readFileSync(`${tmp}/build-info.json`, "utf8"));
const files = [];
function walk(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const path = `${dir}/${entry.name}`;
    if (entry.isDirectory()) walk(path);
    else files.push(path.slice(tmp.length + 1));
  }
}
walk(tmp);
function fail(message) {
  console.error(message);
  process.exit(1);
}
if (manifest.manifest_version !== 3) fail("Safari package manifest must use Manifest V3");
if (manifest.background?.type) fail("Safari package manifest must not declare background.type");
if (manifest.background?.service_worker !== "background.js") fail("Safari package must use background.js as service worker");
for (const permission of ["storage", "activeTab", "tabs", "alarms"]) {
  if (!manifest.permissions?.includes(permission)) fail(`Safari package missing permission: ${permission}`);
}
for (const host of ["http://127.0.0.1/*", "http://*/*", "https://*/*"]) {
  if (!manifest.host_permissions?.includes(host)) fail(`Safari package missing host permission: ${host}`);
}
for (const match of ["https://www.youtube.com/*", "https://youtube.com/*", "https://m.youtube.com/*", "https://music.youtube.com/*", "https://*/*"]) {
  if (!manifest.content_scripts?.[0]?.matches?.includes(match)) fail(`Safari package missing content script match: ${match}`);
}
if (String(JSON.stringify(manifest)).includes("starlee-config.json")) fail("Safari package manifest must not reference starlee-config.json");
console.log(JSON.stringify({
  ok: true,
  package: zip,
  version: manifest.version,
  build_identity: build.git_commit && build.git_commit !== "unknown"
    ? `${build.git_branch || "unknown"}@${build.git_commit}${build.git_dirty === "true" ? "+dirty" : ""}`
    : "unknown",
  built_at: build.built_at || "unknown",
  file_count: files.length,
  safari_ready: {
    manifest_v3: true,
    background_type_removed: true,
    local_bridge_host: manifest.host_permissions.includes("http://127.0.0.1/*"),
    all_site_access_declared: manifest.host_permissions.includes("https://*/*") && manifest.host_permissions.includes("http://*/*")
  }
}, null, 2));
NODE
