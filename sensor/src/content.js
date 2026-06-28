import { capturePayload, detectedType } from "./payload.js";
import { createExtensionApi } from "./browser.js";

const chrome = createExtensionApi();
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
  const selectedText = String(window.getSelection?.()?.toString() || "").trim();
  await sendDiagnostic(requestId, {
    component: "content_script",
    event: "content_script_ready",
    status: pageType === "unsupported" ? "unsupported" : "ready",
    source,
    page: safePageDomainFromDocument(document),
    safe_metadata: {
      page_type: pageType,
      selection_present: String(Boolean(selectedText)),
      selection_char_count: String(selectedText.length)
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
    if (selectedText && payload?.dom_extract && payload.type === "article") {
      payload.dom_extract.selected_text = selectedText;
      await sendDiagnostic(requestId, {
        component: "content_script",
        event: "content_script_selected_text_attached",
        status: "ok",
        source,
        page: safePageDomainFromDocument(document),
        safe_metadata: {
          selection_present: "true",
          selection_char_count: String(selectedText.length)
        }
      });
    }
    await sendDiagnostic(requestId, {
      component: "content_script",
      event: "content_script_payload_ready",
      status: "ok",
      source,
      page: safePageFromDocument(document),
      safe_metadata: payloadMetadata(payload)
    });
    const response = await chrome.runtime.sendMessage({
      type: MESSAGE.capture,
      payload,
      source,
      requestId
    });
    sendResponse(response);
  } catch (error) {
    const message = error.message || "This page cannot be captured by Starlee.";
    const code = captureFailureCode(message, pageType);
    await sendDiagnostic(requestId, {
      component: "content_script",
      event: "content_script_capture_failed",
      status: code,
      source,
      message: failureDiagnosticMessage(code),
      page: safePageFromDocument(document),
      safe_metadata: {
        page_type: pageType,
        error_kind: captureFailureKind(message, pageType),
        error_message: redactedErrorMessage(message)
      }
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

function payloadMetadata(payload = {}) {
  return {
    payload_type: payload.type || "unknown",
    access: payload.access || "unknown",
    text_char_count: String(payload.dom_extract?.text?.length || 0),
    selection_present: String(Boolean(payload.dom_extract?.selected_text)),
    transcript_segment_count: String(payload.transcript?.length || 0),
    transcript_status: payload.transcript_status || "",
    transcript_source: payload.transcript_source || "",
    transcript_reason: payload.transcript_reason || ""
  };
}

function captureFailureCode(message = "", pageType = "unsupported") {
  if (message.includes("does not look like") || pageType === "unsupported") return "unsupported_page";
  if (message.includes("readable article text")) return "empty_extract";
  return "capture_failed";
}

function captureFailureKind(message = "", pageType = "unsupported") {
  if (message.includes("does not look like") || pageType === "unsupported") return "unsupported_page";
  if (message.includes("readable article text")) return "empty_article";
  return "extractor_failure";
}

function failureDiagnosticMessage(code) {
  if (code === "unsupported_page") return "The active page is not a supported article or YouTube watch page.";
  if (code === "empty_extract") return "Starlee reached the page but could not extract article text.";
  return "Starlee could not build a capture payload from the active page.";
}

function redactedErrorMessage(message = "") {
  return String(message)
    .replace(/https?:\/\/\S+/g, "[url]")
    .replace(/[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g, "[email]")
    .slice(0, 160);
}
