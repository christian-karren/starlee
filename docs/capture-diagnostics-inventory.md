# Capture Diagnostics Inventory

This branch reuses the existing Starlee capture diagnostics stack rather than introducing a parallel log.

## Existing Diagnostics

- Native menu-bar capture creates a local bridge request through `gui/StatusMenuController.swift` and `gui/StarleeClient.swift`.
- The local service owns `/extension/hello`, `/bridge-health`, `/capture-request`, `/capture-request/status`, `/capture-request/result`, and `/capture-diagnostics/event` in `src/http.rs`.
- Capture request state and diagnostic events are persisted in `~/Starlee/config.json` as `pending_capture_request`, `capture_request_status`, and `capture_diagnostics`.
- Bridge health, redaction, request expiry, result normalization, next-action mapping, and bounded event storage live in `src/engine/bridge.rs`.
- Trace assembly lives in `Engine::last_capture_trace()` in `src/engine.rs`.
- Extension handoff diagnostics are emitted from `sensor/src/background.js`, `sensor/src/background-handoff.js`, and `sensor/src/content.js`.
- Safe extraction diagnostics for article and YouTube payloads are emitted from the sensor payload/extractor path and are covered by existing sensor tests.

## Current Viewing Surfaces

- `starlee diagnostics` prints the latest redacted diagnostic events.
- `starlee diagnostics --last-capture` prints the latest request-correlated trace.
- `starlee doctor` and `/bridge-health` expose setup state, recent redacted diagnostics, last request status, and recommended next actions.
- The macOS menu includes `Show Last Capture Trace...`, which calls `starlee diagnostics --last-capture`.

## Reused Fields And Policies

- Request correlation: `request_id`.
- Runtime identity: browser, extension version/build, Starlee version, app build identifier, git/source paths.
- Request lifecycle: `queued`, `picked_up`, `extracting`, `posted`, `capture_saved`, `capture_failed`, `permission_denied`, `unsupported_page`, `extension_unavailable`, `content_script_unreachable`, and `timed_out`.
- Safe page metadata: bounded `title`, `url`, and `domain` values.
- Safe diagnostic metadata: bounded string map with forbidden key filtering.
- Redaction rejects token/cookie/html/body/selected-text/transcript/embedding/credential-shaped keys or values and redacts credential-shaped messages.

## Missing Fields Addressed Here

- Top-level latest trace summary now includes `browser`, `extension_build`, `desktop_build`, `result_code`, `user_safe_message`, `failure_step`, `next_action`, and `last_extension_check_in`.
- Native menu-bar final UI feedback now records a request-correlated `menu_bar_capture_result_displayed` event with the displayed animation class.
- Bridge result normalization now preserves `token_missing`, `token_invalid`, `service_down`, `service_unreachable`, and `payload_too_large` instead of collapsing them to generic `capture_failed`.

## Privacy Rules To Preserve

Diagnostics must not log article body, selected text, transcript text, tokens, cookies, raw HTML, embeddings, vault contents, OAuth secrets, bearer headers, SAPISID values, or private credential-shaped values. Existing redaction lives in `src/engine/bridge.rs` and browser-side URL/error scrubbing lives in `sensor/src/background-handoff.js` and `sensor/src/content.js`.

## Browser Session Integration Points

- Chrome, Safari, and Firefox branches should emit the same stable request IDs and result codes through `/capture-diagnostics/event` and `/capture-request/result`.
- Browser-specific active-tab, permission, injection, or packaging fixes should stack on this branch and preserve the existing code names above.
- If a browser branch introduces a new failure code, it should add Rust normalization, a safe user message, a next action, and focused tests before emitting the code.
- Expected merge order: land this shared diagnostics branch first, then rebase browser-specific branches so their findings plug into the unified trace.
