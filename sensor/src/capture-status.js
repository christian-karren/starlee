export const CAPTURE_STATUS = Object.freeze({
  saved: "capture_saved",
  warning: "capture_warning",
  failed: "capture_failed",
  pickedUp: "picked_up",
  extracting: "extracting",
  posted: "posted",
  permissionDenied: "permission_denied",
  unsupportedPage: "unsupported_page",
  contentScriptUnreachable: "content_script_unreachable",
  tokenInvalid: "token_invalid",
  serviceDown: "service_down",
  payloadTooLarge: "payload_too_large",
  tokenMissing: "token_missing",
  noActiveTab: "no_active_tab",
  serviceError: "service_error",
});

export function errorResult(code, error) {
  return { ok: false, code, error };
}

export function captureStatusForResult(result = {}) {
  return result.code || (result.ok ? CAPTURE_STATUS.saved : CAPTURE_STATUS.failed);
}

export function captureErrorForResult(result = {}) {
  if (result.warning) return result.message || result.error;
  return result.ok ? "" : result.error;
}
