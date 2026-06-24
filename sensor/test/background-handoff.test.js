import test from "node:test";
import assert from "node:assert/strict";
import {
  activeTabProblem,
  classifyContentScriptMessageError,
  CONTENT_SCRIPT_UNREACHABLE,
  probeContentScriptReadiness,
  safeTabMetadata,
  sendCaptureMessageToContentScript,
  supportedContentScriptUrl
} from "../src/background-handoff.js";

const request = { id: "req123", source: "menu-bar" };

test("active tab diagnostics classify missing and unsupported URLs", () => {
  assert.deepEqual(activeTabProblem({ id: 7 }), {
    event: "active_tab_missing_url",
    status: "permission_denied",
    message: "Safari did not expose the active tab URL to Starlee."
  });
  assert.deepEqual(activeTabProblem({ id: 7, url: "safari-web-extension://settings" }), {
    event: "active_tab_unsupported_url",
    status: "unsupported_page",
    message: "The active page cannot run the Starlee content script."
  });
  assert.equal(activeTabProblem({ id: 7, url: "https://www.youtube.com/watch?v=abc123" }), null);
  assert.equal(supportedContentScriptUrl("https://www.youtube.com/watch?v=abc123"), true);
  assert.equal(supportedContentScriptUrl("chrome://extensions"), false);
});

test("sendMessage no receiver returns actionable content script failure", async () => {
  const diagnostics = [];
  const result = await sendCaptureMessageToContentScript({
    tab: { id: 7, url: "https://www.youtube.com/watch?v=abc123" },
    request,
    messageType: "STARLEE_CAPTURE_NOW",
    browserName: "Safari",
    recordDiagnostic: (event) => diagnostics.push(event),
    sendMessage: async () => undefined
  });

  assert.equal(result.ok, false);
  assert.equal(result.code, CONTENT_SCRIPT_UNREACHABLE);
  assert.match(result.error, /enable the Starlee Safari extension/);
  assert.match(result.error, /allow it on youtube\.com/);
  assert.match(result.error, /reload the YouTube tab/);
  assert.deepEqual(diagnostics.map((event) => event.event), [
    "content_script_message_send_started",
    "content_script_no_receiver"
  ]);
});

test("readiness probe distinguishes reachable content scripts from no receiver", async () => {
  const diagnostics = [];
  const ready = await probeContentScriptReadiness({
    tab: { id: 7, url: "https://www.youtube.com/watch?v=abc123" },
    request,
    messageType: "STARLEE_CONTENT_SCRIPT_PING",
    browserName: "Safari",
    recordDiagnostic: (event) => diagnostics.push(event),
    sendMessage: async () => ({
      ok: true,
      ready: true,
      code: "content_script_ready",
      page_type: "youtube"
    })
  });

  assert.equal(ready.ok, true);
  assert.equal(ready.ready, true);
  assert.deepEqual(diagnostics.map((event) => event.event), [
    "content_script_probe_started",
    "content_script_probe_succeeded"
  ]);
  assert.equal(diagnostics.at(-1).safe_metadata.page_type, "youtube");

  const failedDiagnostics = [];
  const unreachable = await probeContentScriptReadiness({
    tab: { id: 7, url: "https://www.youtube.com/watch?v=abc123" },
    request,
    messageType: "STARLEE_CONTENT_SCRIPT_PING",
    browserName: "Safari",
    recordDiagnostic: (event) => failedDiagnostics.push(event),
    sendMessage: async () => undefined
  });

  assert.equal(unreachable.ok, false);
  assert.equal(unreachable.code, CONTENT_SCRIPT_UNREACHABLE);
  assert.deepEqual(failedDiagnostics.map((event) => event.event), [
    "content_script_probe_started",
    "content_script_probe_no_receiver"
  ]);
});

test("sendMessage rejected promise records redacted no-receiver diagnostics", async () => {
  const diagnostics = [];
  const result = await sendCaptureMessageToContentScript({
    tab: {
      id: 7,
      url: "https://www.youtube.com/watch?v=abc123&token=secret",
      title: "Private transcript text"
    },
    request,
    messageType: "STARLEE_CAPTURE_NOW",
    browserName: "Safari",
    recordDiagnostic: (event) => diagnostics.push(event),
    sendMessage: async () => {
      throw new Error("Could not establish connection. Receiving end does not exist. https://private.example/token");
    }
  });

  assert.equal(result.ok, false);
  assert.equal(result.code, CONTENT_SCRIPT_UNREACHABLE);
  assert.equal(diagnostics.at(-1).event, "content_script_no_receiver");
  const serialized = JSON.stringify(diagnostics);
  assert.equal(serialized.includes("secret"), false);
  assert.equal(serialized.includes("Private transcript text"), false);
  assert.equal(serialized.includes("https://private.example/token"), false);
});

test("sendMessage lastError-style message is classified as no receiver", () => {
  const classified = classifyContentScriptMessageError("The message port closed before a response was received.");

  assert.equal(classified.event, "content_script_no_receiver");
  assert.equal(classified.status, CONTENT_SCRIPT_UNREACHABLE);
});

test("content script returned failure is recorded separately from transport success", async () => {
  const diagnostics = [];
  const result = await sendCaptureMessageToContentScript({
    tab: { id: 7, url: "https://www.youtube.com/watch?v=abc123" },
    request,
    messageType: "STARLEE_CAPTURE_NOW",
    browserName: "Safari",
    recordDiagnostic: (event) => diagnostics.push(event),
    sendMessage: async () => ({ ok: false, code: "unsupported_page", error: "Unsupported page" })
  });

  assert.equal(result.ok, false);
  assert.deepEqual(diagnostics.map((event) => event.event), [
    "content_script_message_send_started",
    "content_script_message_send_succeeded",
    "content_script_returned_failure"
  ]);
});

test("successful content script response flows normally", async () => {
  const diagnostics = [];
  const result = await sendCaptureMessageToContentScript({
    tab: { id: 7, url: "https://www.youtube.com/watch?v=abc123" },
    request,
    messageType: "STARLEE_CAPTURE_NOW",
    browserName: "Safari",
    recordDiagnostic: (event) => diagnostics.push(event),
    sendMessage: async () => ({ ok: true, code: "capture_saved", record: { id: "saved" } })
  });

  assert.equal(result.ok, true);
  assert.deepEqual(diagnostics.map((event) => event.event), [
    "content_script_message_send_started",
    "content_script_message_send_succeeded"
  ]);
});

test("safe tab metadata excludes page URLs, titles, bodies, transcripts, and tokens", () => {
  const metadata = safeTabMetadata({
    id: 7,
    title: "Private transcript title",
    url: "https://www.youtube.com/watch?v=abc123&token=super-secret"
  });
  const serialized = JSON.stringify(metadata);

  assert.equal(metadata.domain, "youtube.com");
  assert.equal(metadata.url_scheme, "https");
  assert.equal(serialized.includes("watch?v="), false);
  assert.equal(serialized.includes("super-secret"), false);
  assert.equal(serialized.includes("Private transcript title"), false);
});
