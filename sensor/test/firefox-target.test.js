import test from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { access, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const sensorRoot = fileURLToPath(new URL("../", import.meta.url));

test("Firefox manifest keeps local bridge permission separate from content-script page access", async () => {
  const manifest = JSON.parse(await readFile(new URL("../extension/manifest.firefox.json", import.meta.url), "utf8"));

  assert.equal(manifest.manifest_version, 3);
  assert.equal(manifest.name, "Starlee");
  assert.deepEqual(manifest.background.scripts, ["background.js"]);
  assert.equal(manifest.background.type, "module");
  assert.deepEqual(manifest.host_permissions, ["http://127.0.0.1/*"]);
  assert.equal(manifest.optional_host_permissions, undefined);
  assert.ok(manifest.permissions.includes("storage"));
  assert.ok(manifest.permissions.includes("activeTab"));
  assert.ok(manifest.permissions.includes("tabs"));
  assert.ok(manifest.permissions.includes("alarms"));
  assert.equal(manifest.options_ui.page, "options.html");
  assert.equal(manifest.browser_specific_settings.gecko.id, "capture@starlee.local");
  assert.ok(manifest.content_scripts[0].matches.includes("https://www.youtube.com/*"));
  assert.ok(manifest.content_scripts[0].matches.includes("http://*/*"));
  assert.ok(manifest.content_scripts[0].matches.includes("https://*/*"));
});

test("built Firefox target writes a separate extension directory", async () => {
  await execFileAsync(process.execPath, ["scripts/build.mjs", "--target", "firefox"], {
    cwd: sensorRoot
  });

  const manifest = JSON.parse(await readFile(new URL("../dist/firefox-extension/manifest.json", import.meta.url), "utf8"));
  const build = JSON.parse(await readFile(new URL("../dist/firefox-extension/build-info.json", import.meta.url), "utf8"));

  assert.equal(manifest.browser_specific_settings.gecko.id, "capture@starlee.local");
  assert.deepEqual(manifest.background.scripts, ["background.js"]);
  assert.deepEqual(manifest.host_permissions, ["http://127.0.0.1/*"]);
  assert.equal(build.target, "firefox");
  await access(new URL("../dist/firefox-extension/background.js", import.meta.url));
  await access(new URL("../dist/firefox-extension/content.js", import.meta.url));
  await access(new URL("../dist/firefox-extension/options.js", import.meta.url));
  await access(new URL("../dist/firefox-extension/options.html", import.meta.url));
});
