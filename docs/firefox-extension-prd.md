# PRD: Starlee Firefox Extension

**Author:** Christian Karren
**Date:** 2026-06-28
**Status:** Draft
**Version:** 1.0
**Quality-Validated:** Yes

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Problem Statement](#problem-statement)
3. [Goals & Success Metrics](#goals--success-metrics)
4. [User Stories](#user-stories)
5. [Functional Requirements](#functional-requirements)
6. [Non-Functional Requirements](#non-functional-requirements)
7. [Technical Considerations](#technical-considerations)
8. [Implementation Roadmap](#implementation-roadmap)
9. [Out of Scope](#out-of-scope)
10. [Open Questions & Risks](#open-questions--risks)
11. [Validation Checkpoints](#validation-checkpoints)
12. [Appendix: Task Breakdown Hints](#appendix-task-breakdown-hints)

---

## Executive Summary

Starlee should add Firefox as a separate extension target that reuses the existing Chromium extraction, payload, options, and local HTTP bridge modules where browser APIs overlap, while introducing Firefox-specific manifest generation, packaging, compatibility tests, and AMO review assets. This is a moderate port rather than a rewrite: `sensor/src/article.js`, `sensor/src/youtube.js`, `sensor/src/payload.js`, `sensor/src/metadata.js`, and the Rust local service can be shared with targeted adapters, but Manifest V3 background lifecycle behavior, local loopback permissions, toolbar/menu-bar capture reliability, and store review requirements must be validated in Firefox instead of inferred from Chrome.

The expected impact is browser coverage for users who read in Firefox without compromising Starlee's local-first model. A successful release lets a Firefox user install from Mozilla Add-ons or a signed self-distribution channel, connect to `http://127.0.0.1:47291`, capture an article or YouTube transcript with one toolbar click after setup, and respond to a Starlee Mac menu-bar capture request with no external upload of page bodies, transcripts, capture tokens, or vault data.

---

## Problem Statement

### Current Situation

Starlee currently ships Chromium-oriented extension source under `sensor/`:

- `sensor/extension/manifest.json` is Manifest V3 with `background.service_worker`, `action`, `storage`, `activeTab`, `tabs`, `alarms`, and `host_permissions: ["http://127.0.0.1/*"]`.
- `sensor/scripts/build.mjs` bundles `content.js`, `background.js`, and `options.js` with esbuild target `chrome120`, then copies one Chromium manifest into `sensor/dist/extension`.
- `scripts/package-chrome-extension.sh` creates a Chrome ZIP, removes sourcemaps, and strips `starlee-config.json`.
- `sensor/test/manifest.test.js` asserts the package remains MV3 and local-only for network host permissions.
- `docs/chrome-extension-prd.md` treats Firefox as out of scope because it has different extension APIs and review process.
- `src/http.rs` exposes local endpoints for `/extension/hello`, `/capture-request`, and `/capture`, with bearer-token authentication and permissive CORS.
- `src/engine.rs` records extension handshake state, creates pending capture requests, and clears them on take. The requested `src/engine/bridge.rs` path does not exist at this commit; the bridge logic is in `src/engine.rs` and `src/http.rs`.

The extraction code is mostly browser-independent DOM logic, but the current distribution path assumes Chrome and the current runtime API usage assumes the `chrome.*` extension namespace.

### User Impact

- **Who is affected:** Starlee users who read in Firefox on macOS, Windows, or Linux and want rendered-page capture instead of copy/paste, bookmarklets, or switching browsers.
- **How they're affected:** They cannot install a review-ready Starlee extension in Firefox. The existing Chrome build may load during development on some Firefox versions, but lifecycle behavior, permissions, AMO review, and menu-bar capture are not validated.
- **Severity:** High for Firefox-primary users, medium for the overall MVP. Without Firefox support, Starlee's browser capture surface excludes a privacy-conscious audience aligned with Starlee's local-first positioning.

### Business Impact

- **Cost of problem:** Lower adoption among Firefox users, additional support questions about unsupported side-loading, and reduced confidence in Starlee as a browser-neutral local memory tool.
- **Opportunity cost:** Firefox is a credibility channel for a privacy-preserving product; missing support weakens the local-first story and delays cross-browser extension architecture work that Safari and future Chromium variants can reuse.
- **Strategic importance:** A Firefox target forces Starlee to define a portable extension architecture instead of letting the Chrome package become the only supported browser integration.

### Why Solve This Now?

The Chrome extension already has the necessary product primitives: rendered article extraction through Mozilla Readability, YouTube transcript extraction, local-only capture posting, a connection options page, toolbar capture, and menu-bar capture request polling. Firefox support is timely because most work can be shared if the team adds a browser target boundary before Chrome-specific assumptions spread further into `sensor/src`.

### Assumptions

- Firefox support targets desktop Firefox only for v1. Mobile Firefox is out of scope.
- The Firefox extension keeps the same local service port default, `47291`, and bearer token model.
- The Firefox package does not embed `starlee-config.json` or a capture token for AMO distribution.
- The Starlee local service remains the source of truth for capture storage, indexing, and menu-bar request state.
- The engineering decision is to maintain one shared sensor codebase with target-specific manifests and adapters, not fork all extension code.

---

## Goals & Success Metrics

### Goal 1: Determine and ship the correct Firefox target shape

- **Description:** Treat Firefox as a first-class extension target with explicit manifest, packaging, and review assets.
- **Metric:** Firefox-specific files and build commands required for a signed package.
- **Baseline:** 0 Firefox-specific package artifacts; Chrome package only.
- **Target:** 1 repeatable Firefox ZIP build with Firefox manifest validation and package inspection.
- **Timeframe:** Within the Firefox extension MVP milestone.
- **Measurement Method:** CI or local release script output plus AMO validation result.

### Goal 2: Reuse at least 70% of current sensor logic

- **Description:** Preserve shared extraction and payload behavior while isolating browser API differences.
- **Metric:** Shared modules reused unchanged or with browser-neutral adapters.
- **Baseline:** 0 target boundary; Chrome code is bundled directly.
- **Target:** Reuse `article.js`, `youtube.js`, `metadata.js`, `access.js`, `payload.js`, and most options/local bridge logic across Chrome and Firefox.
- **Timeframe:** End of implementation Phase 2.
- **Measurement Method:** File-level code review and tests that run the same fixture suite for both targets.

### Goal 3: Preserve one-click capture behavior in Firefox

- **Description:** A Firefox user can save the active article or YouTube video from the toolbar or Starlee Mac menu bar after setup.
- **Metric:** User actions after setup on supported pages.
- **Baseline:** Not supported in Firefox.
- **Target:** 1 toolbar click for pages with granted access; 1 Mac menu-bar click when Firefox is foreground and content script polling is active; <=2 user permission confirmations on first use.
- **Timeframe:** MVP beta.
- **Measurement Method:** Manual smoke checklist and browser automation where WebExtension support allows.

### Goal 4: Maintain local-first privacy guarantees

- **Description:** Captured page bodies, transcripts, URLs, and tokens remain local to the user's machine.
- **Metric:** External network destinations receiving captured content.
- **Baseline:** Chrome design posts only to `127.0.0.1`.
- **Target:** 0 non-local network destinations in Firefox package source and runtime capture tests.
- **Timeframe:** Continuous, required before AMO submission.
- **Measurement Method:** Package grep, runtime network inspection, AMO disclosure review, and code review.

---

## User Stories

### Story 1: Install Starlee in Firefox

**As a** Firefox user,
**I want to** install a signed Starlee extension,
**So that I can** capture rendered pages without using Chrome or enabling temporary debug extensions.

**Acceptance Criteria:**

- [ ] Firefox package contains a Firefox-compatible `manifest.json`, bundled scripts, options page, PNG icons, and no Chrome-only store metadata.
- [ ] Package validates with Mozilla's extension tooling or AMO upload validation.
- [ ] Listing or self-distribution notes state that Starlee communicates with `127.0.0.1` and stores captures locally.
- [ ] Package contains no `starlee-config.json`, bearer token, vault files, SQLite files, model files, sourcemaps, or `node_modules`.
- [ ] Extension can be installed on a clean Firefox release profile without temporary add-on debugging.

**Task Breakdown Hint:**

- Task 1.1: Add Firefox manifest generation and inspection script (~8h)
- Task 1.2: Add Firefox build command with esbuild target and package output (~6h)
- Task 1.3: Prepare AMO listing disclosures and screenshots (~6h)
- Task 1.4: Run AMO validation and record findings (~4h plus review wait)

**Dependencies:** REQ-001, REQ-009, existing `sensor/extension` assets.

### Story 2: Connect Firefox to local Starlee

**As a** Starlee user,
**I want to** confirm Firefox can reach the local Starlee service,
**So that I can** fix setup before trying to capture a page.

**Acceptance Criteria:**

- [ ] Options page sends `POST http://127.0.0.1:47291/extension/hello` with browser name `Firefox`, extension version, and `can_capture_active_tab`.
- [ ] Options page distinguishes token missing, token invalid, service down, local permission blocked, and unexpected service response.
- [ ] Full capture token is never shown in page UI, extension console logs, package contents, or AMO metadata.
- [ ] `starlee doctor` can report a recent Firefox handshake using existing extension state or an additive browser-aware state.
- [ ] Connection status updates within 5 seconds after opening options on a running local service.

**Task Breakdown Hint:**

- Task 2.1: Add browser API adapter and Firefox browser-name detection (~4h)
- Task 2.2: Verify loopback fetch and CORS preflight behavior in Firefox (~4h)
- Task 2.3: Add options-page Firefox diagnostics tests (~6h)
- Task 2.4: Decide whether `ExtensionState` needs browser-specific history (~4h)

**Dependencies:** REQ-002, REQ-003, `src/http.rs`, `src/engine.rs`.

### Story 3: Capture from the Firefox toolbar

**As a** reader in Firefox,
**I want to** click the Starlee toolbar button,
**So that I can** save the visible article or YouTube transcript to my local Starlee vault.

**Acceptance Criteria:**

- [ ] Toolbar click reaches the active tab using Firefox-supported WebExtension APIs.
- [ ] Article captures include title, URL, site, byline when available, publication date when available, readable text, selected text when available, and HTML metadata.
- [ ] YouTube captures include title, channel, URL, and transcript segments when rendered transcript nodes are available.
- [ ] Payload posts to `POST /capture` with bearer authentication and the existing capture payload schema.
- [ ] User-visible capture result appears within 2 seconds for payloads under 2 MiB and within 8 seconds for payloads under 16 MiB.
- [ ] Unsupported pages produce `empty_extract`, permission, or service errors with a next action.

**Task Breakdown Hint:**

- Task 3.1: Add Firefox toolbar/action compatibility layer (~6h)
- Task 3.2: Run shared article and YouTube fixture tests under Firefox target (~8h)
- Task 3.3: Add manual smoke cases for restricted pages and selected text (~4h)
- Task 3.4: Verify badge or notification feedback behavior (~4h)

**Dependencies:** REQ-004, REQ-005, REQ-006.

### Story 4: Capture from the Starlee Mac menu bar into Firefox

**As a** Mac user who clicks the Starlee menu-bar button,
**I want to** save the active Firefox page,
**So that I can** use one native capture control regardless of browser.

**Acceptance Criteria:**

- [ ] Mac app continues to create pending capture requests through `POST /capture-request`.
- [ ] Firefox extension can pick up `GET /capture-request` and clear exactly one pending request through the existing local service behavior.
- [ ] Duplicate request IDs do not create duplicate captures during one Firefox runtime session.
- [ ] If Firefox background execution cannot maintain polling under MV3, the extension documents and implements the chosen fallback: content-script polling while visible, MV2 persistent background for Firefox, alarm-driven polling, or user-invoked toolbar fallback.
- [ ] Menu-bar capture result is observable through extension storage and local diagnostics within 5 seconds of the request.

**Task Breakdown Hint:**

- Task 4.1: Test Firefox MV3 background/service-worker wake behavior for local polling (~8h)
- Task 4.2: Decide MV2 vs MV3 for initial Firefox release based on measured polling reliability (~4h)
- Task 4.3: Add duplicate-prevention and result-state tests (~6h)
- Task 4.4: Update Mac menu-bar QA checklist for Firefox foreground capture (~4h)

**Dependencies:** REQ-007, REQ-008, local service endpoints.

---

## Functional Requirements

### Must Have (P0) - Critical for Launch

#### REQ-001: Firefox Extension Target Boundary

**Priority:** P0

**Description:** The build system must produce a Firefox-specific extension package without forking the full `sensor/src` tree.

**Acceptance Criteria:**

- [ ] Build supports at least `chrome` and `firefox` targets.
- [ ] Shared modules remain in `sensor/src` unless a Firefox-specific API difference requires an adapter.
- [ ] Firefox manifest generation does not mutate the Chrome manifest in place.
- [ ] Firefox output writes to a separate directory such as `release/firefox-extension` or `sensor/dist/firefox-extension`.
- [ ] Chrome package output remains byte-for-byte functionally equivalent except versioned build metadata.

**Technical Specification:**

```text
Shared code:
  sensor/src/article.js
  sensor/src/youtube.js
  sensor/src/metadata.js
  sensor/src/access.js
  sensor/src/payload.js

Target adapters:
  browser namespace adapter
  manifest generator
  package inspector
  store/review metadata
```

**Task Breakdown:**

- Add target argument to `sensor/scripts/build.mjs`: Medium (5h)
- Add Firefox manifest template or transform: Medium (5h)
- Add package output convention: Small (2h)
- Add regression check for Chrome target: Small (3h)

**Dependencies:** Existing `sensor/scripts/build.mjs`, `sensor/extension/manifest.json`.

#### REQ-002: Firefox Manifest Compatibility Decision

**Priority:** P0

**Description:** Engineering must make an explicit Manifest V2 versus Manifest V3 decision for Firefox before implementation begins.

**Acceptance Criteria:**

- [ ] Decision record documents Firefox release version tested, MV2 support status, MV3 support status, and chosen launch manifest.
- [ ] If MV3 is chosen, background `service_worker` behavior is tested for toolbar capture, options handshake, alarms, and menu-bar request pickup.
- [ ] If MV2 is chosen, package uses Firefox-supported `background.scripts` or persistent background configuration and documents divergence from Chrome MV3.
- [ ] Permission prompts are documented with exact strings or screenshots for AMO review.
- [ ] Chrome MV3 remains unchanged by the Firefox decision.

**Technical Specification:**

```json
{
  "decision_inputs": [
    "Firefox MV3 background lifecycle",
    "local loopback host permission behavior",
    "alarms support",
    "AMO review expectations",
    "menu-bar polling reliability"
  ],
  "launch_rule": "Choose the manifest version that preserves toolbar capture and menu-bar request pickup with documented review compliance."
}
```

**Task Breakdown:**

- Test MV3 behavior in Firefox release and ESR if supported: Medium (8h)
- Prototype MV2 manifest if MV3 polling fails: Medium (6h)
- Write decision record in PR description or docs: Small (2h)
- Review with product/engineering owner: Small (1h)

**Dependencies:** Firefox release install, local Starlee service.

#### REQ-003: Browser API Adapter

**Priority:** P0

**Description:** Extension code must use a browser-neutral adapter for runtime messaging, storage, tabs, alarms, action/browserAction, and manifest APIs.

**Acceptance Criteria:**

- [ ] No new Firefox code duplicates full copies of `background.js`, `content.js`, or `options.js`.
- [ ] Adapter supports `chrome.*` callback APIs and Firefox `browser.*` promise APIs.
- [ ] Adapter exposes runtime message, storage local get/set, tabs query/sendMessage, action badge APIs, alarms, and runtime URL/getManifest.
- [ ] Unit tests cover Chrome-style and Firefox-style adapter behavior with mocked APIs.
- [ ] Existing Chrome tests continue to pass.

**Technical Specification:**

```javascript
const ext = createExtensionApi(globalThis.browser || globalThis.chrome);
await ext.storage.local.get(["captureToken", "capturePort"]);
await ext.runtime.sendMessage({ type: "STARLEE_STATUS" });
```

**Task Breakdown:**

- Implement adapter module: Medium (6h)
- Migrate background/content/options to adapter: Medium (8h)
- Add mocked API tests: Medium (6h)
- Run Chrome regression tests: Small (2h)

**Dependencies:** REQ-001.

#### REQ-004: Local Loopback Permission and CORS Validation

**Priority:** P0

**Description:** Firefox extension must communicate only with the local Starlee service and must pass Firefox permission and CORS behavior for `127.0.0.1`.

**Acceptance Criteria:**

- [ ] Manifest includes the minimum Firefox-supported permission or host permission needed for `http://127.0.0.1/*`.
- [ ] Runtime fetch to `/extension/hello`, `/capture-request`, and `/capture` succeeds with bearer auth when `starlee serve` is running.
- [ ] Runtime fetch fails closed when token is missing or invalid.
- [ ] Package scan finds no non-local `fetch("http...")` or `fetch("https...")` destinations.
- [ ] AMO disclosure states the local network purpose in plain language.

**Technical Specification:**

```http
POST http://127.0.0.1:47291/extension/hello
GET  http://127.0.0.1:47291/capture-request
POST http://127.0.0.1:47291/capture
Authorization: Bearer <local token>
```

**Task Breakdown:**

- Verify Firefox loopback host permission: Medium (4h)
- Add Firefox package grep to inspector: Small (3h)
- Add service-down and token-invalid smoke cases: Small (3h)
- Document AMO disclosure copy: Small (2h)

**Dependencies:** `src/http.rs`, local config token.

#### REQ-005: Shared Article Capture

**Priority:** P0

**Description:** Firefox capture must reuse the existing article extraction path and preserve the capture payload contract.

**Acceptance Criteria:**

- [ ] Firefox uses Mozilla Readability via `sensor/src/article.js`.
- [ ] Article payloads keep `version: 1`, `type: "article"`, `url`, `access`, `dom_extract`, `tags`, and `consumed_at`.
- [ ] Selected text is included for article captures when present.
- [ ] Ambiguous access classification remains fail-closed as `restricted`.
- [ ] Shared fixture tests pass for article extraction.

**Technical Specification:**

```json
{
  "version": 1,
  "type": "article",
  "url": "https://example.com/story",
  "access": "public",
  "dom_extract": {
    "title": "A durable browser memory",
    "text": "..."
  },
  "consumed_at": "2026-06-28T00:00:00.000Z"
}
```

**Task Breakdown:**

- Run existing `sensor/test/article.test.js` for shared modules: Small (2h)
- Add Firefox-specific active-tab fixture smoke: Medium (5h)
- Verify selected text behavior in Firefox: Small (2h)
- Record payload compatibility result: Small (1h)

**Dependencies:** REQ-003.

#### REQ-006: Shared YouTube Capture

**Priority:** P0

**Description:** Firefox capture must reuse the existing YouTube extraction path and preserve transcript segment schema.

**Acceptance Criteria:**

- [ ] Firefox detects YouTube watch URLs.
- [ ] Transcript segments use `{ "t": seconds, "text": segment }`.
- [ ] Capture succeeds with metadata when transcript segments are not rendered.
- [ ] Extension does not bypass YouTube access controls or call unsupported transcript APIs.
- [ ] Shared YouTube fixture tests pass.

**Technical Specification:**

```json
{
  "type": "youtube",
  "dom_extract": { "site": "youtube.com", "text": "" },
  "transcript": [{ "t": 62, "text": "Hello brain" }]
}
```

**Task Breakdown:**

- Run `sensor/test/youtube.test.js` for shared modules: Small (2h)
- Add manual Firefox YouTube transcript smoke: Medium (4h)
- Add transcript unavailable smoke: Small (2h)
- Confirm AMO disclosure does not imply bypassing access controls: Small (1h)

**Dependencies:** REQ-003, YouTube rendered transcript UI.

#### REQ-007: Toolbar Capture in Firefox

**Priority:** P0

**Description:** Firefox toolbar click must capture the active tab using Firefox-supported WebExtension APIs.

**Acceptance Criteria:**

- [ ] Toolbar click sends a capture request to the content script in the active tab.
- [ ] Pages where extensions cannot run produce a permission or unsupported-page result.
- [ ] Badge or notification result appears within 2 seconds for successful local captures under 2 MiB.
- [ ] Capture failure stores `lastCaptureStatus` and `lastCaptureError` in extension local storage.
- [ ] Firefox toolbar capture is included in the release smoke checklist.

**Technical Specification:**

```text
toolbar click -> tabs.query active/currentWindow -> tabs.sendMessage STARLEE_CAPTURE_NOW
content script -> capturePayload(document) -> runtime.sendMessage STARLEE_CAPTURE
background -> POST /capture -> badge/result storage
```

**Task Breakdown:**

- Implement action/browserAction compatibility: Medium (5h)
- Verify active tab permission behavior: Medium (4h)
- Add result storage smoke: Small (3h)
- Add release checklist row: Small (1h)

**Dependencies:** REQ-003, REQ-004, REQ-005.

#### REQ-008: Menu-Bar Request Polling in Firefox

**Priority:** P0

**Description:** Firefox extension must either support Starlee Mac menu-bar request polling or explicitly mark it unavailable with a measured reason and user-facing fallback.

**Acceptance Criteria:**

- [ ] Extension can call `GET /capture-request` with bearer auth.
- [ ] One pending request is captured once and cleared through existing `Engine::take_capture_request`.
- [ ] If MV3 background service worker sleep prevents reliable polling, the selected fallback is implemented and documented.
- [ ] Polling while idle sends no page content and makes no more than 4 local requests per minute per extension context.
- [ ] Menu-bar request result is visible in options diagnostics.

**Technical Specification:**

```text
Existing Chrome behavior:
  background alarm period: 1 minute
  content visible-page interval: 3 seconds
  local request endpoint: GET /capture-request

Firefox must measure:
  background wake reliability
  content-script interval reliability on visible pages
  duplicate request behavior
```

**Task Breakdown:**

- Measure MV3 service-worker polling for 30 minutes: Medium (6h)
- Measure visible content-script polling for 10 foreground pages: Medium (4h)
- Add duplicate request test or manual verification: Small (3h)
- Implement fallback copy/state if unsupported: Small (3h)

**Dependencies:** REQ-002, REQ-004, `src/engine.rs`.

#### REQ-009: Firefox Package and Review Process

**Priority:** P0

**Description:** Starlee must define a repeatable Firefox package and review process separate from Chrome Web Store packaging.

**Acceptance Criteria:**

- [ ] Package script creates `release/firefox-extension/starlee-firefox-extension-<version>.zip` or an equivalent named artifact.
- [ ] Inspection script verifies required files and rejects secrets, local data, sourcemaps, model files, database files, and non-local network destinations.
- [ ] AMO submission notes describe local loopback communication, single purpose, data handling, and permissions.
- [ ] Version number matches `sensor/package.json`.
- [ ] Review checklist includes install, handshake, toolbar capture, menu-bar capture or fallback, and update path.

**Technical Specification:**

```sh
./scripts/package-firefox-extension.sh
./scripts/inspect-firefox-extension-package.sh release/firefox-extension/starlee-firefox-extension-0.1.0.zip
```

**Task Breakdown:**

- Add package script based on Chrome script: Small (4h)
- Add inspector script based on Chrome inspector: Small (4h)
- Add AMO submission notes document: Medium (5h)
- Run validation on clean checkout: Small (2h)

**Dependencies:** REQ-001, REQ-002.

### Should Have (P1) - Important but Not Blocking

#### REQ-010: Browser-Aware Extension Diagnostics

**Priority:** P1

**Description:** Starlee diagnostics should distinguish Firefox from Chrome when multiple browser extensions are installed.

**Acceptance Criteria:**

- [ ] Handshake stores browser name `Firefox`.
- [ ] `starlee doctor` output can identify the most recent browser extension handshake.
- [ ] If feasible without config migration risk, local config stores per-browser handshake history.
- [ ] Diagnostics do not store browsing URLs or captured content.

**Task Breakdown:**

- Evaluate `ExtensionState` schema change: Small (3h)
- Add migration/default handling if per-browser state is chosen: Medium (5h)
- Add Rust tests for backward-compatible config parsing: Medium (4h)

**Dependencies:** `src/config.rs`, `src/engine.rs`.

#### REQ-011: Automated Firefox WebExtension QA

**Priority:** P1

**Description:** The project should add automated Firefox extension tests where tooling supports stable execution.

**Acceptance Criteria:**

- [ ] Shared Node fixture tests continue to run without a browser.
- [ ] At least one automated Firefox install or smoke test covers options handshake or toolbar capture.
- [ ] Test results are documented if full automation is blocked by tooling limitations.
- [ ] Manual QA remains required before AMO submission.

**Task Breakdown:**

- Research Firefox WebExtension automation path: Medium (4h)
- Add one automated smoke if feasible: Medium (8h)
- Document blocked cases: Small (2h)

**Dependencies:** REQ-001, REQ-009.

### Nice to Have (P2) - Future Enhancement

#### REQ-012: Unified Multi-Browser Release Matrix

**Priority:** P2

**Description:** Starlee should maintain a release matrix for Chrome, Firefox, and Safari extension targets.

**Acceptance Criteria:**

- [ ] Matrix lists manifest version, package script, store/review channel, supported capture entry points, and known limitations for each browser.
- [ ] Matrix is updated during extension releases.
- [ ] Release notes identify browser-specific changes.

**Task Breakdown:**

- Create release matrix doc: Small (3h)
- Link Chrome, Firefox, and Safari docs: Small (2h)
- Add release checklist item: Small (1h)

**Dependencies:** Chrome and Safari docs.

---

## Non-Functional Requirements

### Performance

- Toolbar capture result feedback: <=2 seconds for payloads under 2 MiB on a running local service.
- Large payload capture: <=8 seconds for payloads under the existing 16 MiB `MAX_CAPTURE_BYTES` server limit.
- Menu-bar polling: <=4 local idle requests per minute per extension context unless Firefox alarms require a documented lower frequency.
- Options status refresh: <=5 seconds after opening options when local service is running.
- Shared extraction tests: <=10 seconds total on a developer machine for article and YouTube fixture tests.

### Security

- Capture token remains in Firefox extension local storage or user-provided local pairing only; it must not be bundled in AMO packages.
- All extension network requests with captured content must target `127.0.0.1`.
- Package inspection must fail on bearer-token-like strings, vault paths, SQLite files, model files, sourcemaps, `node_modules`, and non-local fetch destinations.
- Local service authentication remains `Authorization: Bearer <token>` for `/extension/hello`, `/capture-request`, and `/capture`.
- Logs and diagnostics must store result codes and timestamps, not article bodies or transcripts.

### Reliability

- Toolbar capture success rate: >=90% across the MVP Firefox fixture/manual page set.
- Failure classification: >=95% of failed captures return one of token missing, token invalid, service down, permission denied, unsupported page, empty extract, payload too large, or service error.
- Duplicate menu-bar captures: 0 duplicate records from one pending request in a 20-request manual stress run.
- Update path: installed beta extension can update to the next signed version without losing stored token or port settings.

### Compatibility

- Required browser: current Firefox desktop release at implementation time.
- Should test: Firefox ESR if the release strategy includes privacy-conscious or enterprise users.
- Required OS for menu-bar path: macOS with Starlee app/service installed.
- Required OS for toolbar-only path: macOS, Windows, and Linux where `starlee serve` can run.
- Not required: Firefox for Android, iOS browsers, Tor Browser, LibreWolf, and enterprise managed extension deployment.

### Privacy and Compliance

- AMO listing must disclose local host communication and data handling.
- Extension must not request remote host permissions beyond `http://127.0.0.1/*` for launch.
- Extension must not collect analytics, browsing history, page text, transcript text, URLs, or token material for external reporting.
- Review notes must explain why content scripts need page access for rendered article extraction and menu-bar capture.

---

## Technical Considerations

### System Architecture

**Current Architecture:** Starlee has a Rust local service, local config at `~/Starlee/config.json`, a local vault/index pipeline, a macOS menu-bar app, and a Chromium WebExtension under `sensor/`. The extension extracts visible DOM content and sends payloads to `src/http.rs` endpoints at `127.0.0.1`.

**Proposed Changes:** Add a Firefox target boundary while keeping Starlee's extension as a thin local sensor. Shared DOM extraction and payload creation stay in `sensor/src`; browser-specific behavior moves to an adapter and manifest/package scripts.

```text
Firefox toolbar / content script
        |
        v
Shared sensor extraction modules
        |
        v
Firefox background or event page adapter
        |
        v
http://127.0.0.1:47291
        |
        v
src/http.rs -> Engine::capture / Engine::record_extension_hello / Engine::take_capture_request
        |
        v
~/Starlee vault + index
```

**Key Components:**

1. **Shared extraction modules:** `article.js`, `youtube.js`, `metadata.js`, `access.js`, and `payload.js`.
2. **Browser API adapter:** Normalizes `chrome.*` and `browser.*` runtime behavior.
3. **Firefox manifest target:** Encodes MV2/MV3 decision, permissions, background strategy, content scripts, options, and action/browserAction.
4. **Local Starlee service:** Existing endpoints in `src/http.rs`.
5. **Engine bridge state:** Existing handshake and pending capture request methods in `src/engine.rs`.
6. **Firefox package/review tooling:** Package script, inspector, and AMO notes.

### Shared Versus Firefox-Specific Code

**Shared with minimal or no changes:**

- `sensor/src/article.js`: Uses `@mozilla/readability`, which is DOM-based and already maintained for Mozilla-origin readability parsing.
- `sensor/src/youtube.js`: DOM selectors may need manual smoke updates but not a browser fork.
- `sensor/src/metadata.js`, `sensor/src/access.js`, `sensor/src/payload.js`: Browser-neutral DOM/data logic.
- `sensor/test/article.test.js`, `sensor/test/youtube.test.js`, `sensor/test/access.test.js`: Shared fixture tests.
- `src/http.rs`: Existing local endpoint behavior is browser-agnostic.
- `src/engine.rs`: Existing handshake and pending request storage can work for Firefox, with optional per-browser diagnostic expansion.

**Needs Firefox-specific adaptation:**

- `sensor/extension/manifest.json`: MV3 service worker and `action` semantics may need Firefox-specific manifest output.
- `sensor/scripts/build.mjs`: Current target is `chrome120`; needs `firefox` build target and manifest selection.
- `sensor/src/background.js`: Uses `chrome.action`, `chrome.alarms`, callback messaging, and background lifecycle assumptions.
- `sensor/src/content.js`: Uses `chrome.runtime` and menu-bar visible-page polling.
- `sensor/src/options.js`: Uses `chrome.storage` and `chrome.runtime`; adapter needed.
- `scripts/package-chrome-extension.sh`: Needs Firefox package variant and review artifacts.
- `sensor/test/manifest.test.js`: Needs Firefox manifest/package tests in addition to Chrome assertions.

### API Specifications

#### Endpoint: Extension Hello

```http
POST /extension/hello
Authorization: Bearer <local token>
Content-Type: application/json

{
  "browser": "Firefox",
  "extension_version": "0.1.0",
  "can_capture_active_tab": true
}
```

Expected response is the serialized `ExtensionState` from the Rust engine. Firefox must handle 200, 401, service down, malformed JSON, and CORS/preflight failures.

#### Endpoint: Capture

```http
POST /capture
Authorization: Bearer <local token>
Content-Type: application/json

CapturePayload version 1
```

Payloads over 16 MiB are rejected by `src/http.rs` with 413 behavior. Firefox UI should map that to `payload_too_large`.

#### Endpoint: Menu-Bar Request

```http
POST /capture-request
GET /capture-request
Authorization: Bearer <local token>
```

The Mac app creates a request and the extension polls/takes it. `Engine::take_capture_request` clears the pending request on read, so Firefox must prevent duplicate captures after pickup.

### Testing Strategy

**Unit and fixture tests:**

- Shared article extraction fixtures.
- Shared YouTube transcript fixtures.
- Access classification fixtures.
- Browser API adapter mocks for Chrome and Firefox.
- Manifest tests for Chrome and Firefox target outputs.

**Integration tests:**

- Local service handshake against `starlee serve` or a test server.
- Authenticated capture POST to `127.0.0.1`.
- Package inspection for forbidden files and network destinations.
- Optional Firefox WebExtension automation for install/options if stable tooling is available.

**Manual QA:**

- Clean Firefox profile install.
- Options token and port entry.
- Service running handshake.
- Service stopped error.
- Token invalid error.
- Toolbar article capture.
- Toolbar selected-text article capture.
- Toolbar YouTube transcript capture.
- Menu-bar request capture or documented fallback.
- Package update preserving settings.

---

## Implementation Roadmap

### Phase 1: Discovery and Decision (Week 1)

**Goal:** Make the Firefox extension target decision with measured MV2/MV3 evidence.

**Tasks:**

- [ ] Task 1.1: Test current MV3 package in Firefox temporary add-on mode (REQ-002)
  - Complexity: Medium (4h)
  - Dependencies: existing Chrome build
  - Owner: Extension engineer
- [ ] Task 1.2: Measure background/service worker and alarm behavior for polling (REQ-008)
  - Complexity: Medium (6h)
  - Dependencies: local service running
  - Owner: Extension engineer
- [ ] Task 1.3: Record MV2/MV3 decision and permission prompt findings (REQ-002)
  - Complexity: Small (2h)
  - Dependencies: Tasks 1.1-1.2
  - Owner: Extension engineer

**Validation Checkpoint:** Decision record states whether Firefox launch uses MV2 or MV3 and why.

### Phase 2: Shared Target Architecture (Week 1-2)

**Goal:** Add target-specific build structure without copying the extension.

**Tasks:**

- [ ] Task 2.1: Add browser API adapter (REQ-003)
  - Complexity: Medium (6h)
  - Dependencies: Phase 1 decision
  - Owner: Extension engineer
- [ ] Task 2.2: Add Firefox manifest generation (REQ-001, REQ-002)
  - Complexity: Medium (5h)
  - Dependencies: Phase 1 decision
  - Owner: Extension engineer
- [ ] Task 2.3: Update build script for `chrome` and `firefox` targets (REQ-001)
  - Complexity: Medium (5h)
  - Dependencies: Task 2.2
  - Owner: Extension engineer
- [ ] Task 2.4: Add manifest tests for both targets (REQ-001, REQ-009)
  - Complexity: Medium (5h)
  - Dependencies: Task 2.3
  - Owner: Extension engineer

**Validation Checkpoint:** Chrome target still builds and Firefox target produces a valid extension directory.

### Phase 3: Runtime Capture and Local Bridge (Week 2)

**Goal:** Verify Firefox can handshake, capture, and process menu-bar requests or fallback states.

**Tasks:**

- [ ] Task 3.1: Verify local loopback fetch and CORS behavior (REQ-004)
  - Complexity: Medium (4h)
  - Dependencies: Phase 2
  - Owner: Extension engineer
- [ ] Task 3.2: Verify toolbar article and YouTube capture (REQ-005, REQ-006, REQ-007)
  - Complexity: Medium (8h)
  - Dependencies: Task 3.1
  - Owner: Extension engineer
- [ ] Task 3.3: Verify menu-bar request pickup or implement fallback state (REQ-008)
  - Complexity: Medium (8h)
  - Dependencies: Task 3.1
  - Owner: Extension engineer
- [ ] Task 3.4: Update diagnostics for Firefox browser name (REQ-010)
  - Complexity: Small (3h)
  - Dependencies: Task 3.1
  - Owner: Extension engineer

**Validation Checkpoint:** Firefox can save an article from toolbar and produce a measured menu-bar result.

### Phase 4: Packaging, Review, and QA (Week 3)

**Goal:** Produce a signed/review-ready Firefox artifact with documented privacy behavior.

**Tasks:**

- [ ] Task 4.1: Add package and inspection scripts (REQ-009)
  - Complexity: Medium (8h)
  - Dependencies: Phase 3
  - Owner: Extension engineer
- [ ] Task 4.2: Add AMO review notes and listing assets (REQ-009)
  - Complexity: Medium (6h)
  - Dependencies: Task 4.1
  - Owner: Product/engineering
- [ ] Task 4.3: Run clean-profile QA matrix (REQ-011)
  - Complexity: Medium (6h)
  - Dependencies: Task 4.1
  - Owner: QA/extension engineer
- [ ] Task 4.4: Submit to AMO unlisted or listed beta channel (REQ-009)
  - Complexity: Small (3h plus review wait)
  - Dependencies: Task 4.3
  - Owner: Release owner

**Validation Checkpoint:** Package passes inspection, AMO validation, and clean-profile smoke tests.

### Effort Estimation

- Phase 1: 12 hours
- Phase 2: 21 hours
- Phase 3: 23 hours
- Phase 4: 23 hours plus review wait
- **Total:** ~79 engineering/release hours
- **Risk Buffer:** +25% (~20 hours) for Firefox MV3 polling and AMO review issues
- **Final Estimate:** ~99 hours, roughly 2-3 focused engineering weeks depending on review wait

---

## Out of Scope

Explicitly NOT included in this release:

1. **Firefox for Android**
   - Reason: Different extension availability and mobile capture UX.
   - Future: Reassess after desktop Firefox launch.

2. **Tor Browser or hardened Firefox forks**
   - Reason: Local loopback, extension permissions, and fingerprinting protections differ materially.
   - Future: Document community-tested configurations if demand appears.

3. **Remote sync or hosted capture**
   - Reason: Violates the local-first scope of this extension target.
   - Future: Separate product decision if Starlee adds opt-in sync.

4. **Automatic extension installation**
   - Reason: Browser stores require user approval and Firefox signing.
   - Future: Installer may open the listing, but the user must approve install.

5. **Native messaging host**
   - Reason: Existing local HTTP bridge is sufficient for the MVP and already shared with Chrome.
   - Future: Consider native messaging only if AMO or Firefox blocks loopback fetch in a way that cannot be mitigated.

6. **Rewriting extraction logic**
   - Reason: Existing Readability and YouTube extraction tests cover the core payload behavior.
   - Future: Improve selectors based on cross-browser QA failures.

---

## Open Questions & Risks

### Open Questions

#### Q1: Should Firefox launch with MV2 or MV3?

- **Current Status:** Existing code is Chrome MV3; Firefox support and lifecycle behavior must be measured.
- **Options:** (A) Firefox MV3 for future alignment, (B) Firefox MV2 for polling reliability if supported, (C) toolbar-only MV3 beta with menu-bar fallback.
- **Owner:** Extension engineer.
- **Deadline:** End of Phase 1.
- **Impact:** High. Affects background code, AMO review, menu-bar capture, and long-term maintenance.

#### Q2: How should Firefox receive the capture token?

- **Current Status:** Store packages must not include `starlee-config.json`; options page can accept token manually.
- **Options:** (A) Manual paste in options, (B) local pairing page served by Starlee, (C) one-time copy button from desktop app.
- **Owner:** Product/engineering.
- **Deadline:** Before AMO beta.
- **Impact:** Medium. Affects activation friction and token exposure risk.

#### Q3: Should local config track per-browser handshakes?

- **Current Status:** `ExtensionState` stores one browser/version/last handshake.
- **Options:** (A) Keep latest handshake only, (B) add per-browser map, (C) add browser-specific diagnostic log without config migration.
- **Owner:** Rust/extension engineer.
- **Deadline:** Phase 3.
- **Impact:** Medium. Affects `starlee doctor` clarity when Chrome and Firefox are both installed.

### Risks & Mitigation

| Risk | Likelihood | Impact | Severity | Mitigation | Contingency |
|------|------------|--------|----------|------------|-------------|
| Firefox MV3 background sleep prevents menu-bar polling | Medium | High | High | Measure in Phase 1; consider MV2 or content-visible polling | Ship toolbar-first beta and mark menu-bar capture unavailable in Firefox |
| AMO rejects local loopback or broad content script explanation | Medium | High | High | Provide single-purpose disclosure and local-only package scan evidence | Adjust permissions, use optional host permissions, or submit unlisted for testing |
| `chrome.*` callback assumptions fail under Firefox `browser.*` promises | High | Medium | High | Add adapter with mocks and migrate all runtime calls | Keep a small Firefox adapter wrapper around existing modules |
| YouTube transcript selectors differ in Firefox-rendered DOM | Medium | Medium | Medium | Add manual YouTube QA and fixture updates | Save metadata-only YouTube records when transcript is unavailable |
| Token accidentally bundled in package | Low | Critical | High | Add Firefox inspector with token-pattern grep | Block release and rotate affected local token if needed |
| Duplicate menu-bar captures from multiple visible tabs polling | Medium | Medium | Medium | Use request IDs and local processed-request set; rely on server take semantics | Limit content polling to active/visible tab contexts if duplicate observed |

---

## Validation Checkpoints

### Checkpoint 1: Firefox Compatibility Decision

**Criteria:**

- [ ] MV2/MV3 decision documented with tested Firefox version.
- [ ] Toolbar click path tested in temporary add-on mode.
- [ ] Menu-bar polling behavior measured for at least 30 minutes.
- [ ] Local loopback permission behavior recorded.

**If Failed:** Do not start packaging; choose toolbar-only beta or defer Firefox target.

### Checkpoint 2: Shared Build Target

**Criteria:**

- [ ] `chrome` target still builds.
- [ ] `firefox` target builds into a separate output directory.
- [ ] Manifest tests pass for both targets.
- [ ] Shared extraction tests pass.

**If Failed:** Fix target boundary before runtime QA.

### Checkpoint 3: Runtime Capture

**Criteria:**

- [ ] Options handshake succeeds with running service.
- [ ] Token invalid and service down states are visible.
- [ ] Toolbar article capture succeeds.
- [ ] Toolbar YouTube capture succeeds or records transcript unavailable state.
- [ ] Menu-bar capture succeeds or fallback is documented in product copy and QA notes.

**If Failed:** Block AMO submission until capture and setup states are reliable.

### Checkpoint 4: Package and Review

**Criteria:**

- [ ] Firefox package inspector passes.
- [ ] AMO validation passes.
- [ ] Package contains no local secrets or forbidden artifacts.
- [ ] AMO notes explain permissions, loopback, and local-first data handling.

**If Failed:** Fix package/review issues before any user-facing beta.

### Checkpoint 5: Beta Release

**Criteria:**

- [ ] Clean Firefox profile installs signed package.
- [ ] Stored token and port survive extension update.
- [ ] `starlee doctor` reports recent Firefox handshake or latest extension handshake.
- [ ] No captured content leaves `127.0.0.1` in runtime network inspection.
- [ ] Known limitations are documented in release notes.

**If Failed:** Hold listed launch and keep distribution to internal/unlisted testers.

---

## Appendix: Task Breakdown Hints

### Suggested Task Structure

**Discovery and decision (3 tasks, ~12 hours)**

1. Test current MV3 package in Firefox temporary add-on mode (4h)
2. Measure background/service-worker and alarm polling behavior (6h)
3. Document MV2/MV3 decision and permission prompts (2h)

**Build architecture (5 tasks, ~24 hours)**

4. Add browser API adapter with Chrome and Firefox mocks (6h)
5. Migrate background/content/options to adapter (8h)
6. Add Firefox manifest template or transform (5h)
7. Add `firefox` build target and output convention (3h)
8. Add Chrome and Firefox manifest regression tests (2h)

**Runtime behavior (6 tasks, ~27 hours)**

9. Verify loopback fetch and CORS in Firefox (4h)
10. Verify options handshake and status states (4h)
11. Verify toolbar article capture (4h)
12. Verify toolbar YouTube capture (4h)
13. Verify menu-bar polling or fallback (8h)
14. Update diagnostics for browser name (3h)

**Packaging and review (5 tasks, ~24 hours plus review wait)**

15. Add Firefox package script (4h)
16. Add Firefox package inspector (4h)
17. Prepare AMO listing notes and screenshots (6h)
18. Run clean-profile QA matrix (6h)
19. Submit AMO beta package and track review findings (4h plus review wait)

**Total:** 19 tasks, ~87 hours before risk buffer.

### Parallelizable Tasks

- AMO listing copy can start after the MV2/MV3 decision.
- Shared extraction fixture testing can run while manifest generation is implemented.
- Package inspector can be adapted from the Chrome inspector while runtime testing proceeds.
- Diagnostics schema review can run in parallel with Firefox toolbar QA.

### Must Be Sequential

- MV2/MV3 decision before final manifest implementation.
- Browser API adapter before migrating runtime code.
- Firefox target build before package inspection.
- Runtime capture QA before AMO submission.

### Critical Path Tasks

1. MV2/MV3 decision.
2. Browser API adapter.
3. Firefox manifest generation.
4. Loopback handshake.
5. Toolbar capture.
6. Menu-bar polling or fallback decision.
7. Package inspection.
8. AMO validation.

---

**End of PRD**

This PRD is structured so an extension engineer can evaluate the Firefox port without rediscovering Starlee's Chrome assumptions, local bridge contract, privacy constraints, package requirements, or menu-bar polling risks.
