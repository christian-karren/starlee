/**
 * Playwright E2E tests for the Starlee browser extension.
 *
 * The tests load the unpacked extension from dist/extension/ into a real
 * Chromium instance and exercise it against a local mock capture server on an
 * ephemeral 127.0.0.1 port to avoid conflicts with a running Starlee instance
 * on the default port.
 *
 * The mock server also serves fixture.html over HTTP so the extension's
 * content scripts (which only match http:// and https:// URLs) are injected.
 *
 * Three suites:
 *   1. Article extraction — navigate to fixture.html, click the button
 *      injected by the content script, assert the background POSTs a valid
 *      article payload to /capture.
 *   2. Menu-bar capture flow — place a pending request at /capture-request,
 *      wait for the extension to poll, extract the active tab, and POST a
 *      result to /capture-request/result.
 *   3. Duplicate URL dedup — same request.id exposed across multiple poll
 *      cycles appears in /capture-request/result exactly once.
 */

import { test, expect, chromium } from "@playwright/test";
import http from "http";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { randomUUID } from "crypto";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const EXTENSION_PATH = path.resolve(__dirname, "../../dist/extension");
const FIXTURE_HTML_PATH = path.resolve(__dirname, "../fixture.html");

const TEST_TOKEN = "test-token-e2e";

// ---------------------------------------------------------------------------
// Module-level server state (one server for the whole test file)
// ---------------------------------------------------------------------------

let _server = null;
let _serverPort = null;

/** @type {Array<{id:string, status:string, message:string}>} */
const _captureRequestResults = [];

/** @type {Array<{payload: object}>} */
const _captures = [];

let _pendingRequest = null;

function resetState() {
  _captureRequestResults.length = 0;
  _captures.length = 0;
  _pendingRequest = null;
}

// ---------------------------------------------------------------------------
// Mock HTTP server
// ---------------------------------------------------------------------------

function startServer() {
  const fixtureHtml = fs.readFileSync(FIXTURE_HTML_PATH, "utf8");

  return new Promise((resolve, reject) => {
    _server = http.createServer((req, res) => {
      const requestUrl = new URL(req.url || "/", `http://${req.headers.host || "127.0.0.1"}`);
      // Serve the article fixture so content scripts run (file:// is excluded
      // from the manifest's content_scripts matches).
      if (req.method === "GET" && requestUrl.pathname === "/fixture.html") {
        res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
        res.end(fixtureHtml);
        return;
      }

      // All other routes require Bearer token auth
      const auth = req.headers["authorization"] || "";
      if (auth !== `Bearer ${TEST_TOKEN}`) {
        res.writeHead(401, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "unauthorized" }));
        return;
      }

      let body = "";
      req.on("data", (c) => (body += c));
      req.on("end", () => {
        const json = (code, data) => {
          res.writeHead(code, { "Content-Type": "application/json" });
          res.end(JSON.stringify(data));
        };

        // Article / direct capture
        if (req.method === "POST" && requestUrl.pathname === "/capture") {
          const parsed = JSON.parse(body || "{}");
          _captures.push(parsed);
          json(200, { ok: true, id: randomUUID() });
          return;
        }

        // Menu-bar polling
        if (req.method === "GET" && requestUrl.pathname === "/capture-request") {
          json(200, { request: _pendingRequest });
          return;
        }
        if (req.method === "POST" && requestUrl.pathname === "/capture-request/result") {
          _captureRequestResults.push(JSON.parse(body || "{}"));
          json(200, { ok: true });
          return;
        }

        // Extension handshake + diagnostics (acknowledge silently)
        if (req.method === "POST" && requestUrl.pathname === "/extension/hello") {
          json(200, {
            ok: true,
            service: "starlee",
            version: "test",
            poll_interval_seconds: 1,
          });
          return;
        }
        if (req.method === "POST" && requestUrl.pathname === "/capture-diagnostics/event") {
          json(200, { ok: true });
          return;
        }
        if (req.method === "GET" && requestUrl.pathname === "/bridge-health") {
          json(200, { ok: true, recommended_next_action: "none" });
          return;
        }

        json(404, { error: "not_found" });
      });
    });

    _server.listen(0, "127.0.0.1", () => {
      _serverPort = _server.address().port;
      resolve();
    });
    _server.once("error", reject);
  });
}

function stopServer() {
  return new Promise((resolve) => {
    if (!_server) {
      resolve();
      return;
    }
    _server.close(() => {
      _server = null;
      _serverPort = null;
      resolve();
    });
  });
}

// ---------------------------------------------------------------------------
// Extension context helpers
// ---------------------------------------------------------------------------

/**
 * Launch a Chromium profile with the unpacked extension loaded, inject the
 * test token and port into chrome.storage.local, and return the context.
 */
async function launchExtensionContext() {
  if (!_serverPort) throw new Error("mock capture server has not started");
  const userDataDir = `/tmp/starlee-e2e-${randomUUID()}`;

  const context = await chromium.launchPersistentContext(userDataDir, {
    headless: false,
    args: [
      `--disable-extensions-except=${EXTENSION_PATH}`,
      `--load-extension=${EXTENSION_PATH}`,
      "--no-sandbox",
      "--disable-dev-shm-usage",
    ],
    ignoreHTTPSErrors: true,
  });

  // Give the service worker time to register
  await waitMs(2000);

  const extensionId = await resolveExtensionId(context);

  // Inject credentials via the extension's own options page so the background
  // picks them up on the next poll cycle.
  const setup = await context.newPage();
  await setup.goto(`chrome-extension://${extensionId}/options.html`, {
    waitUntil: "domcontentloaded",
  });
  // The background reads captureToken / capturePort from chrome.storage.local
  await setup.evaluate(
    ({ captureToken, capturePort }) =>
      chrome.storage.local.set({ captureToken, capturePort }),
    { captureToken: TEST_TOKEN, capturePort: _serverPort }
  );
  await setup.close();

  // Wait for the background to complete at least one hello + poll with the new creds
  await waitMs(3000);

  return { context, extensionId };
}

function fixtureUrl() {
  if (!_serverPort) throw new Error("mock capture server has not started");
  return `http://127.0.0.1:${_serverPort}/fixture.html`;
}

async function resolveExtensionId(context) {
  // Prefer the service worker URL (fastest)
  const workers = context.serviceWorkers();
  if (workers.length > 0) {
    return new URL(workers[0].url()).hostname;
  }
  // Fallback: scrape chrome://extensions
  const p = await context.newPage();
  await p.goto("chrome://extensions");
  await waitMs(1000);
  const id = await p.evaluate(() => {
    const mgr = document.querySelector("extensions-manager");
    if (!mgr?.shadowRoot) return null;
    for (const el of mgr.shadowRoot.querySelectorAll("extensions-item")) {
      if (el.getAttribute("name") === "Starlee") return el.id;
    }
    return null;
  });
  await p.close();
  if (!id) throw new Error("Could not locate Starlee extension id");
  return id;
}

function waitMs(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

/** Poll fn() every intervalMs until it returns truthy or timeoutMs elapses. */
async function waitFor(fn, timeoutMs = 20_000, intervalMs = 400) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const v = await fn();
    if (v) return v;
    await waitMs(intervalMs);
  }
  return null;
}

// ---------------------------------------------------------------------------
// Module-level lifecycle (server starts once, suites share it)
// ---------------------------------------------------------------------------

test.beforeAll(async () => {
  await startServer();
});

test.afterAll(async () => {
  await stopServer();
});

// ---------------------------------------------------------------------------
// Suite 1: Article extraction from fixture.html
//
// Navigates to the fixture served over HTTP (so content scripts run),
// clicks the floating "Save article to Starlee" button injected by the
// extension, and verifies a valid article payload reaches /capture.
// ---------------------------------------------------------------------------

test.describe("Article extraction", () => {
  let context;

  test.beforeAll(async () => {
    resetState();
    ({ context } = await launchExtensionContext());
  });

  test.afterAll(async () => {
    await context.close();
  });

  test("extracts article payload from fixture.html via extension capture button", async () => {
    const page = await context.newPage();
    await page.goto(fixtureUrl(), { waitUntil: "domcontentloaded" });

    // Wait for the content script to inject the floating capture button
    const captureButton = page.locator("#starlee-save-button");
    await expect(captureButton).toBeVisible({ timeout: 10_000 });

    await captureButton.click();

    // Wait for the background to POST the extracted payload to /capture
    // Background sends the payload object directly as the POST body (not wrapped)
    const payload = await waitFor(() => _captures[0], 15_000);
    expect(payload).not.toBeNull();

    expect(payload.type).toBe("article");
    expect(payload.version).toBe(1);
    expect(payload.dom_extract.title).toMatch(/durable browser memory/i);
    expect(payload.access).toBe("public");
    expect(payload.consumed_at).toBeTruthy();

    await page.close();
  });
});

// ---------------------------------------------------------------------------
// Suite 2: Menu-bar capture flow
//
// Places a capture request at /capture-request, loads the fixture so the
// content script is ready on the active tab, then waits for the background
// to poll, extract, and POST a result to /capture-request/result.
// ---------------------------------------------------------------------------

test.describe("Menu-bar capture flow", () => {
  let context;
  let extensionId;

  test.beforeAll(async () => {
    resetState();
    ({ context, extensionId } = await launchExtensionContext());
  });

  test.afterAll(async () => {
    await context.close();
  });

  test("extension polls /capture-request and POSTs result", async () => {
    const articlePage = await context.newPage();
    await articlePage.goto(fixtureUrl(), { waitUntil: "domcontentloaded" });
    await articlePage.bringToFront();

    // Wait for the content script to mount (proves it's ready to capture)
    await expect(articlePage.locator("#starlee-save-button")).toBeVisible({
      timeout: 10_000,
    });

    const requestId = randomUUID();
    _pendingRequest = { id: requestId, source: "menu-bar" };

    // Poke the background to trigger an immediate poll cycle
    const optPage = await context.newPage();
    await optPage.goto(`chrome-extension://${extensionId}/options.html`, {
      waitUntil: "domcontentloaded",
    });
    await optPage.evaluate(() =>
      chrome.runtime.sendMessage({ type: "STARLEE_STATUS" }).catch(() => {})
    );
    await optPage.close();

    // The background polls on its own alarm; wait for the result
    const result = await waitFor(
      () => _captureRequestResults.find((r) => r.id === requestId),
      25_000
    );
    _pendingRequest = null;

    expect(result).not.toBeNull();
    expect(result.id).toBe(requestId);

    await articlePage.close();
  });
});

// ---------------------------------------------------------------------------
// Suite 3: Duplicate request dedup
//
// Exposes the same request.id across multiple poll cycles and verifies that
// the background's processedRequests Set prevents it from being handled more
// than once (the result should appear in /capture-request/result exactly once).
// ---------------------------------------------------------------------------

test.describe("Duplicate request dedup", () => {
  let context;
  let extensionId;

  test.beforeAll(async () => {
    resetState();
    ({ context, extensionId } = await launchExtensionContext());
  });

  test.afterAll(async () => {
    await context.close();
  });

  test("same request.id produces exactly one result entry", async () => {
    const articlePage = await context.newPage();
    await articlePage.goto(fixtureUrl(), { waitUntil: "domcontentloaded" });
    await articlePage.bringToFront();
    await expect(articlePage.locator("#starlee-save-button")).toBeVisible({
      timeout: 10_000,
    });

    const requestId = randomUUID();
    _pendingRequest = { id: requestId, source: "menu-bar" };

    // Trigger several poll cycles in quick succession
    const optPage = await context.newPage();
    await optPage.goto(`chrome-extension://${extensionId}/options.html`, {
      waitUntil: "domcontentloaded",
    });
    for (let i = 0; i < 5; i++) {
      await optPage.evaluate(() =>
        chrome.runtime.sendMessage({ type: "STARLEE_STATUS" }).catch(() => {})
      );
      await waitMs(600);
    }
    await optPage.close();

    // Wait for the first result then give extra time for any duplicate
    await waitFor(() => _captureRequestResults.length > 0, 20_000);
    await waitMs(5_000); // extra window to catch any second processing
    _pendingRequest = null;

    const forId = _captureRequestResults.filter((r) => r.id === requestId);
    expect(forId.length).toBe(1);

    await articlePage.close();
  });
});
