// Pure HTTP functions for the background polling loop.
// Each function accepts explicit {token, port} so they are testable without chrome.storage.
export async function pollCaptureRequest({ token, port }) {
  if (!token) return { ok: false, request: null, code: "token_missing" };
  try {
    const response = await fetch(`http://127.0.0.1:${port}/capture-request`, {
      headers: { Authorization: `Bearer ${token}` }
    });
    if (!response.ok) return { ok: false, request: null, code: response.status === 401 ? "token_invalid" : `http_${response.status}` };
    const { request } = await response.json();
    return { ok: true, request: request || null };
  } catch {
    return { ok: false, request: null, code: "service_down" };
  }
}

export async function postRequestResult({ token, port, id, status, message, page }) {
  if (!token || !id) return;
  await fetch(`http://127.0.0.1:${port}/capture-request/result`, {
    method: "POST",
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: JSON.stringify({ id, status, message, ...(page ? { page } : {}) })
  }).catch(() => {});
}

export async function postCapture({ token, port, payload, requestId }) {
  if (!token) return { ok: false, code: "token_missing", error: "Capture token is not configured.", requestId };
  try {
    const response = await fetch(`http://127.0.0.1:${port}/capture`, {
      method: "POST",
      headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
      body: JSON.stringify(payload)
    });
    const body = await response.json().catch(() => ({}));
    if (!response.ok) {
      const code =
        response.status === 401 ? "token_invalid" :
        response.status === 413 ? "payload_too_large" :
        "capture_failed";
      return { ok: false, code, error: body.error || `HTTP ${response.status}`, requestId };
    }
    return { ok: true, code: "capture_saved", record: body, requestId };
  } catch {
    return { ok: false, code: "service_down", error: "Local Starlee is not reachable.", requestId };
  }
}
