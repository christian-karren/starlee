import { capturePayload, detectedType } from "./payload.js";

const MENU_BAR_POLL_MS = 3000;
const type = detectedType(document);
if (type && !document.getElementById("starlee-save-button")) mountButton(type);
startMenuBarBridge();

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type !== "STARLEE_CAPTURE_NOW") return;
  capture(message, sendResponse);
  return true;
});

function mountButton(type) {
  const button = document.createElement("button");
  button.id = "starlee-save-button";
  const defaultLabel = type === "youtube" ? "Save video to Starlee" : "Save article to Starlee";
  button.textContent = defaultLabel;
  Object.assign(button.style, {
    position: "fixed", zIndex: "2147483647", right: "18px", bottom: "18px",
    border: "0", borderRadius: "999px", padding: "11px 16px", cursor: "pointer",
    background: "#17152b", color: "#fff", font: "600 13px system-ui", boxShadow: "0 6px 24px #0004"
  });
  button.addEventListener("click", () => capture({}, (result) => {
    button.textContent = result.ok ? "Saved to Starlee ✓" : `Starlee: ${result.error}`;
    setTimeout(() => { button.textContent = defaultLabel; }, 3500);
  }));
  document.documentElement.append(button);
}

async function capture(_message, sendResponse) {
  try {
    const payload = capturePayload(document);
    const selectedText = String(window.getSelection?.()?.toString() || "").trim();
    if (selectedText && payload?.dom_extract && payload.type === "article") {
      payload.dom_extract.selected_text = selectedText;
    }
    const response = await chrome.runtime.sendMessage({
      type: "STARLEE_CAPTURE",
      payload,
      source: _message?.source || "active-tab"
    });
    sendResponse(response);
  } catch (error) {
    sendResponse({ ok: false, code: "empty_extract", error: error.message });
  }
}

function startMenuBarBridge() {
  setTimeout(pollMenuBarCaptureRequest, 750);
  setInterval(pollMenuBarCaptureRequest, MENU_BAR_POLL_MS);
}

async function pollMenuBarCaptureRequest() {
  if (document.visibilityState !== "visible") return;
  const response = await chrome.runtime
    .sendMessage({ type: "STARLEE_TAKE_CAPTURE_REQUEST" })
    .catch(() => null);
  if (!response?.request) return;
  capture({ source: "menu-bar" }, () => {});
}
