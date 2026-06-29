import test from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { access, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
let builtChromeDist;

async function ensureChromeDist() {
  builtChromeDist ||= execFileAsync(process.execPath, ["scripts/build.mjs"], {
    cwd: fileURLToPath(new URL("../", import.meta.url))
  });
  await builtChromeDist;
}

test("manifest stays Manifest V3 and requests all-site Safari access plus local bridge access", async () => {
  const manifest = JSON.parse(await readFile(new URL("../extension/manifest.json", import.meta.url), "utf8"));
  assert.equal(manifest.manifest_version, 3);
  assert.equal(manifest.name, "Starlee");
  assert.equal(manifest.background.service_worker, "background.js");
  assert.deepEqual(manifest.host_permissions, ["http://127.0.0.1/*", "http://*/*", "https://*/*"]);
  assert.ok(manifest.permissions.includes("storage"));
  assert.ok(manifest.permissions.includes("activeTab"));
  assert.ok(manifest.permissions.includes("alarms"));
  assert.ok(manifest.content_scripts[0].matches.includes("https://www.youtube.com/*"));
  assert.ok(manifest.content_scripts[0].matches.includes("https://youtube.com/*"));
  assert.ok(manifest.content_scripts[0].matches.includes("https://m.youtube.com/*"));
  assert.ok(manifest.content_scripts[0].matches.includes("https://music.youtube.com/*"));
  assert.ok(manifest.content_scripts[0].matches.includes("https://*/*"));
  assert.deepEqual(manifest.content_scripts[0].js, ["content.js"]);
  assert.equal(manifest.icons["128"], "assets/icon-128.png");
  assert.equal(manifest.action.default_title, "Starlee");
  assert.equal(manifest.action.default_icon["16"], "assets/icon-16.png");
});

test("built dist manifest includes YouTube matches and content script file", async () => {
  await ensureChromeDist();
  const manifest = JSON.parse(await readFile(new URL("../dist/extension/manifest.json", import.meta.url), "utf8"));
  const matches = manifest.content_scripts[0].matches;

  assert.ok(matches.includes("https://www.youtube.com/*"));
  assert.ok(matches.includes("https://youtube.com/*"));
  assert.ok(matches.includes("https://m.youtube.com/*"));
  assert.ok(matches.includes("https://music.youtube.com/*"));
  assert.deepEqual(manifest.content_scripts[0].js, ["content.js"]);
  await access(new URL("../dist/extension/content.js", import.meta.url));
  await access(new URL("../dist/extension/background.js", import.meta.url));
  await access(new URL("../dist/extension/build-info.json", import.meta.url));
});

test("built extension includes release build identity metadata", async () => {
  await ensureChromeDist();
  const build = JSON.parse(await readFile(new URL("../dist/extension/build-info.json", import.meta.url), "utf8"));

  assert.equal(typeof build.git_commit, "string");
  assert.equal(typeof build.git_branch, "string");
  assert.match(build.git_dirty, /^(true|false)$/);
  assert.match(build.built_at, /^\d{4}-\d{2}-\d{2}T/);
});

test("manifest icon assets exist at declared PNG dimensions", async () => {
  const manifest = JSON.parse(await readFile(new URL("../extension/manifest.json", import.meta.url), "utf8"));

  for (const [size, relativePath] of Object.entries(manifest.icons)) {
    const expectedSize = Number(size);
    const dimensions = await pngDimensions(new URL(`../extension/${relativePath}`, import.meta.url));
    assert.deepEqual(dimensions, { width: expectedSize, height: expectedSize });
    assert.equal(manifest.action.default_icon[size], relativePath);
  }

  for (const expectedSize of [32, 64, 96, 256]) {
    const logicalSize = expectedSize / 2;
    const dimensions = await pngDimensions(new URL(`../extension/assets/icon-${logicalSize}@2x.png`, import.meta.url));
    assert.deepEqual(dimensions, { width: expectedSize, height: expectedSize });
  }
});

test("store package source does not bundle local config", async () => {
  const manifest = await readFile(new URL("../extension/manifest.json", import.meta.url), "utf8");
  assert.equal(manifest.includes("starlee-config.json"), false);
  assert.equal(manifest.includes("captureToken"), false);
});

async function pngDimensions(url) {
  const bytes = await readFile(url);
  assert.equal(bytes.toString("ascii", 1, 4), "PNG");
  return {
    width: bytes.readUInt32BE(16),
    height: bytes.readUInt32BE(20),
  };
}
