import { createExtensionApi } from "./browser.js";
import { CAPTURE_STATUS } from "./capture-status.js";

const ext = createExtensionApi();
const form = document.querySelector("form");
const token = document.querySelector("#token");
const port = document.querySelector("#port");
const status = document.querySelector("#status");
const connection = document.querySelector("#connection");
const testButton = document.querySelector("#test-connection");

const saved = await ext.storage.local.get(["captureToken", "capturePort"]);
const bundled = await fetch(ext.runtime.getURL("starlee-config.json"))
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
  await ext.storage.local.set(next);
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
  if (forceHello) await ext.runtime.sendMessage({ type: "STARLEE_HELLO" });
  const state = await ext.runtime.sendMessage({ type: "STARLEE_STATUS" });
  const bridge = await ext.runtime.sendMessage({ type: "STARLEE_BRIDGE_HEALTH" }).catch(() => null);
  const setup = bridge?.browser_setup || bridge?.chrome_setup;
  const label = state.ok ? setupLabel(setup) || "Connected to local Starlee" : statusLabel(state);
  connection.textContent = [
    label,
    setup?.detail ? `Setup detail: ${setup.detail}` : "",
    setup?.next_action ? `Next action: ${setup.next_action}` : bridge?.recommended_next_action ? `Next action: ${bridge.recommended_next_action}` : "",
    `Browser: ${state.browser || "Browser"}`,
    `Extension: ${state.extensionVersion || "unknown"}`,
    `Build: ${state.extensionBuild || bridge?.extension_build || "unknown"}`,
    `Port: ${state.port || 47291}`,
    setup ? `Installed: ${setup.installed ? "yes" : "no"}` : "",
    setup ? `Checked in recently: ${setup.checked_in_recently ? "yes" : "no"}` : "",
    setup ? `Permission needed: ${setup.permission_needed ? "yes" : "no"}` : "",
    setup ? `Capture test passed: ${setup.capture_test_passed ? "yes" : "no"}` : "",
    setup?.capture_test_passed_at ? `Capture test: ${setup.capture_test_passed_at}` : "",
    state.lastHandshakeAt ? `Last handshake: ${state.lastHandshakeAt}` : "Last handshake: not yet connected",
    bridge?.checked_in_recently === false ? "Bridge heartbeat: stale or missing" : "",
    bridge?.can_capture_active_tab === false ? "Active tab capture: not available" : "",
    state.lastCaptureStatus ? `Last capture: ${state.lastCaptureStatus}` : "",
    state.lastMenuRequestStatus ? `Last menu-bar request: ${state.lastMenuRequestStatus}` : "",
    bridge?.last_failure_reason ? `Last bridge failure: ${bridge.last_failure_reason}` : "",
    bridge?.last_failure_message ? bridge.last_failure_message : ""
  ].filter(Boolean).join("\n");
  connection.dataset.state = state.ok && bridge?.ok !== false ? "ok" : "warn";
}

function setupLabel(setup) {
  if (!setup?.state) return "";
  switch (setup.state) {
    case "capture_test_passed":
      return "Browser setup is ready.";
    case "capture_test_needed":
      return "Browser extension is connected. Run a capture test from Starlee desktop setup.";
    case "permission_needed":
      return "Browser extension needs Starlee site access.";
    case "check_in_needed":
      return "Browser extension has not checked in recently.";
    case "install_needed":
      return "Browser extension setup is not complete.";
    default:
      return `Browser setup: ${setup.state}`;
  }
}

function statusLabel(state) {
  if (!state.hasToken) return "Capture token is not configured.";
  if (state.lastHandshakeStatus === CAPTURE_STATUS.tokenInvalid) return "Capture token was rejected by local Starlee.";
  if (state.lastHandshakeStatus === CAPTURE_STATUS.serviceDown) return "Local Starlee is not reachable. Open Starlee or run starlee serve.";
  if (state.lastHandshakeError) return state.lastHandshakeError;
  return "Not connected yet. Click Test connection.";
}
