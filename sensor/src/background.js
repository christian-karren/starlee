const DEFAULT_PORT = 47291;
const DEFAULT_POLL_MINUTES = 1;
const FALLBACK_POLL_SECONDS = 3;
const BADGE_CLEAR_MS = 3500;
const ALARM_NAME = "starlee-poll";
const HELLO_REFRESH_MS = 5000;
const MESSAGE = Object.freeze({
  capture: "STARLEE_CAPTURE",
  status: "STARLEE_STATUS",
  hello: "STARLEE_HELLO",
  takeCaptureRequest: "STARLEE_TAKE_CAPTURE_REQUEST",
  captureNow: "STARLEE_CAPTURE_NOW",
  bridgeHealth: "STARLEE_BRIDGE_HEALTH"
});
const CAPTURE_STATUS = Object.freeze({
  saved: "capture_saved",
  failed: "capture_failed",
  pickedUp: "picked_up",
  extracting: "extracting",
  posted: "posted",
  permissionDenied: "permission_denied",
  unsupportedPage: "unsupported_page"
});
let bundledConfigPromise;
let polling = false;
let lastHelloAt = 0;
const processedRequests = new Set();

chrome.runtime.onMessage.addListener(handleMessage);

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
  if (message?.type === MESSAGE.takeCaptureRequest) {
    takeCaptureRequest().then(sendResponse);
    return true;
  }
  if (message?.type === MESSAGE.bridgeHealth) {
    bridgeHealth().then(sendResponse);
    return true;
  }
}

chrome.action.onClicked.addListener(async (tab) => {
  const result = await captureTab(tab);
  await showResult(result);
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
      await recordCaptureResult(result, options.source);
      await recordCaptureRequestResult(options.requestId, result);
      return result;
    }
    const result = { ok: true, code: "capture_saved", record: body };
    await recordCaptureResult(result, options.source);
    await recordCaptureRequestResult(options.requestId, result);
    return result;
  } catch {
    const result = errorResult("service_down", "Local Starlee is not reachable. Open Starlee or run starlee serve.");
    await recordCaptureResult(result, options.source);
    await recordCaptureRequestResult(options.requestId, result);
    return result;
  }
}

async function captureTab(tab) {
  if (!tab?.id) return errorResult("no_active_tab", "No active browser tab is available.");
  try {
    return await chrome.tabs.sendMessage(tab.id, { type: MESSAGE.captureNow });
  } catch {
    return errorResult("permission_denied", "Chrome has not granted Starlee access to this page, or this page cannot run extensions.");
  }
}

async function startLocalBridge() {
  if (polling) return;
  polling = true;
  await hello();
  await pollCaptureRequest();
  chrome.alarms?.create?.(ALARM_NAME, { periodInMinutes: DEFAULT_POLL_MINUTES });
  chrome.alarms?.onAlarm?.addListener((alarm) => {
    if (alarm.name === ALARM_NAME) pollCaptureRequest();
  });
  setInterval(pollCaptureRequest, FALLBACK_POLL_SECONDS * 1000);
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
        extension_version: chrome.runtime.getManifest().version,
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
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  const result = await captureTabForRequest(tab, request);
  await chrome.storage.local.set({
    lastMenuRequestId: request.id || "",
    lastMenuRequestAt: new Date().toISOString(),
    lastMenuRequestStatus: result.ok ? CAPTURE_STATUS.saved : result.code || CAPTURE_STATUS.failed
  });
  await showResult(result);
}

async function captureTabForRequest(tab, request) {
  if (!tab?.id) {
    const result = errorResult(CAPTURE_STATUS.failed, "No active browser tab is available.");
    await recordCaptureRequestResult(request.id, result);
    return result;
  }
  await recordCaptureRequestStatus(request.id, CAPTURE_STATUS.extracting, "Browser extension is extracting the active tab.");
  try {
    return await chrome.tabs.sendMessage(tab.id, {
      type: MESSAGE.captureNow,
      source: "menu-bar",
      requestId: request.id
    });
  } catch {
    const result = errorResult(CAPTURE_STATUS.permissionDenied, "Chrome has not granted Starlee access to this page, or this page cannot run extensions.");
    await recordCaptureRequestResult(request.id, result);
    return result;
  }
}

async function takeCaptureRequest() {
  const { token, port } = await localSettings();
  if (!token) return { ok: false, request: null, code: "token_missing" };
  await refreshHelloIfNeeded();
  let request;
  try {
    const response = await fetch(`http://127.0.0.1:${port}/capture-request`, {
      headers: { "Authorization": `Bearer ${token}` }
    });
    if (!response.ok) return { ok: false, request: null, code: `http_${response.status}` };
    request = (await response.json()).request;
  } catch {
    return { ok: false, request: null, code: "service_down" };
  }
  if (!request) return { ok: true, request: null };
  if (request.id && processedRequests.has(request.id)) return { ok: true, request: null };
  if (request.id) processedRequests.add(request.id);
  await recordMenuRequest(request, CAPTURE_STATUS.pickedUp);
  return { ok: true, request };
}

async function localSettings() {
  const { captureToken = "", capturePort = DEFAULT_PORT } = await chrome.storage.local.get(["captureToken", "capturePort"]);
  const bundled = await bundledConfig();
  return {
    token: captureToken || bundled.captureToken || "",
    port: capturePort || bundled.capturePort || DEFAULT_PORT
  };
}

async function bundledConfig() {
  bundledConfigPromise ||= fetch(chrome.runtime.getURL("starlee-config.json"))
    .then((response) => response.ok ? response.json() : {})
    .catch(() => ({}));
  return bundledConfigPromise;
}

async function status() {
  const settings = await localSettings();
  const diagnostic = await chrome.storage.local.get([
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
    extensionVersion: chrome.runtime.getManifest().version,
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
  await chrome.storage.local.set({
    lastHandshakeAt: result.ok ? new Date().toISOString() : "",
    lastHandshakeStatus: result.code,
    lastHandshakeError: result.ok ? "" : result.error
  });
}

async function recordCaptureResult(result, source = "unknown") {
  await chrome.storage.local.set({
    lastCaptureAt: new Date().toISOString(),
    lastCaptureSource: source,
    lastCaptureStatus: result.code || (result.ok ? CAPTURE_STATUS.saved : CAPTURE_STATUS.failed),
    lastCaptureError: result.ok ? "" : result.error
  });
}

async function recordMenuRequest(request, status) {
  await chrome.storage.local.set({
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
      ...(page ? { page } : {})
    })
  }).catch(() => {});
}

async function refreshHelloIfNeeded() {
  if (Date.now() - lastHelloAt < HELLO_REFRESH_MS) return;
  await hello();
}

async function showResult(result) {
  const title = result.ok ? "Saved" : "Needs setup";
  const badge = result.ok ? "✓" : "!";
  await chrome.action.setBadgeText?.({ text: badge });
  await chrome.action.setBadgeBackgroundColor?.({ color: result.ok ? "#287a4b" : "#b45309" });
  setTimeout(() => chrome.action.setBadgeText?.({ text: "" }), BADGE_CLEAR_MS);
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
  const agent = navigator.userAgent;
  if (agent.includes("Edg/")) return "Edge";
  if (agent.includes("OPR/")) return "Opera";
  if (agent.includes("Brave/")) return "Brave";
  return "Chrome";
}
