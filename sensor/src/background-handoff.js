export const CONTENT_SCRIPT_UNREACHABLE = "content_script_unreachable";

const LOCALHOST_HOSTS = new Set(["127.0.0.1", "localhost"]);

export function safeTabPage(tab = {}) {
  return {
    domain: domainFromUrl(tab.url || "")
  };
}

export function safeTabMetadata(tab = {}) {
  const parsed = parseUrl(tab.url || "");
  return {
    tab_id_present: String(Boolean(tab.id)),
    url_present: String(Boolean(tab.url)),
    url_scheme: parsed?.protocol?.replace(":", "") || "",
    domain: parsed?.hostname?.replace(/^www\./, "") || ""
  };
}

export function supportedContentScriptUrl(value = "") {
  const parsed = parseUrl(value);
  if (!parsed) return false;
  return parsed.protocol === "http:" || parsed.protocol === "https:";
}

export function activeTabProblem(tab = {}) {
  if (!tab?.id) {
    return {
      event: "active_tab_lookup_failed",
      status: "no_active_tab",
      message: "No active browser tab is available."
    };
  }
  if (!tab.url) {
    return {
      event: "active_tab_missing_url",
      status: "permission_denied",
      message: "Safari did not expose the active tab URL to Starlee."
    };
  }
  if (!supportedContentScriptUrl(tab.url)) {
    return {
      event: "active_tab_unsupported_url",
      status: "unsupported_page",
      message: "The active page cannot run the Starlee content script."
    };
  }
  return null;
}

export function activeTabLookupFailure(error, browserName = "Safari") {
  const message = errorMessage(error);
  const permissionDenied = /permission|access|not allowed|denied/i.test(message);
  return {
    event: permissionDenied ? "active_tab_permission_denied" : "active_tab_lookup_failed",
    status: permissionDenied ? "permission_denied" : "capture_failed",
    message: permissionDenied
      ? `${browserName} did not grant Starlee access to inspect the active tab.`
      : "Starlee could not inspect the active browser tab.",
    safe_metadata: {
      error_kind: permissionDenied ? "permission_denied" : "lookup_failed",
      error_message: redactedErrorMessage(message)
    }
  };
}

export function contentScriptFailureResult(browserName = "Safari") {
  return {
    ok: false,
    code: CONTENT_SCRIPT_UNREACHABLE,
    error: `${browserName} extension could not reach the page content script. Open Safari, enable the Starlee Safari extension, allow it on youtube.com, reload the YouTube tab, then try capture again.`
  };
}

export function classifyContentScriptMessageError(error) {
  const message = errorMessage(error);
  const noReceiver = /receiving end|could not establish connection|no receiver|message port closed|target closed/i.test(message);
  const timeout = /timeout|timed out/i.test(message);
  const permissionDenied = /permission|access|not allowed|denied/i.test(message);
  if (timeout) {
    return {
      event: "content_script_timeout",
      status: CONTENT_SCRIPT_UNREACHABLE,
      error_kind: "timeout",
      message: "Safari extension timed out waiting for the page content script."
    };
  }
  if (noReceiver) {
    return {
      event: "content_script_no_receiver",
      status: CONTENT_SCRIPT_UNREACHABLE,
      error_kind: "no_receiver",
      message: "Safari extension could not reach the page content script."
    };
  }
  if (permissionDenied) {
    return {
      event: "content_script_message_send_failed",
      status: "permission_denied",
      error_kind: "permission_denied",
      message: "Safari did not grant Starlee access to message the page content script."
    };
  }
  return {
    event: "content_script_message_send_failed",
    status: CONTENT_SCRIPT_UNREACHABLE,
    error_kind: "send_failed",
    message: "Safari extension could not message the page content script."
  };
}

export async function sendCaptureMessageToContentScript({
  tab,
  request,
  messageType,
  sendMessage,
  recordDiagnostic,
  browserName = "Safari"
}) {
  const source = request?.source || "menu-bar";
  const baseEvent = {
    component: "extension",
    request_id: request.id,
    source,
    browser: browserName,
    page: safeTabPage(tab)
  };
  const safe_metadata = safeTabMetadata(tab);
  await recordDiagnostic({
    ...baseEvent,
    event: "content_script_message_send_started",
    status: "started",
    message: "Browser extension is messaging the page content script.",
    safe_metadata
  });

  try {
    const result = await sendMessage(tab.id, {
      type: messageType,
      source: "menu-bar",
      requestId: request.id
    });
    if (!result) {
      const failure = contentScriptFailureResult(browserName);
      await recordDiagnostic({
        ...baseEvent,
        event: "content_script_no_receiver",
        status: failure.code,
        message: "Safari extension could not reach the page content script.",
        safe_metadata: {
          ...safe_metadata,
          error_kind: "empty_response"
        }
      });
      return failure;
    }
    await recordDiagnostic({
      ...baseEvent,
      event: "content_script_message_send_succeeded",
      status: result.ok ? "ok" : result.code || "capture_failed",
      message: "Page content script responded to the browser extension.",
      safe_metadata: {
        ...safe_metadata,
        response_ok: String(Boolean(result.ok)),
        response_code: result.code || ""
      }
    });
    if (!result.ok) {
      await recordDiagnostic({
        ...baseEvent,
        event: "content_script_returned_failure",
        status: result.code || "capture_failed",
        message: result.error || "Content script capture failed.",
        safe_metadata: {
          ...safe_metadata,
          response_code: result.code || "capture_failed"
        }
      });
    }
    return result;
  } catch (error) {
    const classified = classifyContentScriptMessageError(error);
    await recordDiagnostic({
      ...baseEvent,
      event: classified.event,
      status: classified.status,
      message: classified.message,
      safe_metadata: {
        ...safe_metadata,
        error_kind: classified.error_kind,
        error_message: redactedErrorMessage(errorMessage(error))
      }
    });
    return classified.status === "permission_denied"
      ? {
          ok: false,
          code: "permission_denied",
          error: `${browserName} has not granted Starlee access to this page, or this page cannot run extensions.`
        }
      : contentScriptFailureResult(browserName);
  }
}

export async function probeContentScriptReadiness({
  tab,
  request,
  messageType,
  sendMessage,
  recordDiagnostic,
  browserName = "Safari"
}) {
  const source = request?.source || "menu-bar";
  const baseEvent = {
    component: "extension",
    request_id: request.id,
    source,
    browser: browserName,
    page: safeTabPage(tab)
  };
  const safe_metadata = safeTabMetadata(tab);
  await recordDiagnostic({
    ...baseEvent,
    event: "content_script_probe_started",
    status: "started",
    message: "Browser extension is checking whether the page content script is reachable.",
    safe_metadata
  });

  try {
    const result = await sendMessage(tab.id, {
      type: messageType,
      source: "menu-bar",
      requestId: request.id
    });
    if (!result) {
      const failure = contentScriptFailureResult(browserName);
      await recordDiagnostic({
        ...baseEvent,
        event: "content_script_probe_no_receiver",
        status: failure.code,
        message: "Safari extension could not reach the page content script.",
        safe_metadata: {
          ...safe_metadata,
          error_kind: "empty_response"
        }
      });
      return failure;
    }
    await recordDiagnostic({
      ...baseEvent,
      event: "content_script_probe_succeeded",
      status: result.ready ? "ready" : "not_ready",
      message: "Page content script responded to the readiness probe.",
      safe_metadata: {
        ...safe_metadata,
        response_ok: String(Boolean(result.ok)),
        response_code: result.code || "",
        page_type: result.page_type || "unknown"
      }
    });
    if (!result.ok || !result.ready) {
      return {
        ok: false,
        code: "content_script_not_ready",
        error: "Safari reached the page content script, but it is not ready to capture."
      };
    }
    return result;
  } catch (error) {
    const classified = classifyContentScriptMessageError(error);
    const event = classified.event === "content_script_no_receiver"
      ? "content_script_probe_no_receiver"
      : classified.event === "content_script_timeout"
        ? "content_script_probe_timeout"
        : "content_script_probe_failed";
    await recordDiagnostic({
      ...baseEvent,
      event,
      status: classified.status,
      message: classified.message,
      safe_metadata: {
        ...safe_metadata,
        error_kind: classified.error_kind,
        error_message: redactedErrorMessage(errorMessage(error))
      }
    });
    return classified.status === "permission_denied"
      ? {
          ok: false,
          code: "permission_denied",
          error: `${browserName} has not granted Starlee access to this page, or this page cannot run extensions.`
        }
      : contentScriptFailureResult(browserName);
  }
}

function parseUrl(value = "") {
  try {
    const parsed = new URL(value);
    if (LOCALHOST_HOSTS.has(parsed.hostname)) return parsed;
    return parsed;
  } catch {
    return null;
  }
}

function domainFromUrl(value = "") {
  return parseUrl(value)?.hostname?.replace(/^www\./, "") || "";
}

function errorMessage(error) {
  return String(error?.message || error || "");
}

function redactedErrorMessage(message = "") {
  return String(message)
    .replace(/https?:\/\/\S+/g, "[url]")
    .replace(/[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g, "[email]")
    .slice(0, 160);
}
