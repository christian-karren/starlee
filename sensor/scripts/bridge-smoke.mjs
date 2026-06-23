import { readFile } from "node:fs/promises";
import { JSDOM } from "jsdom";
import { capturePayload } from "../src/payload.js";

const [address, token, fixturePath] = process.argv.slice(2);
if (!address || !token || !fixturePath) {
  throw new Error("usage: node bridge-smoke.mjs 127.0.0.1:PORT TOKEN fixture.html");
}

const storage = {};
const processedRequests = new Set();
const fixture = await readFile(fixturePath, "utf8");
const dom = new JSDOM(fixture, {
  url: "http://127.0.0.1:4173/test/fixture.html",
});

const headers = {
  Authorization: `Bearer ${token}`,
  "Content-Type": "application/json",
};

await post("/extension/hello", {
  browser: "BridgeSmoke",
  extension_version: "0.1.0",
  can_capture_active_tab: true,
});
const created = await post("/capture-request", { source: "menu-bar" }, 202);
const pickup = await takeCaptureRequest();
const saved = await processRequest(pickup.request);
const duplicate = await processRequest(pickup.request);
const secondPickup = await takeCaptureRequest();

process.stdout.write(JSON.stringify({
  created,
  pickup,
  saved,
  duplicate,
  secondPickup,
  storage,
}));

async function processRequest(request) {
  if (!request?.id) return { ok: false, skipped: "missing_request" };
  if (processedRequests.has(request.id)) {
    return { ok: true, skipped: "duplicate" };
  }
  processedRequests.add(request.id);
  storage.lastMenuRequestId = request.id;
  storage.lastMenuRequestStatus = "picked_up";

  const payload = capturePayload(dom.window.document);
  const captureResponse = await post("/capture", payload, 201);
  const result = {
    id: request.id,
    status: "capture_saved",
    source: request.source || "menu-bar",
    record: {
      metadata: {
        id: captureResponse.metadata.id,
        title: captureResponse.metadata.title,
        url: captureResponse.metadata.url,
      },
    },
  };
  const terminal = await post("/capture-request/result", result);
  storage.lastMenuRequestStatus = terminal.status;
  return { ok: true, terminal };
}

async function takeCaptureRequest() {
  const response = await fetch(`http://${address}/capture-request`, { headers });
  assertStatus(response, 200);
  return response.json();
}

async function post(path, body, expected = 200) {
  const response = await fetch(`http://${address}${path}`, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });
  assertStatus(response, expected);
  return response.json();
}

function assertStatus(response, expected) {
  if (response.status !== expected) {
    throw new Error(`Expected ${expected} from ${response.url}, got ${response.status}`);
  }
}
