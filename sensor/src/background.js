import { browserNameFromUserAgent, createExtensionApi, requestTargetsBrowser } from "./browser.js";
import {
  activeTabLookupFailure,
  activeTabProblem,
  classifyContentScriptMessageError,
  CONTENT_SCRIPT_UNREACHABLE,
  contentScriptFailureResult,
  probeContentScriptReadiness,
  safeTabMetadata,
  safeTabPage,
  sendCaptureMessageToContentScript
} from "./background-handoff.js";

const ext = createExtensionApi();
const DEFAULT_PORT = 47291;
// Alarm at the MV3 minimum so a fully-evicted worker re-wakes quickly and finds a
// still-pending capture request (paired with the engine's longer request TTL).
const DEFAULT_POLL_MINUTES = 0.5;
const FALLBACK_POLL_SECONDS = 3;
const KEEPALIVE_PORT = "starlee-keepalive";
const BADGE_CLEAR_MS = 3500;
const ALARM_NAME = "starlee-poll";
const HELLO_REFRESH_MS = 5000;
const MESSAGE = Object.freeze({
  capture: "STARLEE_CAPTURE",
  status: "STARLEE_STATUS",
  hello: "STARLEE_HELLO",
  pingContentScript: "STARLEE_CONTENT_SCRIPT_PING",
  captureNow: "STARLEE_CAPTURE_NOW",
  bridgeHealth: "STARLEE_BRIDGE_HEALTH",
  diagnostic: "STARLEE_CAPTURE_DIAGNOSTIC"
});
const CAPTURE_STATUS = Object.freeze({
  saved: "capture_saved",
  failed: "capture_failed",
  pickedUp: "picked_up",
  extracting: "extracting",
  posted: "posted",
  permissionDenied: "permission_denied",
  unsupportedPage: "unsupported_page",
  contentScriptUnreachable: CONTENT_SCRIPT_UNREACHABLE
});
let bundledConfigPromise;
let buildInfoPromise;
let polling = false;
let lastHelloAt = 0;
const processedRequests = new Set();

ext.runtime.onMessage.addListener(handleMessage);

function handleMessage(message, _sender, sendResponse) {
  if (message?.type === MESSAGE.capture) {
    sendCapture(message.payload, {
      source: message.source || "content-script",
      requestId: message.requestId
    }).then(sendResponse);
    return true;
  }
  if (message?.type === MESSAGE.status) {
    status().then(sendResponse);
    return true;
  }
  if (message?.type === MESSAGE.hello) {
    hello({ force: true }).then(sendResponse);
    return true;
  }
  if (message?.type === MESSAGE.bridgeHealth) {
    bridgeHealth().then(sendResponse);
    return true;
  }
  if (message?.type === MESSAGE.diagnostic) {
    recordDiagnosticEvent(message.event).then(sendResponse);
    return true;
  }
}

ext.action.onClicked.addListener(async (tab) => {
  const result = await captureTab(tab);
  await showResult(result);
});

// Top-level listeners so a re-spawned service worker rewires itself immediately on
// any wake (browser start, install, alarm, or keep-alive reconnect) — never inside
// an async function, which would miss events that woke the worker.
ext.runtime.onStartup?.addListener?.(startLocalBridge);
ext.runtime.onInstalled?.addListener?.(startLocalBridge);
ext.alarms?.onAlarm?.addListener?.((alarm) => {
  if (alarm.name === ALARM_NAME) pollCaptureRequest();
});
ext.runtime.onConnect?.addListener?.((port) => {
  if (port.name === KEEPALIVE_PORT) {
    // Holding the port open defers worker eviction; it auto-closes after ~5 min
    // and keepAlive() reconnects, so the worker stays warm and keeps polling.
    port.onDisconnect.addListener(() => {});
  }
});

startLocalBridge();

async function sendCapture(payload, options = {}) {
  const { token, port } = await localSettings();
  if (!token) {
    const result = errorResult("token_missing", "Open Starlee Capture settings and connect the local Starlee app.");
    await recordCaptureResult(result, options.source);
    await recordCaptureRequestResult(options.requestId, result);
    return result;
  }
  try {
    await recordCaptureRequestStatus(options.requestId, CAPTURE_STATUS.posted, "Browser extension posted the capture to Starlee.", safePageMetadata(payload));
    await recordDiagnosticEvent({
      component: "extension",
      event: "capture_payload_posted",
      request_id: options.requestId,
      status: CAPTURE_STATUS.posted,
      source: options.source,
      browser: browserName(),
      message: "Browser extension posted the capture to Starlee.",
      page: safePageMetadata(payload),
      safe_metadata: {
        payload_type: payload?.type || "unknown",
        transcript_segment_count: String(payload?.transcript?.length || 0)
      }
    });
    const response = await fetch(`http://127.0.0.1:${port}/capture`, {
      method: "POST",
      headers: { "Authorization": `Bearer ${token}`, "Content-Type": "application/json" },
      body: JSON.stringify(payload)
    });
    const body = await response.json().catch(() => ({}));
    if (!response.ok) {
      const code = response.status === 401
        ? "token_invalid"
        : response.status === 413
          ? "payload_too_large"
          : "capture_failed";
      const result = errorResult(code, body.error || `Starlee rejected the capture with HTTP ${response.status}.`);
      await recordDiagnosticEvent({
        component: "extension",
        event: "capture_failed",
        request_id: options.requestId,
        status: result.code,
        source: options.source,
        browser: browserName(),
        message: result.error,
        page: safePageMetadata(payload),
        safe_metadata: { http_status: String(response.status) }
      });
      await recordCaptureResult(result, options.source);
      await recordCaptureRequestResult(options.requestId, result);
      return result;
    }
    const result = { ok: true, code: "capture_saved", record: body };
    await recordDiagnosticEvent({
      component: "extension",
      event: "capture_saved",
      request_id: options.requestId,
      status: CAPTURE_STATUS.saved,
      source: options.source,
      browser: browserName(),
      message: "Saved to Starlee.",
      page: safePageMetadata(payload)
    });
    await recordCaptureResult(result, options.source);
    await recordCaptureRequestResult(options.requestId, result);
    return result;
  } catch {
    const result = errorResult("service_down", "Local Starlee is not reachable. Open Starlee or run starlee serve.");
    await recordDiagnosticEvent({
      component: "extension",
      event: "capture_failed",
      request_id: options.requestId,
      status: result.code,
      source: options.source,
      browser: browserName(),
      message: result.error,
      page: safePageMetadata(payload)
    });
    await recordCaptureResult(result, options.source);
    await recordCaptureRequestResult(options.requestId, result);
    return result;
  }
}

async function captureTab(tab) {
  const problem = activeTabProblem(tab);
  if (problem) return errorResult(problem.status, problem.message);
  try {
    return await ext.tabs.sendMessage(tab.id, { type: MESSAGE.captureNow });
  } catch (error) {
    const classified = classifyContentScriptMessageError(error);
    return classified.status === CAPTURE_STATUS.permissionDenied
      ? errorResult("permission_denied", `${browserName()} has not granted Starlee access to this page, or this page cannot run extensions.`)
      : contentScriptFailureResult(browserName());
  }
}

async function startLocalBridge() {
  // Always (re)arm the alarm and keep-alive, even if a poll loop is already running
  // in this worker instance, so a freshly-woken worker is fully wired.
  ext.alarms?.create?.(ALARM_NAME, { periodInMinutes: DEFAULT_POLL_MINUTES });
  keepAlive();
  if (polling) return;
  polling = true;
  await hello();
  await pollCaptureRequest();
  setInterval(pollCaptureRequest, FALLBACK_POLL_SECONDS * 1000);
}

// Self-connecting keep-alive: an open runtime port resets the worker's idle timer.
// MV3 force-closes ports after ~5 minutes, so reconnect on disconnect to stay warm.
function keepAlive() {
  try {
    const port = ext.runtime.connect({ name: KEEPALIVE_PORT });
    port.onDisconnect.addListener(() => {
      // Chrome reports benign MV3 service-worker port closures through
      // runtime.lastError unless the listener reads it.
      void globalThis.chrome?.runtime?.lastError;
      keepAlive();
    });
  } catch {
    // connect can throw if no receiver yet; the alarm still re-wakes the worker.
  }
}

async function hello(_options = {}) {
  const { token, port } = await localSettings();
  if (!token) {
    const result = errorResult("token_missing", "Capture token is not configured.");
    await recordHandshake(result);
    return result;
  }
  try {
    const response = await fetch(`http://127.0.0.1:${port}/extension/hello`, {
      method: "POST",
      headers: { "Authorization": `Bearer ${token}`, "Content-Type": "application/json" },
      body: JSON.stringify({
        browser: browserName(),
        extension_version: ext.runtime.getManifest().version,
        extension_build: await extensionBuildIdentity(),
        can_capture_active_tab: true
      })
    });
    const body = await response.json().catch(() => ({}));
    if (!response.ok) {
      const result = errorResult(response.status === 401 ? "token_invalid" : "service_error", body.error || `HTTP ${response.status}`);
      await recordHandshake(result);
      return result;
    }
    const result = { ok: true, code: "connected", service: body };
    lastHelloAt = Date.now();
    await recordHandshake(result);
    return result;
  } catch {
    const result = errorResult("service_down", "Local Starlee is not reachable.");
    await recordHandshake(result);
    return result;
  }
}

async function pollCaptureRequest() {
  const response = await takeCaptureRequest();
  const request = response.request;
  if (!request) return;
  const tab = await lookupActiveTabForRequest(request);
  if (tab?.result) {
    await ext.storage.local.set({
      lastMenuRequestId: request.id || "",
      lastMenuRequestAt: new Date().toISOString(),
      lastMenuRequestStatus: tab.result.code || CAPTURE_STATUS.failed
    });
    await showResult(tab.result);
    return;
  }
  const result = await captureTabForRequest(tab, request);
  await ext.storage.local.set({
    lastMenuRequestId: request.id || "",
    lastMenuRequestAt: new Date().toISOString(),
    lastMenuRequestStatus: result.ok ? CAPTURE_STATUS.saved : result.code || CAPTURE_STATUS.failed
  });
  await showResult(result);
}

async function lookupActiveTabForRequest(request) {
  const source = request.source || "menu-bar";
  await recordDiagnosticEvent({
    component: "extension",
    event: "active_tab_lookup_started",
    request_id: request.id,
    status: "started",
    source,
    browser: browserName(),
    message: "Browser extension is looking up the active tab."
  });
  try {
    const [tab] = await ext.tabs.query({ active: true, currentWindow: true });
    const problem = activeTabProblem(tab);
    if (problem) {
      const result = errorResult(problem.status, problem.message);
      await recordDiagnosticEvent({
        component: "extension",
        event: problem.event,
        request_id: request.id,
        status: problem.status,
        source,
        browser: browserName(),
        message: problem.message,
        page: safeTabPage(tab),
        safe_metadata: safeTabMetadata(tab)
      });
      await recordCaptureRequestResult(request.id, result);
      return { result };
    }
    await recordDiagnosticEvent({
      component: "extension",
      event: "active_tab_lookup_succeeded",
      request_id: request.id,
      status: "ok",
      source,
      browser: browserName(),
      message: "Browser extension found the active tab.",
      page: safeTabPage(tab),
      safe_metadata: safeTabMetadata(tab)
    });
    return tab;
  } catch (error) {
    const failure = activeTabLookupFailure(error, browserName());
    const result = errorResult(failure.status, failure.message);
    await recordDiagnosticEvent({
      component: "extension",
      event: failure.event,
      request_id: request.id,
      status: failure.status,
      source,
      browser: browserName(),
      message: failure.message,
      safe_metadata: failure.safe_metadata
    });
    await recordCaptureRequestResult(request.id, result);
    return { result };
  }
}

async function captureTabForRequest(tab, request) {
  if (!tab?.id) {
    const result = errorResult(CAPTURE_STATUS.failed, "No active browser tab is available.");
    await recordCaptureRequestResult(request.id, result);
    return result;
  }
  await recordCaptureRequestStatus(request.id, CAPTURE_STATUS.extracting, "Browser extension is extracting the active tab.");
  await recordDiagnosticEvent({
    component: "extension",
    event: "extension_extracting_active_tab",
    request_id: request.id,
    status: CAPTURE_STATUS.extracting,
    source: request.source || "menu-bar",
    browser: browserName(),
    message: "Browser extension is extracting the active tab."
  });
  const readiness = await probeContentScriptReadiness({
    tab,
    request,
    messageType: MESSAGE.pingContentScript,
    sendMessage: ext.tabs.sendMessage.bind(ext.tabs),
    recordDiagnostic: recordDiagnosticEvent,
    browserName: browserName()
  });
  if (!readiness?.ok) {
    await recordCaptureRequestResult(request.id, readiness);
    return readiness;
  }
  const result = await sendCaptureMessageToContentScript({
    tab,
    request,
    messageType: MESSAGE.captureNow,
    sendMessage: ext.tabs.sendMessage.bind(ext.tabs),
    recordDiagnostic: recordDiagnosticEvent,
    browserName: browserName()
  });
  if (!result?.ok) {
    await recordCaptureRequestResult(request.id, result);
  }
  return result;
}

async function takeCaptureRequest() {
  const { token, port } = await localSettings();
  if (!token) return { ok: false, request: null, code: "token_missing" };
  await refreshHelloIfNeeded();
  let request;
  try {
    const response = await fetch(`http://127.0.0.1:${port}/capture-request?browser=${encodeURIComponent(browserName())}`, {
      headers: { "Authorization": `Bearer ${token}` }
    });
    if (!response.ok) return { ok: false, request: null, code: `http_${response.status}` };
    request = (await response.json()).request;
  } catch {
    return { ok: false, request: null, code: "service_down" };
  }
  if (!request) return { ok: true, request: null };
  if (!requestTargetsThisBrowser(request)) {
    await recordDiagnosticEvent({
      component: "extension",
      event: "capture_request_ignored_wrong_browser",
      request_id: request.id,
      status: "ignored",
      source: request.source || "menu-bar",
      browser: browserName(),
      message: "Browser extension ignored a capture request targeted to another browser.",
      safe_metadata: {
        requested_browser: request.requested_browser || request.target_browser || "missing",
        handling_browser: browserName()
      }
    });
    return { ok: true, request: null };
  }
  if (request.id && processedRequests.has(request.id)) return { ok: true, request: null };
  if (request.id) processedRequests.add(request.id);
  await recordMenuRequest(request, CAPTURE_STATUS.pickedUp);
  await recordDiagnosticEvent({
    component: "extension",
    event: "capture_request_picked_up",
    request_id: request.id,
    status: CAPTURE_STATUS.pickedUp,
    source: request.source || "menu-bar",
    browser: browserName(),
    message: "Browser extension picked up the capture request."
  });
  return { ok: true, request };
}

async function localSettings() {
  const { captureToken = "", capturePort = 0 } = await ext.storage.local.get(["captureToken", "capturePort"]);
  const bundled = await bundledConfig();
  return {
    token: captureToken || bundled.captureToken || "",
    port: capturePort || bundled.capturePort || DEFAULT_PORT
  };
}

async function bundledConfig() {
  bundledConfigPromise ||= fetch(ext.runtime.getURL("starlee-config.json"))
    .then((response) => response.ok ? response.json() : {})
    .catch(() => ({}));
  return bundledConfigPromise;
}

async function extensionBuildIdentity() {
  const info = await buildInfo();
  const commit = String(info.git_commit || "").trim();
  const branch = String(info.git_branch || "").trim();
  const dirty = String(info.git_dirty || "").trim() === "true";
  const suffix = dirty ? "+dirty" : "";
  if (commit && commit !== "unknown" && branch && branch !== "unknown") {
    return `${branch}@${commit}${suffix}`;
  }
  if (commit && commit !== "unknown") return `${commit}${suffix}`;
  return ext.runtime.getManifest().version;
}

async function buildInfo() {
  buildInfoPromise ||= fetch(ext.runtime.getURL("build-info.json"))
    .then((response) => response.ok ? response.json() : {})
    .catch(() => ({}));
  return buildInfoPromise;
}

async function status() {
  const settings = await localSettings();
  const diagnostic = await ext.storage.local.get([
    "lastHandshakeAt",
    "lastHandshakeStatus",
    "lastHandshakeError",
    "lastCaptureAt",
    "lastCaptureStatus",
    "lastCaptureError",
    "lastMenuRequestAt",
    "lastMenuRequestStatus"
  ]);
  return {
    ok: diagnostic.lastHandshakeStatus === "connected",
    hasToken: Boolean(settings.token),
    port: settings.port,
    extensionVersion: ext.runtime.getManifest().version,
    extensionBuild: await extensionBuildIdentity(),
    browser: browserName(),
    ...diagnostic
  };
}

async function bridgeHealth() {
  const { token, port } = await localSettings();
  if (!token) {
    return {
      ok: false,
      recommended_next_action: "Open Starlee Capture settings and connect the local Starlee app."
    };
  }
  try {
    const response = await fetch(`http://127.0.0.1:${port}/bridge-health`, {
      headers: { "Authorization": `Bearer ${token}` }
    });
    const body = await response.json().catch(() => ({}));
    if (!response.ok) {
      return {
        ok: false,
        recommended_next_action: response.status === 401
          ? "Capture token was rejected by local Starlee."
          : `Local Starlee returned HTTP ${response.status}.`
      };
    }
    return body.bridge_health || {
      ok: false,
      recommended_next_action: "Run starlee doctor to inspect browser bridge health."
    };
  } catch {
    return {
      ok: false,
      recommended_next_action: "Local Starlee is not reachable. Open Starlee or run starlee serve."
    };
  }
}

async function recordHandshake(result) {
  await ext.storage.local.set({
    lastHandshakeAt: result.ok ? new Date().toISOString() : "",
    lastHandshakeStatus: result.code,
    lastHandshakeError: result.ok ? "" : result.error
  });
}

async function recordCaptureResult(result, source = "unknown") {
  await ext.storage.local.set({
    lastCaptureAt: new Date().toISOString(),
    lastCaptureSource: source,
    lastCaptureStatus: result.code || (result.ok ? CAPTURE_STATUS.saved : CAPTURE_STATUS.failed),
    lastCaptureError: result.ok ? "" : result.error
  });
}

async function recordMenuRequest(request, status) {
  await ext.storage.local.set({
    lastMenuRequestId: request.id || "",
    lastMenuRequestAt: new Date().toISOString(),
    lastMenuRequestStatus: status
  });
}

async function recordCaptureRequestResult(requestId, result) {
  if (!requestId) return;
  await recordCaptureRequestStatus(
    requestId,
    result.ok ? CAPTURE_STATUS.saved : result.code || CAPTURE_STATUS.failed,
    result.ok ? "Saved to Starlee." : result.error
  );
}

async function recordCaptureRequestStatus(requestId, status, message, page) {
  if (!requestId) return;
  const { token, port } = await localSettings();
  if (!token) return;
  await fetch(`http://127.0.0.1:${port}/capture-request/result`, {
    method: "POST",
    headers: { "Authorization": `Bearer ${token}`, "Content-Type": "application/json" },
    body: JSON.stringify({
      id: requestId,
      status,
      message,
      browser: browserName(),
      ...(page ? { page } : {})
    })
  }).catch(() => {});
}

async function recordDiagnosticEvent(event = {}) {
  if (!event.request_id) return { ok: false, code: "missing_request_id" };
  const { token, port } = await localSettings();
  if (!token) return { ok: false, code: "token_missing" };
  const payload = {
    timestamp: new Date().toISOString(),
    component: event.component || "extension",
    event: event.event || "unknown",
    request_id: event.request_id,
    status: event.status,
    source: event.source,
    browser: event.browser || browserName(),
    message: event.message,
    page: event.page,
    safe_metadata: event.safe_metadata || {}
  };
  await fetch(`http://127.0.0.1:${port}/capture-diagnostics/event`, {
    method: "POST",
    headers: { "Authorization": `Bearer ${token}`, "Content-Type": "application/json" },
    body: JSON.stringify(payload)
  }).catch(() => {});
  return { ok: true };
}

async function refreshHelloIfNeeded() {
  if (Date.now() - lastHelloAt < HELLO_REFRESH_MS) return;
  await hello();
}

async function showResult(result) {
  const title = result.ok ? "Saved" : "Needs setup";
  const badge = result.ok ? "✓" : "!";
  await ext.action.setBadgeText?.({ text: badge });
  await ext.action.setBadgeBackgroundColor?.({ color: result.ok ? "#287a4b" : "#b45309" });
  setTimeout(() => ext.action.setBadgeText?.({ text: "" }), BADGE_CLEAR_MS);
  if (!result.ok) {
    console.warn(`Starlee capture ${result.code}: ${result.error}`);
  }
  return { title, ...result };
}

function errorResult(code, error) {
  return { ok: false, code, error };
}

function safePageMetadata(payload) {
  const url = payload?.url || "";
  return {
    title: payload?.dom_extract?.title || payload?.youtube?.title || payload?.title || "",
    url,
    domain: domainFromUrl(url)
  };
}

function domainFromUrl(value) {
  try {
    return value ? new URL(value).hostname.replace(/^www\./, "") : "";
  } catch {
    return "";
  }
}

function browserName() {
  return browserNameFromUserAgent(navigator.userAgent);
}

function requestTargetsThisBrowser(request = {}) {
  return requestTargetsBrowser(request, browserName());
}
