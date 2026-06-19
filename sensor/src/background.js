const DEFAULT_PORT = 47291;
let bundledConfigPromise;
let polling = false;

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type !== "STARLEE_CAPTURE") return;
  sendCapture(message.payload).then(sendResponse);
  return true;
});

chrome.action.onClicked.addListener(async (tab) => {
  const result = await captureTab(tab);
  if (!result.ok) console.warn("Starlee capture failed:", result.error);
});

startLocalBridge();

async function sendCapture(payload) {
  const { token, port } = await localSettings();
  if (!token) return { ok: false, error: "Run starlee setup, then reload the unpacked extension" };
  try {
    const response = await fetch(`http://127.0.0.1:${port}/capture`, {
      method: "POST",
      headers: { "Authorization": `Bearer ${token}`, "Content-Type": "application/json" },
      body: JSON.stringify(payload)
    });
    const result = await response.json();
    if (!response.ok) return { ok: false, error: result.error || `HTTP ${response.status}` };
    return { ok: true, record: result };
  } catch {
    return { ok: false, error: "Local Starlee engine is not reachable" };
  }
}

async function captureTab(tab) {
  if (!tab?.id) return { ok: false, error: "No active browser tab" };
  try {
    return await chrome.tabs.sendMessage(tab.id, { type: "STARLEE_CAPTURE_NOW" });
  } catch {
    return { ok: false, error: "This page cannot be captured by Starlee" };
  }
}

async function startLocalBridge() {
  if (polling) return;
  polling = true;
  await hello();
  chrome.alarms?.create?.("starlee-poll", { periodInMinutes: 0.05 });
  chrome.alarms?.onAlarm?.addListener((alarm) => {
    if (alarm.name === "starlee-poll") pollCaptureRequest();
  });
  setInterval(pollCaptureRequest, 3000);
}

async function hello() {
  const { token, port } = await localSettings();
  if (!token) return;
  try {
    await fetch(`http://127.0.0.1:${port}/extension/hello`, {
      method: "POST",
      headers: { "Authorization": `Bearer ${token}`, "Content-Type": "application/json" },
      body: JSON.stringify({
        browser: navigator.userAgent.includes("Edg/") ? "Edge" : "Chromium",
        extension_version: chrome.runtime.getManifest().version,
        can_capture_active_tab: true
      })
    });
  } catch {
    // The local app/service may not be running yet.
  }
}

async function pollCaptureRequest() {
  const { token, port } = await localSettings();
  if (!token) return;
  let request;
  try {
    const response = await fetch(`http://127.0.0.1:${port}/capture-request`, {
      headers: { "Authorization": `Bearer ${token}` }
    });
    if (!response.ok) return;
    request = (await response.json()).request;
  } catch {
    return;
  }
  if (!request) return;
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  const result = await captureTab(tab);
  if (!result.ok) console.warn("Starlee menu-bar capture failed:", result.error);
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
