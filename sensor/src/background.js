const DEFAULT_PORT = 47291;
const DEFAULT_POLL_MINUTES = 1;
const FALLBACK_POLL_SECONDS = 3;
let bundledConfigPromise;
let polling = false;
const processedRequests = new Set();

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type === "STARLEE_CAPTURE") {
    sendCapture(message.payload, { source: message.source || "content-script" }).then(sendResponse);
    return true;
  }
  if (message?.type === "STARLEE_STATUS") {
    status().then(sendResponse);
    return true;
  }
  if (message?.type === "STARLEE_HELLO") {
    hello({ force: true }).then(sendResponse);
    return true;
  }
  if (message?.type === "STARLEE_TAKE_CAPTURE_REQUEST") {
    takeCaptureRequest().then(sendResponse);
    return true;
  }
});

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
    return result;
  }
  try {
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
      return result;
    }
    const result = { ok: true, code: "capture_saved", record: body };
    await recordCaptureResult(result, options.source);
    return result;
  } catch {
    const result = errorResult("service_down", "Local Starlee is not reachable. Open Starlee or run starlee serve.");
    await recordCaptureResult(result, options.source);
    return result;
  }
}

async function captureTab(tab) {
  if (!tab?.id) return errorResult("no_active_tab", "No active browser tab is available.");
  try {
    return await chrome.tabs.sendMessage(tab.id, { type: "STARLEE_CAPTURE_NOW" });
  } catch {
    return errorResult("permission_denied", "Chrome has not granted Starlee access to this page, or this page cannot run extensions.");
  }
}

async function startLocalBridge() {
  if (polling) return;
  polling = true;
  await hello();
  await pollCaptureRequest();
  chrome.alarms?.create?.("starlee-poll", { periodInMinutes: DEFAULT_POLL_MINUTES });
  chrome.alarms?.onAlarm?.addListener((alarm) => {
    if (alarm.name === "starlee-poll") pollCaptureRequest();
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
  const result = await captureTab(tab);
  await chrome.storage.local.set({
    lastMenuRequestId: request.id || "",
    lastMenuRequestAt: new Date().toISOString(),
    lastMenuRequestStatus: result.ok ? "capture_saved" : result.code || "capture_failed"
  });
  await showResult(result);
}

async function takeCaptureRequest() {
  const { token, port } = await localSettings();
  if (!token) return { ok: false, request: null, code: "token_missing" };
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
  await chrome.storage.local.set({
    lastMenuRequestId: request.id || "",
    lastMenuRequestAt: new Date().toISOString(),
    lastMenuRequestStatus: "picked_up"
  });
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
    lastCaptureStatus: result.code || (result.ok ? "capture_saved" : "capture_failed"),
    lastCaptureError: result.ok ? "" : result.error
  });
}

async function showResult(result) {
  const title = result.ok ? "Saved" : "Needs setup";
  const badge = result.ok ? "✓" : "!";
  await chrome.action.setBadgeText?.({ text: badge });
  await chrome.action.setBadgeBackgroundColor?.({ color: result.ok ? "#287a4b" : "#b45309" });
  setTimeout(() => chrome.action.setBadgeText?.({ text: "" }), 3500);
  if (!result.ok) {
    console.warn(`Starlee capture ${result.code}: ${result.error}`);
  }
  return { title, ...result };
}

function errorResult(code, error) {
  return { ok: false, code, error };
}

function browserName() {
  const agent = navigator.userAgent;
  if (agent.includes("Edg/")) return "Edge";
  if (agent.includes("OPR/")) return "Opera";
  if (agent.includes("Brave/")) return "Brave";
  return "Chrome";
}
