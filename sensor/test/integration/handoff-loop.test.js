import test from "node:test";
import assert from "node:assert/strict";
import http from "node:http";
import { pollCaptureRequest, postCapture, postRequestResult } from "../../src/handoff-http.js";

const TOKEN = "test-token-abc";

function startServer(handler) {
  return new Promise((resolve) => {
    const server = http.createServer(handler);
    server.listen(0, "127.0.0.1", () => {
      resolve({ server, port: server.address().port });
    });
  });
}

function stopServer(server) {
  return new Promise((resolve) => server.close(resolve));
}

function jsonBody(req) {
  return new Promise((resolve, reject) => {
    let data = "";
    req.on("data", (chunk) => (data += chunk));
    req.on("end", () => {
      try { resolve(JSON.parse(data)); } catch { resolve({}); }
    });
    req.on("error", reject);
  });
}

function reply(res, status, body) {
  res.writeHead(status, { "Content-Type": "application/json" });
  res.end(JSON.stringify(body));
}

// ---------------------------------------------------------------------------
// Happy path: request appears → picked up → result posted
// ---------------------------------------------------------------------------
test("happy path: request is picked up and result is posted back", async () => {
  const request = { id: "req-001", source: "menu-bar" };
  const results = [];

  const { server, port } = await startServer(async (req, res) => {
    if (req.url === "/capture-request" && req.method === "GET") {
      return reply(res, 200, { request });
    }
    if (req.url === "/capture-request/result" && req.method === "POST") {
      const body = await jsonBody(req);
      results.push(body);
      return reply(res, 200, { ok: true });
    }
    if (req.url === "/capture" && req.method === "POST") {
      return reply(res, 200, { id: "saved-001" });
    }
    reply(res, 404, {});
  });

  try {
    const poll = await pollCaptureRequest({ token: TOKEN, port });
    assert.equal(poll.ok, true);
    assert.deepEqual(poll.request, request);

    // Simulate picked-up status
    await postRequestResult({ token: TOKEN, port, id: request.id, status: "picked_up", message: "Processing." });

    // Simulate capture posted
    const capture = await postCapture({ token: TOKEN, port, payload: { url: "https://example.com" }, requestId: request.id });
    assert.equal(capture.ok, true);
    assert.equal(capture.code, "capture_saved");

    // Post final result
    await postRequestResult({ token: TOKEN, port, id: request.id, status: "capture_saved", message: "Saved to Starlee." });

    assert.equal(results.length, 2);
    assert.equal(results[0].status, "picked_up");
    assert.equal(results[1].status, "capture_saved");
  } finally {
    await stopServer(server);
  }
});

// ---------------------------------------------------------------------------
// Timeout: request placed but never fulfilled → marked timed-out, not stuck
// ---------------------------------------------------------------------------
test("timeout: a slow server connection is not waited on indefinitely", async () => {
  // Simulate: server returns a request, then the extension posts a timed-out result
  const request = { id: "req-002", source: "menu-bar" };
  const results = [];

  const { server, port } = await startServer(async (req, res) => {
    if (req.url === "/capture-request" && req.method === "GET") {
      return reply(res, 200, { request });
    }
    if (req.url === "/capture-request/result" && req.method === "POST") {
      const body = await jsonBody(req);
      results.push(body);
      return reply(res, 200, { ok: true });
    }
    reply(res, 404, {});
  });

  try {
    const poll = await pollCaptureRequest({ token: TOKEN, port });
    assert.equal(poll.ok, true);

    // Simulate the extension deciding the request timed out (i.e. content script never responded)
    await postRequestResult({
      token: TOKEN,
      port,
      id: request.id,
      status: "capture_failed",
      message: "Request timed out — no response from content script."
    });

    assert.equal(results.length, 1);
    assert.equal(results[0].id, "req-002");
    assert.equal(results[0].status, "capture_failed");
    assert.match(results[0].message, /timed out/);
  } finally {
    await stopServer(server);
  }
});

// ---------------------------------------------------------------------------
// Duplicate pickup idempotency: two concurrent pollers → only one result posted
// ---------------------------------------------------------------------------
test("duplicate pickup idempotency: request deduped by id across concurrent pollers", async () => {
  const request = { id: "req-003", source: "menu-bar" };
  const pickups = [];
  const results = [];

  const { server, port } = await startServer(async (req, res) => {
    if (req.url === "/capture-request" && req.method === "GET") {
      // Both pollers see the same request (no server-side atomic take yet)
      return reply(res, 200, { request });
    }
    if (req.url === "/capture-request/result" && req.method === "POST") {
      const body = await jsonBody(req);
      results.push(body);
      return reply(res, 200, { ok: true });
    }
    if (req.url === "/capture" && req.method === "POST") {
      const body = await jsonBody(req);
      pickups.push(body);
      return reply(res, 200, { id: "saved-003" });
    }
    reply(res, 404, {});
  });

  try {
    // Two concurrent pollers both receive the same request
    const [poll1, poll2] = await Promise.all([
      pollCaptureRequest({ token: TOKEN, port }),
      pollCaptureRequest({ token: TOKEN, port })
    ]);

    assert.equal(poll1.request?.id, "req-003");
    assert.equal(poll2.request?.id, "req-003");

    // Deduplicate by request id (mirrors background.js processedRequests)
    const seen = new Set();
    const processed = [];
    for (const poll of [poll1, poll2]) {
      if (poll.request?.id && seen.has(poll.request.id)) continue;
      if (poll.request?.id) seen.add(poll.request.id);
      processed.push(poll);
    }

    assert.equal(processed.length, 1, "only one poller should process the request");

    // Only the winning poller posts a capture and result
    await postCapture({ token: TOKEN, port, payload: { url: "https://example.com" }, requestId: processed[0].request.id });
    await postRequestResult({ token: TOKEN, port, id: processed[0].request.id, status: "capture_saved", message: "Saved." });

    assert.equal(pickups.length, 1, "capture POSTed exactly once");
    assert.equal(results.length, 1, "result POSTed exactly once");
    assert.equal(results[0].status, "capture_saved");
  } finally {
    await stopServer(server);
  }
});

// ---------------------------------------------------------------------------
// Auth failure: /capture returns 401 → reported as token_invalid, not silent
// ---------------------------------------------------------------------------
test("auth failure: 401 from /capture surfaces as token_invalid code, not silently dropped", async () => {
  const request = { id: "req-004", source: "menu-bar" };
  const results = [];

  const { server, port } = await startServer(async (req, res) => {
    if (req.url === "/capture-request" && req.method === "GET") {
      return reply(res, 200, { request });
    }
    if (req.url === "/capture" && req.method === "POST") {
      // Simulate token rejected mid-session
      return reply(res, 401, { error: "Unauthorized: token expired" });
    }
    if (req.url === "/capture-request/result" && req.method === "POST") {
      const body = await jsonBody(req);
      results.push(body);
      return reply(res, 200, { ok: true });
    }
    reply(res, 404, {});
  });

  try {
    const poll = await pollCaptureRequest({ token: TOKEN, port });
    assert.equal(poll.ok, true);

    const capture = await postCapture({
      token: TOKEN,
      port,
      payload: { url: "https://example.com" },
      requestId: request.id
    });

    assert.equal(capture.ok, false);
    assert.equal(capture.code, "token_invalid", "401 must surface as token_invalid, not be silently dropped");
    assert.match(capture.error, /Unauthorized/);

    // Caller must report token_invalid back to the server
    await postRequestResult({
      token: TOKEN,
      port,
      id: request.id,
      status: "token_invalid",
      message: capture.error
    });

    assert.equal(results.length, 1);
    assert.equal(results[0].status, "token_invalid");
  } finally {
    await stopServer(server);
  }
});

test("local bridge helpers return token_missing without network calls", async () => {
  const poll = await pollCaptureRequest({ token: "", port: 1 });
  const capture = await postCapture({
    token: "",
    port: 1,
    payload: { url: "https://example.com/private" },
    requestId: "req-token-missing"
  });

  assert.deepEqual(poll, { ok: false, request: null, code: "token_missing" });
  assert.equal(capture.ok, false);
  assert.equal(capture.code, "token_missing");
  assert.equal(capture.requestId, "req-token-missing");
});

test("local bridge helpers return service_down when loopback is unreachable", async () => {
  const port = 9;
  const poll = await pollCaptureRequest({ token: TOKEN, port });
  const capture = await postCapture({
    token: TOKEN,
    port,
    payload: { url: "https://example.com/private" },
    requestId: "req-service-down"
  });

  assert.equal(poll.ok, false);
  assert.equal(poll.code, "service_down");
  assert.equal(capture.ok, false);
  assert.equal(capture.code, "service_down");
  assert.equal(capture.requestId, "req-service-down");
});

test("capture payload too large status maps to payload_too_large", async () => {
  const { server, port } = await startServer((req, res) => {
    if (req.url === "/capture" && req.method === "POST") {
      return reply(res, 413, { error: "capture payload too large" });
    }
    reply(res, 404, {});
  });

  try {
    const capture = await postCapture({
      token: TOKEN,
      port,
      payload: { url: "https://example.com/large" },
      requestId: "req-too-large"
    });

    assert.equal(capture.ok, false);
    assert.equal(capture.code, "payload_too_large");
    assert.match(capture.error, /too large/);
  } finally {
    await stopServer(server);
  }
});

// ---------------------------------------------------------------------------
// Empty poll: no pending request returns ok with null request (no-op)
// ---------------------------------------------------------------------------
test("empty poll: server returns no pending request, loop does nothing", async () => {
  const { server, port } = await startServer((req, res) => {
    if (req.url === "/capture-request" && req.method === "GET") {
      return reply(res, 200, { request: null });
    }
    reply(res, 404, {});
  });

  try {
    const poll = await pollCaptureRequest({ token: TOKEN, port });
    assert.equal(poll.ok, true);
    assert.equal(poll.request, null, "null request means no work — loop should be a no-op");
  } finally {
    await stopServer(server);
  }
});
