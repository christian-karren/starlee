const DEFAULT_PORT = 47291;
let bundledConfigPromise;

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type !== "STARLEE_CAPTURE") return;
  sendCapture(message.payload).then(sendResponse);
  return true;
});

chrome.action.onClicked.addListener(async (tab) => {
  if (!tab.id) return;
  try { await chrome.tabs.sendMessage(tab.id, { type: "STARLEE_CAPTURE_NOW" }); }
  catch { /* unsupported browser pages cannot be captured */ }
});

async function sendCapture(payload) {
  const { captureToken = "", capturePort = DEFAULT_PORT } = await chrome.storage.local.get(["captureToken", "capturePort"]);
  const bundled = await bundledConfig();
  const token = captureToken || bundled.captureToken || "";
  const port = capturePort || bundled.capturePort || DEFAULT_PORT;
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

async function bundledConfig() {
  bundledConfigPromise ||= fetch(chrome.runtime.getURL("starlee-config.json"))
    .then((response) => response.ok ? response.json() : {})
    .catch(() => ({}));
  return bundledConfigPromise;
}
