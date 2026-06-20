const form = document.querySelector("form");
const token = document.querySelector("#token");
const port = document.querySelector("#port");
const status = document.querySelector("#status");
const connection = document.querySelector("#connection");
const testButton = document.querySelector("#test-connection");

const saved = await chrome.storage.local.get(["captureToken", "capturePort"]);
const bundled = await fetch(chrome.runtime.getURL("starlee-config.json"))
  .then((response) => response.ok ? response.json() : {})
  .catch(() => ({}));
const hasToken = Boolean(saved.captureToken || bundled.captureToken);
token.value = "";
token.placeholder = hasToken ? "Token configured — leave blank to keep it" : "Paste local capture token";
port.value = saved.capturePort || bundled.capturePort || 47291;
await renderStatus();

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const next = { capturePort: Number(port.value) };
  const nextToken = token.value.trim();
  if (nextToken) next.captureToken = nextToken;
  await chrome.storage.local.set(next);
  token.value = "";
  token.placeholder = "Token configured — leave blank to keep it";
  status.textContent = "Saved locally.";
  await renderStatus({ forceHello: true });
});

testButton.addEventListener("click", async () => {
  status.textContent = "Checking local Starlee…";
  await renderStatus({ forceHello: true });
  status.textContent = "";
});

async function renderStatus({ forceHello = false } = {}) {
  if (forceHello) await chrome.runtime.sendMessage({ type: "STARLEE_HELLO" });
  const state = await chrome.runtime.sendMessage({ type: "STARLEE_STATUS" });
  const label = state.ok ? "Connected to local Starlee" : statusLabel(state);
  connection.textContent = [
    label,
    `Browser: ${state.browser || "Chrome"}`,
    `Extension: ${state.extensionVersion || "unknown"}`,
    `Port: ${state.port || 47291}`,
    state.lastHandshakeAt ? `Last handshake: ${state.lastHandshakeAt}` : "Last handshake: not yet connected",
    state.lastCaptureStatus ? `Last capture: ${state.lastCaptureStatus}` : "",
    state.lastMenuRequestStatus ? `Last menu-bar request: ${state.lastMenuRequestStatus}` : ""
  ].filter(Boolean).join("\n");
  connection.dataset.state = state.ok ? "ok" : "warn";
}

function statusLabel(state) {
  if (!state.hasToken) return "Capture token is not configured.";
  if (state.lastHandshakeStatus === "token_invalid") return "Capture token was rejected by local Starlee.";
  if (state.lastHandshakeStatus === "service_down") return "Local Starlee is not reachable. Open Starlee or run starlee serve.";
  if (state.lastHandshakeError) return state.lastHandshakeError;
  return "Not connected yet. Click Test connection.";
}
