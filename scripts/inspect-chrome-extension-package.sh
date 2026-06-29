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
const path = require("path");
const [tmp, zip] = process.argv.slice(2);
const manifest = JSON.parse(fs.readFileSync(`${tmp}/manifest.json`, "utf8"));
const build = JSON.parse(fs.readFileSync(`${tmp}/build-info.json`, "utf8"));
function fail(message) {
  console.error(message);
  process.exit(1);
}
function sameMembers(actual, expected) {
  return actual.length === expected.length && expected.every((value) => actual.includes(value));
}
if (manifest.manifest_version !== 3) {
  fail(`manifest_version must be 3, got ${manifest.manifest_version}`);
}
if (manifest.background?.service_worker !== "background.js" || manifest.background?.type !== "module") {
  fail("manifest background must use module service worker background.js");
}
if (manifest.options_page !== "options.html") {
  fail("manifest options_page must be options.html");
}
if (manifest.action?.default_title !== "Starlee") {
  fail("manifest action.default_title must be Starlee");
}
if (!sameMembers(manifest.permissions || [], ["storage", "activeTab", "tabs", "alarms"])) {
  fail(`manifest permissions changed unexpectedly: ${(manifest.permissions || []).join(", ")}`);
}
if (!sameMembers(manifest.host_permissions || [], ["http://127.0.0.1/*", "http://*/*", "https://*/*"])) {
  fail(`manifest host_permissions changed unexpectedly: ${(manifest.host_permissions || []).join(", ")}`);
}
const scripts = manifest.content_scripts || [];
if (scripts.length !== 1 || !sameMembers(scripts[0].js || [], ["content.js"])) {
  fail("manifest must include exactly one content script entry using content.js");
}
for (const match of ["http://*/*", "https://*/*", "https://www.youtube.com/*"]) {
  if (!(scripts[0].matches || []).includes(match)) {
    fail(`manifest content script matches must include ${match}`);
  }
}
for (const size of ["16", "32", "48", "128"]) {
  const icon = manifest.icons?.[size];
  if (icon !== `assets/icon-${size}.png`) {
    fail(`manifest icon ${size} must be assets/icon-${size}.png`);
  }
  if (manifest.action?.default_icon?.[size] !== icon) {
    fail(`action default icon ${size} must match manifest icon`);
  }
}
const basename = path.basename(zip);
const expectedBasename = `starlee-capture-${manifest.version}.zip`;
if (basename !== expectedBasename) {
  fail(`package filename ${basename} does not match manifest version; expected ${expectedBasename}`);
}
for (const field of ["git_commit", "git_branch", "git_dirty", "built_at"]) {
  if (typeof build[field] !== "string" || !build[field]) {
    fail(`build-info.json missing string field ${field}`);
  }
}
if (!/^\d{4}-\d{2}-\d{2}T/.test(build.built_at)) {
  fail("build-info.json built_at must be an ISO-like timestamp");
}
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
  build_identity: build.git_commit && build.git_commit !== "unknown"
    ? `${build.git_branch || "unknown"}@${build.git_commit}${build.git_dirty === "true" ? "+dirty" : ""}`
    : "unknown",
  built_at: build.built_at || "unknown",
  file_count: files.length
}, null, 2));
NODE
