import http from "node:http";
import { spawn } from "node:child_process";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { randomUUID } from "node:crypto";

const sensorRoot = fileURLToPath(new URL("../", import.meta.url));
const firefoxDist = path.join(sensorRoot, "dist/firefox-extension");
const token = "firefox-smoke-token";
const port = Number(process.env.STARLEE_FIREFOX_SMOKE_PORT || 47319);
const firefoxBinary = process.env.FIREFOX_BIN || "";
const timeoutMs = Number(process.env.STARLEE_FIREFOX_SMOKE_TIMEOUT_MS || 45_000);

const state = {
  mode: "",
  captures: [],
  requestResults: [],
  diagnostics: [],
  hellos: 0,
  pendingRequest: null
};

await exec(process.execPath, ["scripts/build.mjs", "--target", "firefox"], { cwd: sensorRoot });
await mkdir(firefoxDist, { recursive: true });
await writeFile(
  path.join(firefoxDist, "starlee-config.json"),
  JSON.stringify({ captureToken: token, capturePort: port }, null, 2)
);

const server = await startServer();
try {
  await runToolbarSmoke();
  await runMenuBarSmoke();
  assertDiagnosticsRedacted();
  console.log(JSON.stringify({
    ok: true,
    firefox: firefoxBinary || "web-ext default",
    port,
    captures: state.captures.length,
    request_results: state.requestResults.length,
    diagnostics: state.diagnostics.length
  }, null, 2));
} finally {
  await closeServer(server);
  await rm(path.join(firefoxDist, "starlee-config.json"), { force: true });
}

async function runToolbarSmoke() {
  reset("toolbar");
  await withFirefox(`/article-toolbar.html`, async () => {
    const capture = await waitFor(() => state.captures.find((entry) => entry.source === "toolbar"), timeoutMs);
    assertArticlePayload(capture?.payload, {
      source: "toolbar",
      selectedText: "selected Firefox smoke quote"
    });
  });
}

async function runMenuBarSmoke() {
  reset("menu-bar");
  state.pendingRequest = {
    id: `firefox-smoke-${randomUUID()}`,
    source: "menu-bar",
    created_at: new Date().toISOString()
  };
  await withFirefox(`/article-menu.html`, async () => {
    const saved = await waitFor(
      () => state.requestResults.find((entry) => entry.id === state.pendingRequest.id && entry.status === "capture_saved"),
      timeoutMs
    );
    if (!saved) {
      throw new Error(`Firefox menu-bar smoke did not report capture_saved. Results: ${JSON.stringify(state.requestResults)}`);
    }
    const capture = state.captures.find((entry) => entry.source === "menu-bar");
    assertArticlePayload(capture?.payload, { source: "menu-bar" });
  });
}

async function withFirefox(startPath, callback) {
  const startUrl = `http://127.0.0.1:${port}${startPath}`;
  const profile = `/tmp/starlee-firefox-smoke-${randomUUID()}`;
  await mkdir(profile, { recursive: true });
  const args = [
    "--yes",
    "web-ext",
    "run",
    "--source-dir",
    firefoxDist,
    "--firefox-profile",
    profile,
    "--start-url",
    startUrl,
    "--no-input"
  ];
  if (firefoxBinary) {
    args.splice(5, 0, "--firefox", firefoxBinary);
  }
  const child = spawn("npx", args, {
    cwd: sensorRoot,
    detached: true,
    stdio: ["ignore", "pipe", "pipe"]
  });
  let output = "";
  child.stdout.on("data", (chunk) => {
    output += String(chunk);
  });
  child.stderr.on("data", (chunk) => {
    output += String(chunk);
  });
  const exitPromise = new Promise((resolve) => {
    child.once("exit", (code, signal) => resolve({ code, signal }));
  });
  try {
    await callback();
  } catch (error) {
    const earlyExit = await Promise.race([exitPromise, wait(1_000).then(() => null)]);
    if (earlyExit) {
      throw new Error(`${error.message}\nweb-ext exited early: ${JSON.stringify(earlyExit)}\n${output}`);
    }
    throw new Error(`${error.message}\nweb-ext output:\n${output}`);
  } finally {
    try {
      process.kill(-child.pid, "SIGTERM");
    } catch {}
    await Promise.race([exitPromise, wait(5_000)]);
    try {
      process.kill(-child.pid, "SIGKILL");
    } catch {}
    await rm(profile, { recursive: true, force: true });
  }
}

function reset(mode) {
  state.mode = mode;
  state.captures.length = 0;
  state.requestResults.length = 0;
  state.diagnostics.length = 0;
  state.hellos = 0;
  state.pendingRequest = null;
}

function startServer() {
  return new Promise((resolve, reject) => {
    const server = http.createServer(async (req, res) => {
      const url = new URL(req.url || "/", `http://127.0.0.1:${port}`);
      if (req.method === "GET" && url.pathname === "/article-toolbar.html") {
        return jsonHtml(res, articleHtml({ autoClick: true }));
      }
      if (req.method === "GET" && url.pathname === "/article-menu.html") {
        return jsonHtml(res, articleHtml({ autoClick: false }));
      }
      const auth = req.headers.authorization || "";
      if (auth !== `Bearer ${token}`) {
        return json(res, 401, { error: "unauthorized" });
      }
      const body = await readBody(req);
      if (req.method === "POST" && url.pathname === "/extension/hello") {
        state.hellos += 1;
        return json(res, 200, { ok: true, service: "starlee-firefox-smoke" });
      }
      if (req.method === "GET" && url.pathname === "/capture-request") {
        return json(res, 200, { request: state.mode === "menu-bar" ? state.pendingRequest : null });
      }
      if (req.method === "POST" && url.pathname === "/capture") {
        state.captures.push({
          source: state.mode,
          payload: JSON.parse(body || "{}")
        });
        return json(res, 200, { ok: true, id: randomUUID() });
      }
      if (req.method === "POST" && url.pathname === "/capture-request/result") {
        state.requestResults.push(JSON.parse(body || "{}"));
        return json(res, 200, { ok: true });
      }
      if (req.method === "POST" && url.pathname === "/capture-diagnostics/event") {
        state.diagnostics.push(JSON.parse(body || "{}"));
        return json(res, 200, { ok: true });
      }
      if (req.method === "GET" && url.pathname === "/bridge-health") {
        return json(res, 200, { bridge_health: { ok: true, recommended_next_action: "none" } });
      }
      return json(res, 404, { error: "not_found" });
    });
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => resolve(server));
  });
}

function articleHtml({ autoClick }) {
  const clickScript = autoClick ? `
    const quote = document.querySelector("#selected-quote");
    const range = document.createRange();
    range.selectNodeContents(quote);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    const timer = setInterval(() => {
      const button = document.querySelector("#starlee-save-button");
      if (!button) return;
      clearInterval(timer);
      button.click();
    }, 250);
    setTimeout(() => clearInterval(timer), 15000);
  ` : "";
  const paragraphs = Array.from({ length: 8 }, (_, index) => `
    <p>
      This local Firefox smoke article paragraph ${index + 1} exercises real rendered article extraction for Starlee.
      It contains enough readable body text for Mozilla Readability to produce a stable payload without depending on a
      remote site, login state, or third-party scripts.
    </p>
  `).join("\n");
  return `<!doctype html>
    <html>
      <head>
        <meta charset="utf-8">
        <title>Firefox Smoke Article</title>
        <link rel="canonical" href="http://127.0.0.1:${port}/canonical-firefox-smoke">
        <meta name="author" content="Starlee Smoke Author">
        <meta property="og:site_name" content="Starlee Smoke Site">
        <script type="application/ld+json">{"@context":"https://schema.org","@type":"NewsArticle","isAccessibleForFree":true}</script>
      </head>
      <body>
        <article>
          <h1>Firefox Smoke Article</h1>
          <p id="selected-quote">selected Firefox smoke quote</p>
          ${paragraphs}
        </article>
        <script>${clickScript}</script>
      </body>
    </html>`;
}

function assertArticlePayload(payload, { source, selectedText } = {}) {
  if (!payload) throw new Error(`Firefox ${source} smoke did not post a capture payload.`);
  if (payload.type !== "article") throw new Error(`Expected article payload, got ${payload.type}`);
  if (payload.url !== `http://127.0.0.1:${port}/canonical-firefox-smoke`) throw new Error(`Unexpected canonical URL: ${payload.url}`);
  if (payload.dom_extract?.title !== "Firefox Smoke Article") throw new Error(`Unexpected title: ${payload.dom_extract?.title}`);
  if (!payload.dom_extract?.text?.includes("local Firefox smoke article paragraph 4")) throw new Error("Article body text was not extracted.");
  if (payload.dom_extract?.byline !== "Starlee Smoke Author") throw new Error(`Unexpected byline: ${payload.dom_extract?.byline}`);
  if (payload.dom_extract?.site !== "Starlee Smoke Site") throw new Error(`Unexpected site name: ${payload.dom_extract?.site}`);
  if (payload.access !== "public") throw new Error(`Unexpected access classification: ${payload.access}`);
  if (!Array.isArray(payload.tags)) throw new Error(`Tags must be an array: ${JSON.stringify(payload.tags)}`);
  if (!payload.consumed_at) throw new Error("Missing consumed_at timestamp.");
  if (selectedText && payload.dom_extract?.selected_text !== selectedText) {
    throw new Error(`Selected text mismatch: ${payload.dom_extract?.selected_text}`);
  }
}

function assertDiagnosticsRedacted() {
  const serialized = JSON.stringify(state.diagnostics);
  for (const forbidden of [
    token,
    "local Firefox smoke article paragraph 4",
    "selected Firefox smoke quote"
  ]) {
    if (serialized.includes(forbidden)) {
      throw new Error(`Diagnostic payload leaked forbidden text: ${forbidden}`);
    }
  }
}

function jsonHtml(res, body) {
  res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
  res.end(body);
}

function json(res, status, body) {
  res.writeHead(status, { "Content-Type": "application/json" });
  res.end(JSON.stringify(body));
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    let body = "";
    req.on("data", (chunk) => {
      body += String(chunk);
    });
    req.on("end", () => resolve(body));
    req.on("error", reject);
  });
}

function closeServer(server) {
  return new Promise((resolve) => server.close(resolve));
}

function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitFor(fn, timeout, interval = 500) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const value = await fn();
    if (value) return value;
    await wait(interval);
  }
  return null;
}

function exec(command, args, options) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { ...options, stdio: ["ignore", "pipe", "pipe"] });
    let output = "";
    child.stdout.on("data", (chunk) => {
      output += String(chunk);
    });
    child.stderr.on("data", (chunk) => {
      output += String(chunk);
    });
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) resolve(output);
      else reject(new Error(`${command} ${args.join(" ")} exited ${code}\n${output}`));
    });
  });
}
