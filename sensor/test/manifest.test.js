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

test("store package source does not bundle local config", async () => {
  const manifest = await readFile(new URL("../extension/manifest.json", import.meta.url), "utf8");
  assert.equal(manifest.includes("starlee-config.json"), false);
  assert.equal(manifest.includes("captureToken"), false);
});
