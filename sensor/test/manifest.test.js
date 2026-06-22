import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

test("manifest stays Manifest V3 and local-only for network host permissions", async () => {
  const manifest = JSON.parse(await readFile(new URL("../extension/manifest.json", import.meta.url), "utf8"));
  assert.equal(manifest.manifest_version, 3);
  assert.equal(manifest.name, "Starlee");
  assert.equal(manifest.background.service_worker, "background.js");
  assert.deepEqual(manifest.host_permissions, ["http://127.0.0.1/*"]);
  assert.ok(manifest.permissions.includes("storage"));
  assert.ok(manifest.permissions.includes("activeTab"));
  assert.ok(manifest.permissions.includes("alarms"));
  assert.ok(manifest.content_scripts[0].matches.includes("https://*/*"));
  assert.equal(manifest.icons["128"], "assets/icon-128.png");
  assert.equal(manifest.action.default_title, "Starlee");
  assert.equal(manifest.action.default_icon["16"], "assets/icon-16.png");
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
