import { capturePayload, detectedType } from "./payload.js";

const BUTTON_RESET_MS = 3500;
const MESSAGE = Object.freeze({
  ping: "STARLEE_CONTENT_SCRIPT_PING",
  captureNow: "STARLEE_CAPTURE_NOW",
  capture: "STARLEE_CAPTURE",
  diagnostic: "STARLEE_CAPTURE_DIAGNOSTIC"
});
const type = detectedType(document);
if (type && !document.getElementById("starlee-save-button")) mountButton(type);

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type === MESSAGE.ping) {
    sendResponse(contentScriptReadiness(message));
    return false;
  }
  if (message?.type !== MESSAGE.captureNow) return;
  capture(message, sendResponse);
  return true;
});

function contentScriptReadiness(message = {}) {
  const pageType = detectedType(document) || "unsupported";
  return {
    ok: true,
    code: "content_script_ready",
    ready: true,
    requestId: message.requestId,
    page_type: pageType,
    page: safePageDomainFromDocument(document)
  };
}

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
  const requestId = _message?.requestId;
  const source = _message?.source || "active-tab";
  const pageType = detectedType(document) || "unsupported";
  await sendDiagnostic(requestId, {
    component: "content_script",
    event: "content_script_ready",
    status: pageType === "unsupported" ? "unsupported" : "ready",
    source,
    page: safePageDomainFromDocument(document),
    safe_metadata: {
      page_type: pageType
    }
  });
  await sendDiagnostic(requestId, {
    component: "content_script",
    event: "content_script_capture_started",
    status: "started",
    source,
    page: safePageFromDocument(document)
  });
  try {
    const diagnosticEvents = [];
    if (detectedType(document) === "youtube") {
      await sendDiagnostic(requestId, {
        component: "content_script",
        event: "content_script_youtube_detected",
        status: "ok",
        source,
        page: safePageFromDocument(document)
      });
    }
    const payload = await capturePayload(document, {
      discoverYouTubeTranscript: true,
      onDiagnostic: (event) => {
        diagnosticEvents.push({
          ...event,
          source,
          page: safePageFromDocument(document)
        });
      }
    });
    for (const event of diagnosticEvents) {
      await sendDiagnostic(requestId, event);
    }
    const selectedText = String(window.getSelection?.()?.toString() || "").trim();
    if (selectedText && payload?.dom_extract && payload.type === "article") {
      payload.dom_extract.selected_text = selectedText;
    }
    const response = await chrome.runtime.sendMessage({
      type: MESSAGE.capture,
      payload,
      source,
      requestId
    });
    sendResponse(response);
  } catch (error) {
    const message = error.message || "This page cannot be captured by Starlee.";
    const code = message.includes("does not look like")
      ? "unsupported_page"
      : "capture_failed";
    await sendDiagnostic(requestId, {
      component: "content_script",
      event: "content_script_capture_failed",
      status: code,
      source,
      message,
      page: safePageFromDocument(document)
    });
    sendResponse({ ok: false, code, error: message });
  }
}

async function sendDiagnostic(requestId, event) {
  if (!requestId) return;
  await chrome.runtime
    .sendMessage({
      type: MESSAGE.diagnostic,
      event: {
        ...event,
        request_id: requestId
      }
    })
    .catch(() => {});
}

function safePageFromDocument(document) {
  const url = document.location?.href || "";
  return {
    title: document.title || "",
    url,
    domain: domainFromUrl(url)
  };
}

function safePageDomainFromDocument(document) {
  return {
    domain: domainFromUrl(document.location?.href || "")
  };
}

function domainFromUrl(value) {
  try {
    return value ? new URL(value).hostname.replace(/^www\./, "") : "";
  } catch {
    return "";
  }
}
