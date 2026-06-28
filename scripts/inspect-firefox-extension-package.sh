#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  printf 'usage: %s /path/to/starlee-firefox-extension.zip\n' "$0" >&2
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
test -f "$TMP/build-info.json"
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

node - "$TMP" "$ZIP" <<'NODE'
const fs = require("fs");
const [tmp, zip] = process.argv.slice(2);
const manifest = JSON.parse(fs.readFileSync(`${tmp}/manifest.json`, "utf8"));
const build = JSON.parse(fs.readFileSync(`${tmp}/build-info.json`, "utf8"));
const requiredPermissions = new Set(["storage", "activeTab", "tabs", "alarms"]);
for (const permission of requiredPermissions) {
  if (!manifest.permissions?.includes(permission)) {
    throw new Error(`missing permission: ${permission}`);
  }
}
if (manifest.manifest_version !== 3) throw new Error("Firefox target must use Manifest V3");
if (manifest.background?.service_worker !== "background.js") throw new Error("missing Firefox background service worker");
if (manifest.host_permissions?.length !== 1 || manifest.host_permissions[0] !== "http://127.0.0.1/*") {
  throw new Error("Firefox package host_permissions must stay local-only");
}
if (!manifest.browser_specific_settings?.gecko?.id) throw new Error("missing Gecko extension id");
if (build.target !== "firefox") throw new Error("build-info target must be firefox");
const files = [];
function walk(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const path = `${dir}/${entry.name}`;
    if (entry.isDirectory()) walk(path);
    else files.push(path.slice(tmp.length + 1));
  }
}
walk(tmp);
console.log(JSON.stringify({
  ok: true,
  package: zip,
  version: manifest.version,
  gecko_id: manifest.browser_specific_settings.gecko.id,
  build_identity: build.git_commit && build.git_commit !== "unknown"
    ? `${build.git_branch || "unknown"}@${build.git_commit}${build.git_dirty === "true" ? "+dirty" : ""}`
    : "unknown",
  built_at: build.built_at || "unknown",
  file_count: files.length
}, null, 2));
NODE
