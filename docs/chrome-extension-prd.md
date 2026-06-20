# PRD: Starlee Capture Chrome Extension

**Author:** Christian Karren  
**Date:** 2026-06-19  
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

Starlee needs a real Chrome extension, not only an unpacked developer extension, so users can install one trusted browser component and save the article they are reading into their local Starlee knowledge base. This PRD specifies a Manifest V3 Chrome Web Store extension that extracts rendered article text, metadata, selected text, and YouTube transcripts from the active tab, then sends the payload only to the local Starlee service at `127.0.0.1`.

The expected impact is a dramatic reduction in capture friction: after onboarding, the user clicks either the browser toolbar button or the Starlee Mac menu-bar button, and the current article is saved, indexed, and searchable in `~/Starlee`. The extension must preserve Starlee's core promise: private, local-first capture with no external upload of article bodies, vault data, capture tokens, or browsing history.

---

## Problem Statement

### Current Situation

Starlee currently includes Chromium extension source files under `sensor/`, and the installer can generate an unpacked extension folder at `~/Starlee/sensor-extension`. That proves the extraction path works for a technical user, but it is not a finished user experience:

- Users must open `chrome://extensions`, enable Developer Mode, load an unpacked folder, and understand why this is safe.
- There is no Chrome Web Store listing, review-ready package, privacy disclosure, extension icon set, screenshot set, or release channel.
- The extension connection state is visible to `starlee doctor`, but not yet obvious to normal users during onboarding.
- Broad page-access permissions can scare users if the product does not clearly explain that captured text stays local.
- One-click menu-bar capture depends on the extension being installed, connected, and able to read the active tab.

### User Impact

- **Who is affected:** Mac users who read articles in Chrome, Arc, Brave, or other Chromium-family browsers and want to save public, logged-in, or paywalled pages into Starlee.
- **How they're affected:** They can use the current extension only if they are comfortable with developer-mode installation. Non-technical users are likely to abandon setup before the first successful article capture.
- **Severity:** High for activation and retention. Starlee's value compounds only when saving feels effortless during reading.

### Business Impact

- **Cost of problem:** Lower setup completion, lower capture frequency, higher support burden, and a product that feels unfinished despite a working local engine.
- **Opportunity cost:** Without a polished extension, Starlee cannot deliver the native one-click capture loop promised by the menu-bar app and Codex plugin.
- **Strategic importance:** The Chrome extension is the bridge between what the user can see in the browser and what Starlee can store locally. It is the most important browser capture surface for the MVP.

### Why Solve This Now?

The core primitives already exist: local capture endpoint, bearer-token authentication, article payload contract, YouTube transcript capture, extension handshake, and menu-bar capture request polling. Building the Chrome extension into a store-ready, privacy-reviewed product is now the shortest path from “developer prototype” to “normal user can install and use this.”

### Assumptions

- The MVP targets Chrome first, with compatibility expected for Arc, Brave, and Edge where Chrome Web Store extensions are supported.
- The extension communicates only with the local Starlee service at `http://127.0.0.1:47291`.
- The Starlee installer or onboarding flow can open the Chrome Web Store listing, but Chrome will still require explicit user installation and permission approval.
- The extension must comply with Manifest V3 and Chrome Web Store policies.
- The extension is useful only when the Starlee local app/service is installed, initialized, and running.

---

## Goals & Success Metrics

### Goal 1: Ship a Chrome Web Store-installable extension

- **Description:** Users can install Starlee Capture from a Chrome Web Store listing instead of loading an unpacked extension folder.
- **Metric:** Successful installation path for Chrome stable users.
- **Baseline:** 0% store-installable; current flow is unpacked developer mode only.
- **Target:** 100% of Chrome stable users can install from a listing or unlisted beta listing.
- **Timeframe:** MVP release.
- **Measurement Method:** Manual install verification on a clean Chrome profile plus Chrome Web Store dashboard status.

### Goal 2: Enable one-click capture from the active browser tab

- **Description:** A user reading an article can click the Starlee extension button or the Mac menu-bar button and save the active page.
- **Metric:** Number of user actions after opening an article.
- **Baseline:** 4-8 actions in the current unpacked/manual flow.
- **Target:** 1 click after onboarding for pages with granted permission; 2 clicks when Chrome asks for site access.
- **Timeframe:** MVP release.
- **Measurement Method:** Local event log with redacted result codes: `toolbar_clicked`, `menu_request_seen`, `extract_success`, `capture_saved`, `capture_failed`.

### Goal 3: Preserve local-first privacy

- **Description:** Captured article bodies and transcripts never leave the user's machine unless the user explicitly exports or shares them.
- **Metric:** External network destinations receiving captured content.
- **Baseline:** 0 for current local endpoint design.
- **Target:** 0 for the Chrome extension MVP.
- **Timeframe:** Continuous.
- **Measurement Method:** Code review, static audit of fetch destinations, and runtime network inspection during capture tests.

### Goal 4: Improve rendered-page capture reliability

- **Description:** The extension captures readable article text and useful metadata from public, logged-in, and dynamically rendered pages visible to the user.
- **Metric:** Successful capture rate across a fixture set of representative pages.
- **Baseline:** Current prototype has working extraction but no formal browser fixture gate.
- **Target:** ≥90% success across the MVP fixture suite; ≥95% of failures produce a clear recovery message.
- **Timeframe:** Within 30 days of MVP release.
- **Measurement Method:** Playwright or Chrome-extension integration tests using local fixture pages and selected real-world manual smoke tests.

---

## User Stories

### Story 1: Install the extension from Chrome Web Store

**As a** Starlee user,  
**I want to** install the browser sensor from the Chrome Web Store,  
**So that I can** trust the extension and avoid developer-mode setup.

**Acceptance Criteria:**

- [ ] Chrome Web Store package includes `manifest.json`, service worker, content/extraction scripts, options page, icons, screenshots, and privacy disclosures.
- [ ] Listing explains that Starlee sends captures only to the local service at `127.0.0.1`.
- [ ] Extension installation requires the minimum permissions needed for active-tab capture, local service communication, storage, and menu-bar bridge polling.
- [ ] Store build contains no local vault data, config files, capture token, build cache, or model files.
- [ ] User can install on a clean Chrome stable profile without enabling Developer Mode.
- [ ] Extension version matches the release version recorded in the Starlee repository.

**Task Breakdown Hint:**

- Task 1.1: Audit extension permissions and reduce scope where practical (~6h)
- Task 1.2: Add icons and store listing assets (~6h)
- Task 1.3: Add privacy disclosure and single-purpose statement (~4h)
- Task 1.4: Build reproducible ZIP packaging script (~6h)
- Task 1.5: Submit unlisted beta to Chrome Web Store (~4h plus review wait)

**Dependencies:** Existing `sensor/extension` assets and Starlee local service.

---

### Story 2: Connect extension to local Starlee

**As a** user who installed the extension,  
**I want to** know whether it can reach Starlee,  
**So that I can fix setup before trying to save articles.

**Acceptance Criteria:**

- [ ] Extension attempts a handshake with `POST http://127.0.0.1:47291/extension/hello`.
- [ ] Options page shows one of: connected, Starlee not running, token missing, token invalid, unsupported service version, or blocked by browser permission.
- [ ] Extension never displays or logs the full capture token.
- [ ] If the local app is not running, options page instructs the user to open Starlee or run `starlee serve`.
- [ ] If the token is missing, options page offers a safe local setup path without printing the token to web pages.
- [ ] `starlee doctor` reports whether the extension has recently completed a handshake.

**Task Breakdown Hint:**

- Task 2.1: Formalize handshake response schema and version checks (~4h)
- Task 2.2: Improve options page connection-state UI (~8h)
- Task 2.3: Add redacted diagnostic events to local service (~6h)
- Task 2.4: Add tests for token missing/invalid/service down states (~6h)

**Dependencies:** Local capture service and extension storage.

---

### Story 3: Save an article from the toolbar button

**As a** reader,  
**I want to** click the Starlee toolbar button on the article I am reading,  
**So that** Starlee saves the rendered article body and metadata to my local knowledge base.

**Acceptance Criteria:**

- [ ] Toolbar click captures the active tab only after Chrome grants active-tab access.
- [ ] Extension extracts title, URL, site, byline when available, publication date when available, readable article text, selected text when available, and relevant HTML metadata.
- [ ] Extension classifies access conservatively, defaulting ambiguous pages to `restricted`.
- [ ] Extension posts payload to `POST http://127.0.0.1:47291/capture` with bearer authentication.
- [ ] Successful capture produces visible feedback within 2 seconds for payloads under 2 MiB.
- [ ] Capture failure gives a specific reason and one next action.

**Task Breakdown Hint:**

- Task 3.1: Harden active-tab capture flow (~8h)
- Task 3.2: Improve article extraction and metadata normalization (~12h)
- Task 3.3: Add success/failure badge or notification states (~6h)
- Task 3.4: Add fixture-based tests for article extraction (~10h)

**Dependencies:** Capture payload contract and local endpoint.

---

### Story 4: Save an article from the Mac menu bar

**As a** user who prefers one native button,  
**I want to** click the Starlee Mac menu-bar button,  
**So that** the Chrome extension captures my active article without me switching to the extension toolbar.

**Acceptance Criteria:**

- [ ] Mac app creates a local pending capture request with `POST /capture-request`.
- [ ] Extension polls `GET /capture-request` at a low-frequency interval while Chrome is open.
- [ ] When a request exists, extension captures the active tab and posts the standard article payload to `/capture`.
- [ ] Extension acknowledges or clears the pending request so duplicate saves are avoided.
- [ ] Menu-bar app reports success, extension unavailable, Chrome unavailable, page permission missing, extraction empty, service unreachable, or token invalid.
- [ ] Polling consumes negligible CPU and does not transmit browsing data when idle.

**Task Breakdown Hint:**

- Task 4.1: Define capture-request lifecycle and duplicate prevention (~6h)
- Task 4.2: Harden extension polling and wake-up behavior under Manifest V3 service worker lifecycle (~10h)
- Task 4.3: Connect result states to Mac notifications/menu UI (~8h)
- Task 4.4: Add end-to-end manual verification checklist (~4h)

**Dependencies:** macOS app, local service, extension background worker.

---

### Story 5: Save a YouTube transcript

**As a** user watching a YouTube video,  
**I want to** save the transcript with timestamps,  
**So that** Starlee can later answer questions with moment-level provenance.

**Acceptance Criteria:**

- [ ] Extension detects YouTube watch pages.
- [ ] Extension captures title, channel, URL, and transcript segments when available.
- [ ] Transcript segments are sent as `{ "t": seconds, "text": segment }`.
- [ ] If transcript is unavailable, Starlee still saves a useful note with `[Transcript unavailable]`.
- [ ] Capture does not bypass YouTube access controls or use unsupported scraping methods.
- [ ] Fixture tests cover transcript available, transcript unavailable, and non-watch YouTube pages.

**Task Breakdown Hint:**

- Task 5.1: Audit current YouTube extraction flow (~6h)
- Task 5.2: Add transcript result states and fallback copy (~6h)
- Task 5.3: Add YouTube fixture tests (~8h)

**Dependencies:** Existing YouTube transcript extraction tests and capture renderer.

---

## Functional Requirements

### Must Have (P0) - Critical for Launch

#### REQ-001: Manifest V3 Chrome Extension Package

**Description:** Starlee Capture must ship as a valid Manifest V3 Chrome extension package suitable for Chrome Web Store review.

**Acceptance Criteria:**

- [ ] `manifest.json` uses `manifest_version: 3`.
- [ ] Background logic runs through a service worker.
- [ ] Extension package builds reproducibly from repository source.
- [ ] Package excludes local config, vault data, generated model files, build caches, and secrets.
- [ ] Store ZIP validates on a clean checkout.

**Technical Specification:**

```json
{
  "manifest_version": 3,
  "name": "Starlee Capture",
  "description": "Save rendered articles and YouTube transcripts to your local Starlee brain.",
  "permissions": ["storage", "activeTab"],
  "host_permissions": ["http://127.0.0.1/*"]
}
```

The final permission list may include `tabs`, `alarms`, or `scripting` only if implementation tests prove they are necessary.

**Task Breakdown:**

- Audit current manifest permissions: Small (3h)
- Update package build script: Medium (5h)
- Add package exclusion tests: Small (3h)

**Dependencies:** Existing `sensor/extension/manifest.json`.

---

#### REQ-002: Local Service Handshake

**Description:** Extension must verify that the local Starlee service is available and compatible before claiming setup success.

**Acceptance Criteria:**

- [ ] Sends `POST /extension/hello` with extension version and browser name.
- [ ] Handles service down, invalid token, outdated service, CORS failure, and unexpected response.
- [ ] Stores only non-sensitive connection state.
- [ ] Updates options page status within 5 seconds of opening the page.

**Technical Specification:**

```http
POST http://127.0.0.1:47291/extension/hello
Authorization: Bearer <local token>
Content-Type: application/json
```

```json
{
  "browser": "Chrome",
  "extension_version": "0.1.0",
  "can_capture_active_tab": true
}
```

**Task Breakdown:**

- Define typed handshake result: Small (3h)
- Implement retry/backoff: Medium (5h)
- Add options-page diagnostics: Medium (6h)
- Test all failure cases: Medium (6h)

**Dependencies:** Local capture service.

---

#### REQ-003: Toolbar Article Capture

**Description:** User can click the extension toolbar icon to capture the active tab's rendered article content.

**Acceptance Criteria:**

- [ ] Captures only the active tab for the current user action.
- [ ] Extracts readable body text from rendered DOM.
- [ ] Extracts title, URL, site, byline, publication date, selected text, and meta tags when present.
- [ ] Sends payload matching `docs/capture-payload.md`.
- [ ] Shows success or failure feedback.

**Technical Specification:**

```typescript
interface StarleeArticlePayload {
  version: 1;
  type: "article";
  url: string;
  access: "public" | "restricted";
  dom_extract: {
    title: string;
    byline?: string | null;
    site?: string | null;
    published_at?: string | null;
    text: string;
    summary?: string | null;
    html_meta: Record<string, string>;
    selected_text?: string | null;
  };
  tags: string[];
}
```

**Task Breakdown:**

- Normalize extraction schema: Medium (6h)
- Improve readable text extraction: Medium (8h)
- Add toolbar result UI: Medium (6h)
- Add extraction fixture tests: Medium (8h)

**Dependencies:** REQ-002.

---

#### REQ-004: Menu-Bar Capture Bridge

**Description:** Extension must respond to capture requests created by the Starlee Mac menu-bar app.

**Acceptance Criteria:**

- [ ] Polls local `/capture-request` endpoint at a configurable low-frequency interval.
- [ ] Captures active tab when request source is `menu-bar`.
- [ ] Posts normal article or YouTube payload to `/capture`.
- [ ] Prevents duplicate processing of the same request.
- [ ] Reports result state back to local service.

**Technical Specification:**

```http
GET http://127.0.0.1:47291/capture-request
Authorization: Bearer <local token>
```

```json
{
  "request_id": "uuid",
  "source": "menu-bar",
  "created_at": "2026-06-19T16:00:00Z"
}
```

**Task Breakdown:**

- Add request ID handling: Medium (5h)
- Harden polling under service-worker suspension: Medium (8h)
- Implement result acknowledgment: Medium (6h)
- Test duplicate prevention: Small (3h)

**Dependencies:** Mac app and local service capture-request endpoints.

---

#### REQ-005: YouTube Transcript Capture

**Description:** Extension must capture YouTube metadata and transcript segments when available.

**Acceptance Criteria:**

- [ ] Detects YouTube watch pages reliably.
- [ ] Captures title, channel, URL, and transcript segments.
- [ ] Saves fallback item when transcript is unavailable.
- [ ] Does not send transcript or video metadata to external services.
- [ ] Tests cover available, unavailable, and malformed transcript states.

**Technical Specification:**

```typescript
interface StarleeYouTubePayload {
  version: 1;
  type: "youtube";
  url: string;
  access: "public" | "restricted";
  dom_extract: {
    title: string;
    byline?: string | null;
    site: "youtube.com";
    text: string;
    html_meta: Record<string, string>;
  };
  transcript: Array<{ t: number; text: string }>;
}
```

**Task Breakdown:**

- Audit current transcript extractor: Small (4h)
- Add YouTube result states: Medium (5h)
- Add fixture coverage: Medium (8h)

**Dependencies:** REQ-003.

---

#### REQ-006: Review-Ready Store Assets and Disclosures

**Description:** Extension release must include the assets and disclosures required for Chrome Web Store submission.

**Acceptance Criteria:**

- [ ] Icons exist at required sizes: 16, 32, 48, and 128 px.
- [ ] Store listing includes short description, full description, screenshots, category, support contact, and privacy policy URL or page.
- [ ] Privacy disclosure states that article text is sent only to a local service on the user's computer.
- [ ] Permission justification explains active tab access, local host access, storage, and any additional permissions.
- [ ] Release checklist includes manual review of package contents before upload.

**Technical Specification:**

Store copy must use plain language:

> Starlee Capture reads the page you choose to save and sends it to the Starlee app running locally on your computer. It does not upload article text to Starlee servers.

**Task Breakdown:**

- Create icon set and screenshots: Medium (6h)
- Draft listing copy and privacy disclosure: Medium (5h)
- Add package inspection script: Small (3h)
- Submit beta listing: Medium (4h plus review wait)

**Dependencies:** REQ-001.

---

### Should Have (P1) - Important but Not Blocking

#### REQ-007: Guided Options Page

**Description:** Options page should guide users through connecting the extension to the local Starlee service.

**Acceptance Criteria:**

- [ ] Shows connection status and browser support.
- [ ] Explains how to start Starlee if the local service is down.
- [ ] Shows last successful handshake time.
- [ ] Provides a test capture button for a local fixture page.

**Task Breakdown:**

- Build options UI states: Medium (8h)
- Add local fixture test page: Small (4h)
- Add user-facing recovery copy: Small (3h)

**Dependencies:** REQ-002.

---

#### REQ-008: Permission Minimization Mode

**Description:** Extension should prefer permission patterns that avoid broad “read all data” prompts when feasible.

**Acceptance Criteria:**

- [ ] Engineering evaluates `activeTab` plus programmatic injection as an alternative to broad persistent content scripts.
- [ ] If broad host access is retained, PR explains why menu-bar capture requires it.
- [ ] Permission copy is reflected in store listing.
- [ ] Automated tests cover whichever permission model is selected.

**Task Breakdown:**

- Prototype programmatic injection path: Medium (8h)
- Compare reliability against current content script: Medium (6h)
- Document permission decision: Small (2h)

**Dependencies:** REQ-003 and REQ-004.

---

#### REQ-009: Better In-Browser Feedback

**Description:** Extension should give clear visual feedback after capture.

**Acceptance Criteria:**

- [ ] Toolbar badge or notification shows saving, saved, and failed states.
- [ ] Failure copy names the likely issue.
- [ ] User can open Starlee recent captures from the extension when local app supports it.

**Task Breakdown:**

- Add action badge states: Small (4h)
- Add error-to-copy mapping: Small (4h)
- Add recent-captures link: Medium (5h)

**Dependencies:** REQ-003.

---

### Nice to Have (P2) - Future Enhancement

#### REQ-010: Cross-Browser Packaging

**Description:** Extension should be packaged for Edge, Brave, and Arc compatibility guidance after Chrome MVP.

**Acceptance Criteria:**

- [ ] Document installation behavior for Arc, Brave, and Edge.
- [ ] Confirm local service handshake in each browser.
- [ ] Add known limitations table to README.

**Task Breakdown:**

- Manual compatibility pass: Medium (8h)
- Add browser-specific docs: Small (3h)
- Add bug backlog items: Small (2h)

**Dependencies:** Chrome MVP approved.

---

## Non-Functional Requirements

### Performance

**Response Time:**

- Toolbar click to visible success/failure: <2 seconds p95 for payloads under 2 MiB.
- Extension handshake status on options page: <5 seconds p95.
- Article extraction execution in tab context: <500 ms p95 on fixture pages under 100k DOM nodes.

**Throughput:**

- One capture at a time per browser profile for MVP.
- Duplicate menu-bar requests must not create duplicate notes.

**Resource Usage:**

- Idle polling for menu-bar requests: ≤4 local requests per minute unless Chrome alarms impose a different minimum interval.
- Idle CPU use: not perceptible in Chrome Task Manager during a 10-minute observation.
- Extension package target: <2 MB excluding screenshots for store listing.

---

### Security

**Authentication:**

- All write endpoints use bearer token authentication.
- Token is stored only in extension local storage or Chrome storage.
- Token is never printed to console logs, DOM, notifications, or store assets.

**Authorization:**

- Extension captures only after user installation and Chrome-granted page access.
- Starlee must not bypass paywalls, browser permissions, site access controls, or logged-in boundaries.

**Data Protection:**

- Article bodies, transcripts, selected text, and metadata are sent only to `127.0.0.1` during capture.
- No remote scripts, remote code execution, telemetry endpoints, analytics SDKs, or external content-processing APIs are allowed in MVP.
- Errors include result codes but not captured article bodies.

**Compliance:**

- Chrome Web Store privacy practices and permission justifications must be completed before submission.
- Privacy disclosure must match actual network behavior verified in code review.

---

### Reliability

**Availability:**

- Extension gracefully handles local service not running.
- Extension gracefully handles service worker suspension and restart.
- Capture request polling resumes after browser restart.

**Error Handling:**

- Error categories: `service_down`, `token_missing`, `token_invalid`, `permission_denied`, `unsupported_page`, `empty_extract`, `payload_too_large`, `capture_failed`, `unknown_error`.
- User-facing error copy must include one next action.
- Developer logs must be useful without containing secrets or article bodies.

**Monitoring:**

- Local-only diagnostic events may be written to Starlee's setup state.
- No external telemetry is permitted in MVP.

---

### Accessibility

**Standards:**

- Options page and popup UI must be keyboard navigable.
- Buttons and status indicators must have accessible names.
- Color contrast target: WCAG 2.1 AA for text and controls.

**Testing:**

- Manual keyboard-only pass for options page.
- Automated static checks where practical.

---

### Compatibility

**Browsers:**

- MVP required: Chrome stable on macOS.
- Expected but not launch-blocking: Arc, Brave, and Edge using Chromium extension support.

**Operating Systems:**

- MVP required: macOS with Starlee local service and menu-bar app installed.
- Future: Windows and Linux extension use with `starlee serve`, without native menu-bar integration.

**Starlee Versions:**

- Extension must declare compatible local service versions.
- If service version is too old, options page tells the user to update Starlee.

---

## Technical Considerations

### System Architecture

**Current Architecture:**

Starlee has a Rust CLI and local service, a local vault at `~/Starlee`, a Markdown capture pipeline, SQLite FTS5 + sqlite-vec indexing, Codex MCP tools, a macOS menu-bar app, and prototype Chromium extension assets.

**Proposed Changes:**

The Chrome extension becomes a first-class release artifact. It remains a thin local sensor: it reads the current page only when requested, extracts content and metadata, and sends the payload to the local Starlee service.

**Diagram:**

```text
┌─────────────────────┐
│ User reading article │
└──────────┬──────────┘
           │ click toolbar or Mac menu-bar button
           v
┌─────────────────────┐      local HTTP       ┌──────────────────────┐
│ Starlee Chrome Ext   │ ───────────────────> │ Starlee local service │
│ MV3 service worker   │                      │ 127.0.0.1:47291       │
│ page extractor       │ <─────────────────── │ capture-request state │
└──────────┬──────────┘      handshake        └──────────┬───────────┘
           │                                             │
           │ rendered DOM payload                         │ write + index
           v                                             v
┌─────────────────────┐                      ┌──────────────────────┐
│ Active browser tab   │                      │ ~/Starlee vault/index │
└─────────────────────┘                      └──────────────────────┘
```

**Key Components:**

1. **Manifest V3 service worker:** Handles toolbar clicks, handshake, polling, and local capture requests.
2. **Page extraction script:** Reads rendered DOM, metadata, selection, and YouTube transcript state.
3. **Options page:** Shows connection status and setup recovery steps.
4. **Local Starlee service:** Authenticates extension requests and stores captures.
5. **Mac menu-bar app:** Creates pending capture requests for the extension to process.

---

### API Specifications

#### Endpoint: Extension handshake

```http
POST /extension/hello
Host: 127.0.0.1:47291
Authorization: Bearer {capture_token}
Content-Type: application/json
```

```json
{
  "browser": "Chrome",
  "extension_version": "0.1.0",
  "can_capture_active_tab": true
}
```

**Response:**

```json
{
  "ok": true,
  "service_version": "0.1.0",
  "vault_ready": true
}
```

#### Endpoint: Capture rendered page

```http
POST /capture
Host: 127.0.0.1:47291
Authorization: Bearer {capture_token}
Content-Type: application/json
```

Body follows `docs/capture-payload.md`.

#### Endpoint: Poll menu-bar request

```http
GET /capture-request
Host: 127.0.0.1:47291
Authorization: Bearer {capture_token}
```

**Response when request exists:**

```json
{
  "request_id": "uuid",
  "source": "menu-bar",
  "created_at": "2026-06-19T16:00:00Z"
}
```

---

### Storage Schema

**Chrome Storage Keys:**

```typescript
interface StarleeExtensionStorage {
  captureToken?: string;
  servicePort: number; // default 47291
  lastHandshakeAt?: string;
  lastHandshakeStatus?: "connected" | "service_down" | "token_missing" | "token_invalid" | "version_mismatch";
  lastCaptureStatus?: "saved" | "failed";
  lastCaptureError?: string;
}
```

Sensitive values:

- `captureToken` must never be logged or rendered after entry.
- No article bodies or transcripts should be persisted in Chrome storage.

---

### Technology Stack

**Extension:**

- Manifest V3
- JavaScript service worker
- HTML/CSS options page
- Existing sensor test stack under `sensor/`

**Local Service:**

- Rust Starlee CLI/service
- Loopback HTTP on `127.0.0.1:47291`
- Markdown vault and SQLite index

**Testing:**

- Existing `npm test --prefix sensor`
- Fixture-based extraction tests
- Manual Chrome stable smoke tests
- Optional Playwright/Chrome extension harness for end-to-end tests

---

### External Dependencies

**Third-Party Services:**

1. **Chrome Web Store**
   - Purpose: Distribute extension to normal users.
   - Failure handling: If review is delayed, support an unlisted beta or unpacked dev build for testers.

**Internal Dependencies:**

- Starlee local service must be installed and running.
- Starlee Mac app is required for menu-bar initiated capture.
- Starlee CLI `doctor` should verify handshake state.

---

### Migration Strategy

1. **Phase 1: Keep unpacked extension for development**
   - Existing `~/Starlee/sensor-extension` remains supported for local testing.
   - Tests continue to run against source assets.

2. **Phase 2: Add store package pipeline**
   - Generate `release/starlee-capture-chrome.zip`.
   - Inspect package contents before upload.

3. **Phase 3: Submit unlisted beta**
   - Use unlisted Chrome Web Store distribution for testers.
   - Verify install, handshake, toolbar capture, menu-bar capture, and update path.

4. **Phase 4: Public listing**
   - Promote to public listing after review, privacy copy validation, and fixture reliability targets.

**Rollback Plan:**

- Remove or pause listing if privacy, permission, or capture bugs are found.
- Keep local unpacked extension path available for development.
- Version-gate incompatible extension/service pairs.

---

### Testing Strategy

**Unit Tests:**

- Access classification
- Metadata normalization
- Payload shape validation
- Error-code mapping
- YouTube transcript parsing

**Integration Tests:**

- Toolbar capture to mocked local service
- Handshake success and failure states
- Menu-bar capture polling
- Token missing/invalid behavior

**E2E Tests:**

- Clean Chrome profile installs extension from ZIP or store beta.
- Starlee service running: article capture succeeds.
- Starlee service stopped: extension shows clear recovery message.
- Mac menu-bar request triggers extension capture.
- YouTube transcript page saves timestamped segments.

**Security Tests:**

- Package inspection for secrets and local data.
- Network audit confirms no external article-body upload.
- Console audit confirms token is not logged.
- Permission audit documents why each permission exists.

---

## Implementation Roadmap

### Phase 1: Extension Audit and Permission Decision (Week 1)

**Goal:** Convert the existing prototype into a clear technical plan for store submission.

**Tasks:**

- [ ] Task 1.1: Audit current manifest permissions (REQ-001, REQ-008)
  - Complexity: Small (3h)
  - Dependencies: None
  - Owner: Extension engineer

- [ ] Task 1.2: Decide between persistent content script and activeTab/programmatic injection (REQ-003, REQ-008)
  - Complexity: Medium (8h)
  - Dependencies: Task 1.1
  - Owner: Extension engineer

- [ ] Task 1.3: Document final permission rationale for store listing (REQ-006)
  - Complexity: Small (3h)
  - Dependencies: Task 1.2
  - Owner: Product/engineering

**Validation Checkpoint:** Permission model is documented, tested locally, and ready for implementation.

---

### Phase 2: Connection and Capture Hardening (Week 1-2)

**Goal:** Make toolbar capture, handshake, and error states reliable.

**Tasks:**

- [ ] Task 2.1: Improve handshake schema and options status UI (REQ-002, REQ-007)
  - Complexity: Medium (8h)
  - Dependencies: Phase 1
  - Owner: Extension engineer

- [ ] Task 2.2: Harden toolbar article capture and payload validation (REQ-003)
  - Complexity: Large (12h)
  - Dependencies: Task 2.1
  - Owner: Extension engineer

- [ ] Task 2.3: Harden YouTube transcript capture (REQ-005)
  - Complexity: Medium (8h)
  - Dependencies: Task 2.2
  - Owner: Extension engineer

- [ ] Task 2.4: Add visible capture feedback states (REQ-009)
  - Complexity: Medium (6h)
  - Dependencies: Task 2.2
  - Owner: Extension engineer

**Validation Checkpoint:** Toolbar capture works across fixture pages and all expected failure states.

---

### Phase 3: Menu-Bar Bridge Reliability (Week 2)

**Goal:** Ensure the Mac menu-bar app can trigger capture through Chrome.

**Tasks:**

- [ ] Task 3.1: Add request IDs and duplicate prevention to capture-request flow (REQ-004)
  - Complexity: Medium (6h)
  - Dependencies: Phase 2
  - Owner: Full-stack engineer

- [ ] Task 3.2: Harden polling under Manifest V3 worker lifecycle (REQ-004)
  - Complexity: Medium (8h)
  - Dependencies: Task 3.1
  - Owner: Extension engineer

- [ ] Task 3.3: Report bridge result states to local service and Mac app (REQ-004)
  - Complexity: Medium (8h)
  - Dependencies: Task 3.1
  - Owner: Full-stack engineer

**Validation Checkpoint:** Clicking the Mac menu-bar button saves the current Chrome article on a clean machine.

---

### Phase 4: Store Packaging and Privacy Review (Week 3)

**Goal:** Produce a review-ready Chrome Web Store package.

**Tasks:**

- [ ] Task 4.1: Add reproducible package script for Chrome ZIP (REQ-001)
  - Complexity: Medium (6h)
  - Dependencies: Phase 2
  - Owner: Extension engineer

- [ ] Task 4.2: Create icons, screenshots, and listing copy (REQ-006)
  - Complexity: Medium (8h)
  - Dependencies: Task 4.1
  - Owner: Product/design

- [ ] Task 4.3: Write privacy disclosure and permission justifications (REQ-006)
  - Complexity: Medium (5h)
  - Dependencies: Task 4.2
  - Owner: Product/engineering

- [ ] Task 4.4: Run package inspection and network privacy audit (REQ-006)
  - Complexity: Medium (6h)
  - Dependencies: Task 4.1
  - Owner: Engineering

**Validation Checkpoint:** Package is safe to upload and disclosures match behavior.

---

### Phase 5: Beta Submission and Launch (Week 4)

**Goal:** Validate the extension through a Chrome Web Store beta, then launch.

**Tasks:**

- [ ] Task 5.1: Submit unlisted beta to Chrome Web Store
  - Complexity: Small (4h plus review wait)
  - Dependencies: Phase 4
  - Owner: Release owner

- [ ] Task 5.2: Run clean-profile install test after approval
  - Complexity: Medium (4h)
  - Dependencies: Task 5.1
  - Owner: QA/release owner

- [ ] Task 5.3: Run full capture smoke: toolbar article, menu-bar article, YouTube transcript
  - Complexity: Medium (6h)
  - Dependencies: Task 5.2
  - Owner: QA/release owner

- [ ] Task 5.4: Promote listing to public after beta signoff
  - Complexity: Small (2h)
  - Dependencies: Task 5.3
  - Owner: Release owner

**Validation Checkpoint:** Public or unlisted listing installs cleanly and captures reliably.

---

### Task Dependencies Visualization

```text
Phase 1:
  1.1 Permission Audit → 1.2 Permission Model → 1.3 Store Rationale

Phase 2:
  Phase 1 → 2.1 Handshake UI → 2.2 Toolbar Capture → 2.3 YouTube Capture
                                      └──────────────→ 2.4 Feedback States

Phase 3:
  Phase 2 → 3.1 Request IDs → 3.2 MV3 Polling → 3.3 Result Reporting

Phase 4:
  Phase 2 → 4.1 Package Script → 4.2 Store Assets → 4.3 Privacy Copy
                                  └──────────────→ 4.4 Privacy Audit

Phase 5:
  Phase 4 + Phase 3 → 5.1 Beta Submit → 5.2 Clean Install → 5.3 Smoke Tests → 5.4 Public Launch

Critical Path:
  Permission Model → Toolbar Capture → Menu-Bar Bridge → Package Script → Privacy Audit → Beta Submit → Smoke Tests
```

---

### Effort Estimation

**Total Estimated Effort:**

- Phase 1: 14 hours
- Phase 2: 34 hours
- Phase 3: 22 hours
- Phase 4: 25 hours
- Phase 5: 16 hours plus Chrome Web Store review time
- **Total:** ~111 engineering/product hours plus external review wait

**Risk Buffer:** +25% for Manifest V3 lifecycle issues and Chrome Web Store review feedback  
**Final Estimate:** ~139 hours, or about 3-4 weeks for one focused engineer plus part-time product/design/release support.

---

## Out of Scope

Explicitly NOT included in this release:

1. **Safari extension release**
   - Reason: Different packaging and App Store/Safari Web Extension flow.
   - Future: Covered by separate Safari/macOS PRD.

2. **Firefox extension release**
   - Reason: Different extension APIs and review process.
   - Future: Consider after Chrome MVP proves the capture loop.

3. **Automatic browser extension installation**
   - Reason: Chrome requires explicit user installation and permission approval.
   - Future: Installer can open the store listing and verify connection, but cannot safely bypass user consent.

4. **Cloud sync or remote article processing**
   - Reason: Violates MVP local-first guarantee.
   - Future: Only if user explicitly opts in.

5. **Paywall bypassing**
   - Reason: Starlee captures what the user can already see in the browser; it must not bypass access controls.

6. **Full knowledge graph UI**
   - Reason: This PRD covers browser capture only.
   - Future: Use captured data in Starlee search, MCP retrieval, graph views, and share bundles.

---

## Open Questions & Risks

### Open Questions

#### Q1: Should MVP use broad content scripts or activeTab plus programmatic injection?

- **Current Status:** Existing prototype uses content scripts on HTTP/HTTPS pages.
- **Options:** (A) Keep content scripts for reliability and menu-bar bridge, (B) switch to `activeTab` plus `scripting`, (C) hybrid with optional host permissions.
- **Owner:** Extension engineer.
- **Deadline:** End of Phase 1.
- **Impact:** High. This affects Chrome permission prompts, store review, user trust, and menu-bar capture reliability.

#### Q2: How should the extension receive the capture token?

- **Current Status:** Existing local setup can generate config for unpacked extension; store extension needs a clean token-entry or local pairing flow.
- **Options:** (A) User enters token in options, (B) local pairing page opens from Starlee app, (C) temporary one-time pairing code.
- **Owner:** Product/engineering.
- **Deadline:** Before Phase 2 completion.
- **Impact:** High. Token setup is the main onboarding friction after installation.

#### Q3: What is the first release channel?

- **Current Status:** No Chrome Web Store listing exists.
- **Options:** (A) Unlisted beta first, (B) public listing immediately, (C) private test group.
- **Owner:** Release owner.
- **Deadline:** Start of Phase 4.
- **Impact:** Medium. Affects user access and review speed.

---

### Risks & Mitigation

| Risk | Likelihood | Impact | Severity | Mitigation | Contingency |
|------|------------|--------|----------|------------|-------------|
| Chrome Web Store review rejects broad permissions | Medium | High | High | Minimize permissions and provide precise justifications | Ship unlisted/dev build while revising permissions |
| Manifest V3 service worker sleeps before menu-bar polling | Medium | High | High | Use alarms and lifecycle-aware polling tests | Menu-bar app opens extension action as fallback |
| Users distrust page-access prompt | High | High | Critical | Plain-language privacy copy and local-only network proof | Offer toolbar-only activeTab mode if feasible |
| Token setup feels too technical | Medium | High | High | Build local pairing flow with redacted diagnostics | Options page manual token entry as fallback |
| Extraction fails on dynamic article pages | Medium | Medium | Medium | Fixture tests and fallback selected-text capture | Save URL + selected text with clear warning |
| Captured content accidentally logged | Low | Critical | High | Log redaction tests and code review | Emergency patch and extension version rollback |

---

## Validation Checkpoints

### Checkpoint 1: End of Phase 1

**Criteria:**

- [ ] Permission model chosen and documented.
- [ ] Prototype still captures active tab after permission changes.
- [ ] Store permission rationale drafted.

**If Failed:** Keep prototype flow, do not start store packaging until permission model is defensible.

---

### Checkpoint 2: End of Phase 2

**Criteria:**

- [ ] Toolbar capture works against fixture articles.
- [ ] YouTube transcript capture works against fixture pages.
- [ ] Options page shows accurate connected/disconnected states.
- [ ] Token is not logged or displayed.

**If Failed:** Fix capture and connection states before menu-bar bridge hardening.

---

### Checkpoint 3: End of Phase 3

**Criteria:**

- [ ] Mac menu-bar click saves the active Chrome article.
- [ ] Duplicate request test passes.
- [ ] Extension handles Chrome restart and Starlee service restart.

**If Failed:** Ship toolbar-first beta and mark menu-bar bridge experimental.

---

### Checkpoint 4: End of Phase 4

**Criteria:**

- [ ] Chrome ZIP builds reproducibly.
- [ ] Package inspection shows no secrets, vault files, local config, model files, or build cache.
- [ ] Network audit confirms no article-body upload outside `127.0.0.1`.
- [ ] Store listing, screenshots, icons, and privacy disclosures are ready.

**If Failed:** Do not upload package.

---

### Checkpoint 5: Beta or Public Launch

**Criteria:**

- [ ] Clean Chrome profile can install extension from listing.
- [ ] Extension handshakes with local Starlee.
- [ ] Toolbar article capture succeeds.
- [ ] Menu-bar article capture succeeds.
- [ ] YouTube transcript capture succeeds or gracefully saves transcript-unavailable fallback.
- [ ] `starlee doctor` reports recent extension handshake.

**If Failed:** Keep listing unlisted or paused until the failing launch path is fixed.

---

## Appendix: Task Breakdown Hints

### Suggested Task Structure

**Extension Package and Permissions (5 tasks, ~25 hours)**

1. Audit existing manifest permissions (3h)
2. Prototype activeTab/programmatic injection alternative (8h)
3. Finalize permission model and document rationale (3h)
4. Add reproducible package script (6h)
5. Add package inspection for secrets/build artifacts (5h)

**Connection and Onboarding (5 tasks, ~29 hours)**

6. Formalize handshake response schema (4h)
7. Improve options page status UI (8h)
8. Implement token pairing or safer token setup flow (10h)
9. Add redacted local diagnostics (4h)
10. Add service-down/token-invalid tests (3h)

**Capture Features (7 tasks, ~50 hours)**

11. Harden toolbar capture flow (8h)
12. Improve readable article extraction (10h)
13. Normalize metadata fields (5h)
14. Harden YouTube transcript extraction (8h)
15. Add selected-text support (6h)
16. Add visible feedback states (5h)
17. Add fixture-based extraction tests (8h)

**Menu-Bar Bridge (4 tasks, ~24 hours)**

18. Add capture request IDs (5h)
19. Add duplicate prevention and acknowledgment (5h)
20. Harden MV3 polling/lifecycle behavior (8h)
21. Connect result states to Mac app notifications (6h)

**Store Release (6 tasks, ~27 hours plus review wait)**

22. Create icons and screenshots (6h)
23. Draft short and long store descriptions (3h)
24. Write privacy disclosure and permission justifications (5h)
25. Run privacy/network audit (6h)
26. Submit unlisted beta (3h plus review wait)
27. Run clean-profile launch smoke tests (4h)

**Total:** 27 tasks, ~155 hours including risk-adjusted product/release work.

### Parallelizable Tasks

**Can work in parallel:**

- Store assets and privacy copy can start while capture hardening continues.
- YouTube tests can run in parallel with article extraction tests.
- Package inspection can be developed while options page UI is built.

**Must be sequential:**

- Permission decision before final store copy.
- Handshake/token setup before reliable capture tests.
- Package inspection before Chrome Web Store upload.
- Beta install verification before public launch.

### Critical Path Tasks

1. Permission model decision
2. Token setup/pairing flow
3. Toolbar capture reliability
4. Menu-bar bridge reliability
5. Reproducible package script
6. Privacy/network audit
7. Chrome Web Store beta approval
8. Clean-profile launch smoke tests

**Critical path duration:** ~75 focused hours plus Chrome Web Store review wait.

---

**End of PRD**

This PRD is structured so an engineering agent can break the Chrome extension into implementation tasks without rediscovering the product intent, privacy constraints, browser permission tradeoffs, or release gates.
