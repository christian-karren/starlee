const DEFAULT_PORT = 47291;

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
  if (!captureToken) return { ok: false, error: "Add your local token in extension options" };
  try {
    const response = await fetch(`http://127.0.0.1:${capturePort}/capture`, {
      method: "POST",
      headers: { "Authorization": `Bearer ${captureToken}`, "Content-Type": "application/json" },
      body: JSON.stringify(payload)
    });
    const result = await response.json();
    if (!response.ok) return { ok: false, error: result.error || `HTTP ${response.status}` };
    return { ok: true, record: result };
  } catch {
    return { ok: false, error: "Local Starlee engine is not reachable" };
  }
}

