// Pure HTTP functions for the background polling loop.
// Each function accepts explicit {token, port} so they are testable without chrome.storage.
export async function pollCaptureRequest({ token, port }) {
  const response = await fetch(`http://127.0.0.1:${port}/capture-request`, {
    headers: { Authorization: `Bearer ${token}` }
  });
  if (!response.ok) return { ok: false, request: null, code: `http_${response.status}` };
  const { request } = await response.json();
  return { ok: true, request: request || null };
}

export async function postRequestResult({ token, port, id, status, message, page }) {
  await fetch(`http://127.0.0.1:${port}/capture-request/result`, {
    method: "POST",
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: JSON.stringify({ id, status, message, ...(page ? { page } : {}) })
  }).catch(() => {});
}

export async function postCapture({ token, port, payload, requestId }) {
  const response = await fetch(`http://127.0.0.1:${port}/capture`, {
    method: "POST",
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: JSON.stringify(payload)
  });
  const body = await response.json().catch(() => ({}));
  if (!response.ok) {
    const code =
      response.status === 401 ? "auth_error" :
      response.status === 413 ? "payload_too_large" :
      "capture_failed";
    return { ok: false, code, error: body.error || `HTTP ${response.status}`, requestId };
  }
  return { ok: true, code: "capture_saved", record: body, requestId };
}
