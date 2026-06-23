import { capturePayload, detectedType } from "./payload.js";

const MENU_BAR_POLL_MS = 350;
const MENU_BAR_INITIAL_POLL_MS = 150;
const BUTTON_RESET_MS = 3500;
const MESSAGE = Object.freeze({
  captureNow: "STARLEE_CAPTURE_NOW",
  capture: "STARLEE_CAPTURE",
  takeCaptureRequest: "STARLEE_TAKE_CAPTURE_REQUEST"
});
const type = detectedType(document);
if (type && !document.getElementById("starlee-save-button")) mountButton(type);
startMenuBarBridge();

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type !== MESSAGE.captureNow) return;
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
    setTimeout(() => { button.textContent = defaultLabel; }, BUTTON_RESET_MS);
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
      type: MESSAGE.capture,
      payload,
      source: _message?.source || "active-tab",
      requestId: _message?.requestId
    });
    sendResponse(response);
  } catch (error) {
    const message = error.message || "This page cannot be captured by Starlee.";
    const code = message.includes("does not look like")
      ? "unsupported_page"
      : "capture_failed";
    sendResponse({ ok: false, code, error: message });
  }
}

function startMenuBarBridge() {
  setTimeout(pollMenuBarCaptureRequest, MENU_BAR_INITIAL_POLL_MS);
  setInterval(pollMenuBarCaptureRequest, MENU_BAR_POLL_MS);
}

async function pollMenuBarCaptureRequest() {
  if (document.visibilityState !== "visible") return;
  const response = await takeMenuBarCaptureRequest();
  if (!response?.request) return;
  capture({ source: "menu-bar", requestId: response.request.id }, () => {});
}

async function takeMenuBarCaptureRequest() {
  return chrome.runtime
    .sendMessage({ type: MESSAGE.takeCaptureRequest })
    .catch(() => null);
}
