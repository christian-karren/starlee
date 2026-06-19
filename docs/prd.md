# PRD: Starlee Native Onboarding and One-Click Article Capture

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

Starlee currently has the right local-first engine but asks too much from the user during setup and daily capture: terminal commands, plugin registration, background service setup, and browser-extension configuration. This PRD specifies a macOS-first product experience where a user gives Codex the Starlee GitHub repo, says “Starlee, set yourself up,” completes one browser permission step, and then saves the article they are reading with one click from a Mac menu-bar button.

The proposed solution separates the system into four roles: Codex orchestrates installation, the Starlee macOS app provides the daily visible button, the browser extension extracts rendered article text and metadata, and the local Starlee service stores and indexes captures in `~/Starlee`. Expected impact is reducing first successful setup from a multi-command developer workflow to a guided flow under 5 minutes and reducing daily article capture from 4-8 manual actions to 1 click after onboarding.

---

## Problem Statement

### Current Situation

Starlee is implemented as a local-first digital brain with a Rust CLI, MCP server, local Markdown vault, SQLite FTS5 + sqlite-vec index, browser capture endpoint, and Codex plugin metadata. It can capture pasted notes, public URLs, rendered browser payloads, and YouTube transcripts. However, the current installation and capture experience still feels like a developer tool:

- The user must clone a repo, run an installer, verify a Codex plugin, start or trust a background service, and load a browser extension folder.
- The visible daily capture surface is a browser extension or page button, not a native Mac-level Starlee affordance.
- Users do not have a clear mental model for when URL fetch works versus when browser-rendered capture is required.
- Browser-extension installation is unavoidable for rendered article extraction, but the product has not framed that permission as a one-time trust step.

### User Impact

- **Who is affected:** Mac users who read articles, newsletters, research pages, YouTube transcripts, and logged-in web content and want those sources saved into a local knowledge base.
- **How they are affected:** Users hesitate or abandon setup when asked to run terminal commands or load unpacked browser extensions. After setup, users may forget which capture surface to use or why Starlee cannot fetch a paid/logged-in article by URL.
- **Severity:** High for adoption. The knowledge base only compounds if capture happens during reading. Any capture workflow above 2 clicks competes with “I’ll save this later,” which usually means the article is lost.

### Business Impact

- **Cost of problem:** Lower activation rate, lower repeat capture frequency, more support/debugging around plugin setup and browser tokens, and weaker perceived product polish.
- **Opportunity cost:** Starlee cannot occupy the “ambient personal knowledge tool” category if the primary interaction feels like a CLI. Competitors with native overlays, menu-bar apps, and calendar/browser integrations feel more present even when their underlying operations are ordinary OS primitives.
- **Strategic importance:** A native one-click capture loop makes Starlee feel like a daily companion rather than a developer utility. This is required before investing heavily in knowledge graph features, sharing, or recall UX.

### Why Solve This Now?

The technical foundation exists: local vault, index, MCP tools, authenticated loopback capture endpoint, browser sensor, and plugin packaging are already in place. The remaining gap is orchestration and UX. Solving this now converts Starlee from “works if configured” to “sets itself up with Codex and becomes present in the user’s Mac workflow.”

### Assumptions

- Initial release targets macOS because the desired daily surface is a Mac menu-bar app.
- Safari and Chromium-family browsers are first-class for MVP planning.
- The product may use Codex to assist installation, but must not depend on Codex Computer Use as the primary user experience.
- Browser-extension permission cannot and should not be bypassed. The user must explicitly install/enable page access.
- Captured article bodies remain local unless the user explicitly exports or shares them.

---

## Goals & Success Metrics

### Goal 1: Reduce first successful setup time

- **Description:** A new user can install Starlee from the GitHub repo through Codex and complete the required browser permission step without reading separate documentation.
- **Metric:** Median time from prompt submission to successful `GET /health` plus extension handshake.
- **Baseline:** 10-20 minutes for a technical user using the current manual flow.
- **Target:** <5 minutes median for a user with Codex installed and browser available.
- **Timeframe:** Within 30 days of MVP release.
- **Measurement Method:** Local setup timestamps, installer phase logs without secrets, and opt-in manual usability tests.

### Goal 2: Reduce daily article capture friction

- **Description:** After onboarding, a user can save the current readable article from the Mac menu bar.
- **Metric:** Number of user actions after opening an article.
- **Baseline:** 4-8 actions depending on extension/options/service state.
- **Target:** 1 click for the happy path; 2 clicks when browser permission requires host confirmation.
- **Timeframe:** At MVP release.
- **Measurement Method:** Instrument local event counts for button click, extension response, capture success, and fallback path, stored locally.

### Goal 3: Increase capture reliability for rendered pages

- **Description:** Starlee should capture rendered article body and metadata from pages the user can see in the active browser.
- **Metric:** Successful capture rate for supported browser/article combinations.
- **Baseline:** Current extension works when manually loaded/configured, but setup-dependent failures are common.
- **Target:** ≥90% success on supported public and logged-in article pages after extension is enabled; ≥95% clear recovery messaging for failures.
- **Timeframe:** Within 60 days of MVP release.
- **Measurement Method:** Local capture result codes, extension handshake checks, and test suite using fixture pages.

### Goal 4: Preserve local-first privacy guarantees

- **Description:** Daily capture must not upload article bodies, capture tokens, vault content, or model cache data to external services.
- **Metric:** Number of network destinations receiving captured article body during capture.
- **Baseline:** 0 for current local engine.
- **Target:** 0 for MVP and subsequent releases unless user explicitly exports/shares.
- **Timeframe:** Continuous.
- **Measurement Method:** Code review, privacy invariant tests, and local network audit during validation.

---

## User Stories

### Story 1: Codex-assisted setup

**As a** Mac user who has Codex installed,  
**I want to** paste the Starlee GitHub repo into Codex and say “Starlee, set yourself up,”  
**So that I can install Starlee without manually translating docs into terminal steps.

**Acceptance Criteria:**

- [ ] Codex can clone or update the Starlee repository from GitHub.
- [ ] Codex can run a single repository installer command.
- [ ] Installer registers Starlee as a Codex plugin and local MCP server through plugin packaging, not a separate ad-hoc MCP-only path.
- [ ] Installer initializes `~/Starlee`, starts the local capture service, and launches or installs the Mac menu-bar app.
- [ ] Installer does not print the capture token to Codex output.
- [ ] If a required permission cannot be automated, the user receives one sentence explaining what they must approve and why.

**Task Breakdown Hint:**

- Task 1.1: Harden `scripts/install.sh` into idempotent installer phases (~8h)
- Task 1.2: Add setup verification command returning redacted state (~6h)
- Task 1.3: Add Codex plugin install/reinstall docs and skill guidance (~4h)
- Task 1.4: Add automated setup tests using temporary home directories (~8h)

**Dependencies:** Existing Starlee CLI, plugin manifest, and macOS service installer.

---

### Story 2: Browser choice and extension handoff

**As a** user during setup,  
**I want to** choose Safari, Chrome, Arc, Brave, or another supported browser,  
**So that** Starlee opens the exact extension setup path I need.

**Acceptance Criteria:**

- [ ] Starlee detects installed browsers from known macOS application bundle IDs.
- [ ] User can override detected browser choice.
- [ ] Safari path opens the bundled Safari Web Extension enablement screen or clear instructions when direct deep-linking is unavailable.
- [ ] Chromium path opens the Chrome Web Store listing or local unpacked-extension page depending on release channel.
- [ ] Setup verifies extension-to-local-service communication before declaring onboarding complete.
- [ ] Unsupported browsers fall back to URL capture and selected-text capture instructions.

**Task Breakdown Hint:**

- Task 2.1: Implement browser detection in macOS app (~6h)
- Task 2.2: Build onboarding browser picker UI (~8h)
- Task 2.3: Add per-browser setup launchers and recovery copy (~8h)
- Task 2.4: Implement extension handshake verification (~8h)

**Dependencies:** REQ-003, REQ-006, browser extension packaging.

---

### Story 3: One-click menu-bar capture

**As a** reader with Starlee onboarded,  
**I want to** click a Starlee button in the Mac menu bar,  
**So that** the article currently open in my browser is saved to my local knowledge base.

**Acceptance Criteria:**

- [ ] Menu-bar button is visible while Starlee is running.
- [ ] Click triggers capture of the active browser tab through the installed extension when available.
- [ ] Extension extracts rendered article text, URL, title, site, author/byline, publication date when present, selected text when relevant, and source type.
- [ ] Local service writes Markdown to `~/Starlee/vault` and updates the search index.
- [ ] User receives a macOS notification within 2 seconds of successful capture for payloads under 2 MiB.
- [ ] If the extension is unavailable, Starlee falls back to active-tab URL capture for public pages or selected-text capture instructions.

**Task Breakdown Hint:**

- Task 3.1: Implement menu-bar capture action and status menu (~8h)
- Task 3.2: Implement active browser/tab resolver (~10h)
- Task 3.3: Implement app-to-extension capture request bridge (~16h)
- Task 3.4: Add notification result states (~6h)
- Task 3.5: Add end-to-end capture fixtures (~10h)

**Dependencies:** REQ-004, REQ-005, REQ-006, local capture endpoint.

---

### Story 4: Honest fallback for pages Starlee cannot capture directly

**As a** user reading a paywalled or logged-in page,  
**I want** Starlee to explain why URL-only capture is insufficient,  
**So that** I know whether to enable the extension, select text, or use another fallback.

**Acceptance Criteria:**

- [ ] URL-only capture remains disabled for ambiguous, metered, or known paid pages unless explicit public-access metadata is present.
- [ ] Failure messages distinguish extension unavailable, page permission denied, unreadable DOM, article extraction empty, service unreachable, and payload too large.
- [ ] User can choose selected-text capture when extension capture is unavailable.
- [ ] Recovery instructions fit in a notification plus one menu detail view.

**Task Breakdown Hint:**

- Task 4.1: Define error taxonomy and result codes (~4h)
- Task 4.2: Map engine, extension, and app failures to user-facing copy (~6h)
- Task 4.3: Add selected-text capture fallback using clipboard/selection with permission gating (~12h)
- Task 4.4: Add tests for paid/public/fallback routing (~8h)

**Dependencies:** Existing public URL fetch guardrails, macOS app UI.

---

## Functional Requirements

### Must Have (P0) - Required for MVP

#### REQ-001: Idempotent Codex setup command

**Priority:** P0

**Description:** Starlee must provide one repository command that Codex can run to install or update the Codex plugin, CLI, local service, menu-bar app, and browser setup assets.

**Acceptance Criteria:**

- [ ] Running the installer twice produces the same installed state without duplicate LaunchAgents, duplicate marketplace entries, or duplicate plugin config.
- [ ] Installer exits non-zero when build, plugin registration, service start, or extension asset generation fails.
- [ ] Installer output redacts tokens and local vault body content.
- [ ] Installer writes only approved local paths: repository build output, `~/.local/bin/starlee`, `~/Starlee`, `~/Library/LaunchAgents`, `~/plugins/starlee`, and `~/.agents/plugins/marketplace.json`.

**Technical Specification:**

```text
Command: ./scripts/install.sh
Phases:
1. Build CLI and browser assets.
2. Install binary to ~/.local/bin/starlee.
3. Initialize ~/Starlee.
4. Generate browser extension assets.
5. Install/update macOS LaunchAgent.
6. Install/update Codex plugin marketplace entry.
7. Launch menu-bar app.
8. Run redacted health checks.
```

**Task Breakdown:**

- Refactor installer phases with explicit result codes: Medium (6h)
- Add redacted `starlee doctor --json`: Medium (8h)
- Add idempotency tests with temporary HOME: Medium (8h)
- Document Codex prompt and setup flow: Small (3h)

**Dependencies:** Existing installer, plugin manifest, Rust CLI.

---

#### REQ-002: Native macOS menu-bar app as primary daily surface

**Priority:** P0

**Description:** Starlee must ship a signed or locally installable macOS menu-bar app that starts at login, shows service health, exposes “Save Current Article,” and displays capture results.

**Acceptance Criteria:**

- [ ] Menu-bar icon appears within 5 seconds after app launch.
- [ ] Menu contains Save Current Article, Recent Captures, Open Starlee Folder, Browser Setup, Service Status, and Quit.
- [ ] App can start or restart the local capture service when it is not running.
- [ ] App shows red/yellow/green service state based on local health and extension handshake.
- [ ] App can run without Codex open.

**Technical Specification:**

```text
App: Starlee.app
Runtime: Swift/AppKit
Service dependency: ~/.local/bin/starlee serve
Health endpoint: GET http://127.0.0.1:47291/health
Notification mechanism: UserNotifications
```

**Task Breakdown:**

- Expand Swift menu-bar app from package shell to functional UI: Large (16h)
- Add LaunchAgent/login item management: Medium (8h)
- Add service health polling every 15 seconds: Small (4h)
- Add notifications and result menu state: Medium (6h)

**Dependencies:** REQ-001, local capture endpoint.

---

#### REQ-003: Browser onboarding chooser

**Priority:** P0

**Description:** Starlee must guide the user through browser-specific extension setup after installation.

**Acceptance Criteria:**

- [ ] On first launch, Starlee displays detected supported browsers and asks the user to choose one.
- [ ] Browser choice can be changed later from the menu.
- [ ] Safari setup explains that a Safari Web Extension must be enabled by the user.
- [ ] Chromium setup opens the proper extension setup location for Chrome, Arc, Brave, or Edge.
- [ ] Setup does not claim completion until extension handshake succeeds or the user chooses a fallback-only mode.

**Technical Specification:**

```text
Supported MVP browsers:
- Safari via Safari Web Extension bundled with Starlee.app
- Chrome via Web Extension
- Arc/Brave/Edge via Chromium-compatible Web Extension where supported

Handshake:
Extension -> POST /extension/hello or existing /health-compatible route
App polls local service for last successful extension handshake timestamp.
```

**Task Breakdown:**

- Implement browser detection by bundle ID: Small (4h)
- Build onboarding UI screen/menu item: Medium (8h)
- Implement extension handshake route and local state: Medium (8h)
- Add per-browser copy and troubleshooting: Small (4h)

**Dependencies:** REQ-002, REQ-006.

---

#### REQ-004: Menu-bar-to-extension capture bridge

**Priority:** P0

**Description:** Clicking Save Current Article in the menu-bar app must request capture from the active browser extension rather than relying only on URL fetching.

**Acceptance Criteria:**

- [ ] App identifies the frontmost supported browser within 250ms.
- [ ] App sends a capture request to the browser-specific bridge.
- [ ] Extension captures the active tab only after browser/user permissions allow page access.
- [ ] If no extension bridge is available, app attempts URL-only capture only for explicitly public pages.
- [ ] Capture result includes success/failure code, title, URL, and record ID when saved.

**Technical Specification:**

```text
Bridge options by browser:
- Safari: Safari Web Extension native app messaging or app group/local service mediation.
- Chromium: extension listens for local service request token or native messaging host request.
- Fallback: app obtains active tab URL/title via AppleScript where allowed, then uses capture-url guardrails.
```

**Task Breakdown:**

- Spike Safari app-to-extension messaging: Medium (8h)
- Spike Chromium native messaging versus localhost polling: Medium (8h)
- Implement selected bridge abstraction: Large (20h)
- Add timeout/retry behavior: Medium (6h)
- Add integration test harness: Medium (8h)

**Dependencies:** REQ-002, REQ-003, REQ-006.

---

#### REQ-005: Rendered article and metadata extraction

**Priority:** P0

**Description:** Browser extensions must extract article body and metadata from the rendered page visible to the user, then send a versioned capture payload to the local service.

**Acceptance Criteria:**

- [ ] Extension extracts clean article body using Readability-style extraction.
- [ ] Extension includes URL, canonical URL when available, title, site, byline/author when available, published date when available, selected text when present, and HTML metadata.
- [ ] Extension classifies access as public only when explicit page metadata indicates free public access; otherwise restricted.
- [ ] YouTube pages include rendered transcript segments with timestamps when available.
- [ ] Payloads over 16 MiB fail with a clear payload-too-large error before sending or at service boundary.

**Technical Specification:**

```json
{
  "version": 1,
  "type": "article",
  "url": "https://example.com/story",
  "access": "restricted",
  "dom_extract": {
    "title": "Story title",
    "byline": "Author",
    "site": "example.com",
    "published_at": "2026-06-19",
    "text": "Rendered article text",
    "summary": null,
    "html_meta": {}
  },
  "tags": []
}
```

**Task Breakdown:**

- Reuse Chromium extractor for Safari Web Extension: Medium (8h)
- Normalize metadata fields across browsers: Medium (6h)
- Add extraction quality fixtures: Medium (8h)
- Add YouTube transcript compatibility checks: Medium (6h)

**Dependencies:** Existing browser sensor code, capture payload contract.

---

#### REQ-006: Local authenticated capture service

**Priority:** P0

**Description:** The local service must accept browser capture payloads over loopback only, authenticate requests, write Markdown, and update the index.

**Acceptance Criteria:**

- [ ] Capture service binds only to `127.0.0.1`.
- [ ] Capture endpoint requires bearer authentication or an equivalent per-install local secret.
- [ ] Capture token is never printed in normal setup logs.
- [ ] Successful captures are searchable through CLI and MCP within 2 seconds for payloads under 2 MiB.
- [ ] Repeated capture of the same canonical URL updates the existing record instead of creating duplicate records.

**Technical Specification:**

```text
Endpoint: POST http://127.0.0.1:47291/capture
Max payload: 16 MiB
Output: Markdown record in ~/Starlee/vault + SQLite index update
Health: GET /health returns service name, status, and payload version only
```

**Task Breakdown:**

- Add extension handshake endpoint/state: Medium (6h)
- Confirm service lifecycle under LaunchAgent: Small (4h)
- Add token redaction tests: Small (3h)
- Add capture latency test: Medium (6h)

**Dependencies:** Existing Rust engine and HTTP endpoint.

---

#### REQ-007: Fallback capture modes

**Priority:** P0

**Description:** Starlee must offer fallback modes when rendered browser capture is unavailable.

**Acceptance Criteria:**

- [ ] URL-only capture runs only for HTTP(S) pages with explicit public/free access signals.
- [ ] Active tab URL/title capture is available for Safari and Chromium browsers when OS scripting permissions allow it.
- [ ] Selected-text capture is available through a menu item or global hotkey after the user grants required macOS permissions.
- [ ] Fallback captures clearly label source type and access level.
- [ ] Fallback failures include the next recommended action.

**Technical Specification:**

```text
Fallback priority:
1. Browser extension rendered capture.
2. Public URL fetch with access guardrails.
3. Selected text capture.
4. Manual paste capture.
```

**Task Breakdown:**

- Implement active tab URL resolver: Medium (8h)
- Add selected-text capture flow: Large (12h)
- Add fallback routing tests: Medium (8h)
- Add menu copy for recovery paths: Small (3h)

**Dependencies:** REQ-002, public fetch guardrails.

---

### Should Have (P1) - Important after MVP

#### REQ-008: Global keyboard shortcut

**Priority:** P1

**Description:** Starlee should support a configurable global shortcut for Save Current Article.

**Acceptance Criteria:**

- [ ] User can enable or disable the shortcut from the menu-bar app.
- [ ] Default shortcut avoids common browser and macOS conflicts.
- [ ] Shortcut triggers the same capture path as the menu-bar button.
- [ ] If macOS permission is missing, app opens the relevant permission instructions.

**Technical Specification:**

```text
Default candidate: Control+Option+S
Storage: ~/Starlee/config.json or macOS user defaults with no token exposure
```

**Task Breakdown:**

- Add hotkey registration: Medium (6h)
- Add shortcut preferences UI: Medium (6h)
- Add permission recovery copy: Small (3h)

**Dependencies:** REQ-002, REQ-004.

---

#### REQ-009: Floating on-screen save button

**Priority:** P1

**Description:** Starlee should optionally show a small floating save button on screen for users who prefer a persistent visual target.

**Acceptance Criteria:**

- [ ] User can enable or disable floating button.
- [ ] Button does not cover the menu bar, Dock, or active text cursor by default.
- [ ] Button opacity/position is user-adjustable.
- [ ] Click triggers the same capture path as menu-bar save.

**Technical Specification:**

```text
macOS implementation: NSPanel or borderless NSWindow at floating level
Position persistence: user defaults
Accessibility: keyboard-accessible menu equivalent remains available
```

**Task Breakdown:**

- Build floating panel: Medium (8h)
- Add drag/reposition persistence: Medium (6h)
- Add click-through/visibility settings: Medium (6h)

**Dependencies:** REQ-002, REQ-004.

---

#### REQ-010: Capture quality preview

**Priority:** P1

**Description:** Starlee should allow users to inspect the extracted title/body snippet before or after saving when extraction confidence is low.

**Acceptance Criteria:**

- [ ] If extracted body has fewer than 500 characters or metadata is missing title and URL, app flags low confidence.
- [ ] Low-confidence result offers Open Record, Retry with Selection, and Ignore.
- [ ] Preview never uploads content externally.

**Technical Specification:**

```text
Confidence inputs:
- body length
- title presence
- URL/canonical URL presence
- Readability score where available
- transcript availability for YouTube
```

**Task Breakdown:**

- Add confidence scoring: Medium (6h)
- Add preview popover: Medium (8h)
- Add retry-with-selection flow: Medium (6h)

**Dependencies:** REQ-005, REQ-007.

---

### Nice to Have (P2) - Future Enhancement

#### REQ-011: Packaged extension distribution

**Priority:** P2

**Description:** Starlee could ship signed/listed Safari and Chromium extensions to reduce friction compared with unpacked developer-mode installation.

**Acceptance Criteria:**

- [ ] Safari extension is bundled in the signed Starlee Mac app.
- [ ] Chromium extension is available through an official extension store or documented enterprise install path.
- [ ] Installer chooses packaged extension path when available and local unpacked path only for development builds.

**Task Breakdown:**

- Apple developer signing and notarization: Large (16h)
- Chrome Web Store packaging: Large (16h)
- Release channel detection: Medium (8h)

**Dependencies:** Developer accounts, extension review policies.

---

#### REQ-012: Browser history or reading queue suggestions

**Priority:** P2

**Description:** Starlee could suggest unsaved articles from recent browser history only after explicit user permission.

**Acceptance Criteria:**

- [ ] Feature is opt-in.
- [ ] User can review suggested URLs before capture.
- [ ] No browser history leaves the local machine.

**Task Breakdown:**

- Research browser history permission model: Medium (8h)
- Build local suggestion engine: Large (16h)
- Add review UI: Medium (8h)

**Dependencies:** Browser extension permissions, privacy review.

---

## Non-Functional Requirements

### Performance

**Response Time:**

- Menu-bar click to capture request dispatch: <250ms p95.
- Extension extraction for article pages under 2 MiB of text: <1.5s p95.
- Local service save + index update for payloads under 2 MiB: <2s p95.
- Notification after successful capture: <2s p95 from service response.

**Throughput:**

- Support 10 capture requests per minute on one local machine without dropped requests.
- Support search immediately after capture for 1 active user and a 10,000-record local vault.

**Resource Usage:**

- Idle menu-bar app memory: <80 MiB.
- Idle capture service memory excluding embedding model cache: <150 MiB.
- Capture service CPU while idle: <1% average over 5 minutes.
- Extension background activity while idle: 0 network requests per minute except handshake/health checks capped at 4 per minute.

---

### Security

**Authentication:**

- Local capture requests require a per-install token or equivalent local secret with at least 256 bits of entropy.
- Token files use owner-only permissions on Unix-like systems.
- Token values are redacted in logs, Codex output, diagnostics, and PRD/test fixtures.

**Authorization:**

- Browser extension captures only active tab content for the browser/profile where the user enabled the extension.
- Starlee must not bypass browser extension permission prompts.
- URL-only capture refuses known paid domains and pages without explicit public/free access metadata.

**Data Protection:**

- Captured article bodies stay in `~/Starlee/vault` unless the user explicitly exports or shares.
- Share bundles strip restricted bodies.
- Local inference uses local model files and does not send text to external inference providers.
- Logs may include result codes and timing but not article bodies or tokens.

---

### Reliability

**Availability:**

- Capture service restarts automatically through LaunchAgent if it exits unexpectedly.
- Menu-bar app detects service unavailable state within 15 seconds.
- If extension bridge fails, fallback options remain accessible from the menu.

**Error Handling:**

- Every failed capture maps to one of: service unreachable, extension missing, host permission denied, page unreadable, extraction empty, payload too large, auth failed, URL fetch refused, index write failed, unknown.
- Unknown errors include a local log path and no secret values.
- Error notification appears within 2 seconds of failure detection.

---

### Compatibility

**Operating System:**

- MVP: macOS 14+.
- Future: macOS 13 if Swift/AppKit APIs permit without reducing security behavior.

**Browsers:**

- Safari current and previous major version.
- Chrome current and previous two stable versions.
- Arc, Brave, and Edge through Chromium extension compatibility where APIs match.

**Codex:**

- Codex plugin install path must work through local personal marketplace.
- Starlee remains usable outside Codex after initial setup.

---

### Accessibility

- Menu-bar actions have text labels and keyboard alternatives.
- Notifications include actionable labels where supported.
- Floating button, if implemented, has a menu-bar equivalent and does not become the only capture path.
- Color is not the only indicator of service status; status text is available.

---

## Technical Considerations

### System Architecture

**Current Architecture:**

Starlee currently has a Rust CLI/engine, Markdown vault, SQLite FTS5 + sqlite-vec search index, local embedding model, MCP server, loopback HTTP capture endpoint, and Chromium-compatible browser sensor.

**Proposed Changes:**

Add a production-grade macOS menu-bar app and browser-extension bridge. The app becomes the user-facing control surface. The extension remains the page-reading component. The local service remains the storage/indexing authority.

**Diagram:**

```text
Codex setup prompt
      |
      v
Repo installer ──> Codex plugin + MCP tools
      |
      ├──> Starlee.app menu-bar UI
      ├──> LaunchAgent: starlee serve
      └──> Browser extension setup

Daily capture:

User click in menu bar
      |
      v
Starlee.app identifies active browser
      |
      v
Browser extension captures active tab DOM
      |
      v
POST /capture on 127.0.0.1:47291
      |
      v
Markdown vault + SQLite FTS/vector index
      |
      v
macOS notification + searchable MCP/CLI results
```

**Key Components:**

1. **Codex Plugin:** Bundles Starlee skill guidance and MCP server configuration so Codex can install, verify, and use Starlee.
2. **Rust Local Service:** Owns setup, capture ingestion, vault writes, indexing, search, export, ingest, and MCP tools.
3. **macOS Menu-Bar App:** Owns onboarding UI, browser setup guidance, daily save button, health display, notifications, and fallback selection.
4. **Browser Extensions:** Own rendered DOM extraction, page metadata extraction, YouTube transcript capture, and extension permission boundary.
5. **Local Vault/Index:** Markdown in `~/Starlee/vault` is canonical; SQLite index is rebuildable.

---

### API Specifications

#### Endpoint: Capture Rendered Page

```http
POST http://127.0.0.1:47291/capture
Authorization: Bearer <local-token>
Content-Type: application/json
```

```json
{
  "version": 1,
  "type": "article",
  "url": "https://example.com/story",
  "access": "restricted",
  "dom_extract": {
    "title": "Story title",
    "byline": "Author",
    "site": "example.com",
    "published_at": "2026-06-19",
    "text": "Rendered article text.",
    "summary": null,
    "html_meta": {}
  },
  "tags": []
}
```

Response `201 Created`:

```json
{
  "id": "2026-0619-example",
  "title": "Story title",
  "url": "https://example.com/story",
  "file_path": "/Users/user/Starlee/vault/2026/example.md",
  "access": "restricted"
}
```

#### Endpoint: Health

```http
GET http://127.0.0.1:47291/health
```

Response `200 OK`:

```json
{
  "status": "ready",
  "service": "starlee-capture",
  "payload_version": 1
}
```

#### Proposed Endpoint: Extension Handshake

```http
POST http://127.0.0.1:47291/extension/hello
Authorization: Bearer <local-token>
Content-Type: application/json
```

```json
{
  "browser": "Safari",
  "extension_version": "0.1.0",
  "can_capture_active_tab": true
}
```

---

### Data Model

**Existing canonical storage:**

```text
~/Starlee/
  config.json
  vault/{year}/{id}-{slug}.md
  index.db
  models/
  sensor-extension/
  logs/
```

**New local state candidates:**

```json
{
  "onboarding": {
    "completed_at": "2026-06-19T00:00:00Z",
    "preferred_browser": "Safari",
    "fallback_only": false
  },
  "extension": {
    "last_handshake_at": "2026-06-19T00:00:00Z",
    "browser": "Safari",
    "version": "0.1.0"
  },
  "menu_bar": {
    "launch_at_login": true,
    "notifications_enabled": true,
    "global_hotkey": null
  }
}
```

---

### Technology Stack

**Local Engine:**

- Rust 2024
- SQLite with FTS5 and sqlite-vec
- FastEmbed with local quantized BGE-small model
- tiny_http loopback server

**macOS App:**

- Swift/AppKit
- UserNotifications
- LaunchAgent or login item management
- Optional NSPanel for future floating button

**Browser Extensions:**

- Safari Web Extension bundled with Starlee.app
- Chromium Manifest V3 extension
- Mozilla Readability-style extraction
- Localhost or native messaging bridge, selected per browser

**Codex Integration:**

- `.codex-plugin/plugin.json`
- `.mcp.json`
- Starlee MCP tools
- Local personal marketplace install path

---

### External Dependencies

**Third-Party Services:**

1. **GitHub**
   - Purpose: Source repo distribution.
   - Failure handling: User can install from a local checkout.

2. **Browser Extension Stores or Developer Extension Modes**
   - Purpose: User-approved browser extension installation.
   - Failure handling: Use local unpacked extension for development builds or selected-text fallback.

3. **Apple Developer Signing/Notarization**
   - Purpose: Lower-friction macOS and Safari extension distribution.
   - Failure handling: Local development builds continue to work with explicit user approval.

**Internal Dependencies:**

- Starlee CLI and engine.
- Starlee browser capture payload contract.
- Starlee Codex plugin manifest and MCP server.
- Starlee release/package scripts.

---

### Migration Strategy

1. **Phase 1: Preserve current CLI/plugin behavior**
   - Keep existing commands and MCP tools working.
   - Add `doctor`/verification without breaking `setup`.

2. **Phase 2: Add menu-bar app around current service**
   - Ship UI that uses existing `/health` and `/capture`.
   - No vault migration required.

3. **Phase 3: Add extension handshake**
   - Existing extension capture continues to work.
   - New handshake improves setup verification.

4. **Phase 4: Add Safari Web Extension**
   - Chromium extension remains available.
   - Safari users move from fallback URL capture to rendered page capture.

5. **Rollback Plan**
   - Disable menu-bar app launch item.
   - Continue using CLI/MCP/browser extension directly.
   - `starlee reindex` restores search index from Markdown vault.

---

### Testing Strategy

**Unit Tests:**

- Installer state transitions and redaction.
- Browser detection mapping.
- Error taxonomy mapping.
- Capture payload validation.
- Extension handshake state.

**Integration Tests:**

- Installer with temporary HOME.
- LaunchAgent/service health lifecycle where macOS CI permits.
- Menu-bar app calling local service.
- Extension sending article fixture payloads.
- Duplicate canonical URL update.

**E2E Tests:**

- Codex-assisted setup dry run.
- Safari extension setup and active article capture.
- Chrome extension setup and active article capture.
- Paywalled/logged-in fixture route uses extension path, not URL fetch.
- Service-down recovery from menu-bar app.

**Privacy Tests:**

- No token in logs.
- No article body in installer output.
- Restricted bodies stripped from share bundles.
- URL fetch refuses ambiguous paid content.

---

## Implementation Roadmap

### Phase 1: Setup Foundation (Week 1)

**Goal:** Make installation idempotent, diagnosable, and plugin-native.

**Tasks:**

- [ ] Task 1.1: Add `starlee doctor --json` with redacted output (REQ-001, REQ-006)
  - Complexity: Medium (8h)
  - Dependencies: Existing CLI
  - Owner: Engine

- [ ] Task 1.2: Refactor installer into named phases with explicit failures (REQ-001)
  - Complexity: Medium (6h)
  - Dependencies: Existing installer
  - Owner: Platform

- [ ] Task 1.3: Add temporary-HOME installer tests (REQ-001)
  - Complexity: Medium (8h)
  - Dependencies: Task 1.2
  - Owner: Platform

**Validation Checkpoint:** Running installer twice leaves one plugin entry, one LaunchAgent, healthy service, and no token in output.

---

### Phase 2: Menu-Bar MVP (Week 2)

**Goal:** Ship menu-bar app as the primary visible interface.

**Tasks:**

- [ ] Task 2.1: Build menu items and service status polling (REQ-002)
  - Complexity: Large (16h)
  - Dependencies: Phase 1
  - Owner: macOS

- [ ] Task 2.2: Add notification result states (REQ-002)
  - Complexity: Medium (6h)
  - Dependencies: Task 2.1
  - Owner: macOS

- [ ] Task 2.3: Add Recent Captures and Open Folder menu actions (REQ-002)
  - Complexity: Medium (6h)
  - Dependencies: Task 2.1
  - Owner: macOS

**Validation Checkpoint:** User can launch Starlee.app, see status, and save via existing extension/page path or fallback URL path.

---

### Phase 3: Browser Onboarding and Handshake (Week 3)

**Goal:** Convert extension setup from documentation into guided product flow.

**Tasks:**

- [ ] Task 3.1: Add browser detection and picker (REQ-003)
  - Complexity: Medium (8h)
  - Dependencies: Phase 2
  - Owner: macOS

- [ ] Task 3.2: Add extension handshake endpoint and state (REQ-003, REQ-006)
  - Complexity: Medium (8h)
  - Dependencies: Phase 1
  - Owner: Engine

- [ ] Task 3.3: Add Chromium setup launcher and verification (REQ-003)
  - Complexity: Medium (8h)
  - Dependencies: Task 3.2
  - Owner: Browser

**Validation Checkpoint:** Setup is not marked complete until a browser extension handshake succeeds or fallback-only mode is selected.

---

### Phase 4: Menu-Bar Capture Bridge (Week 4-5)

**Goal:** Make the menu-bar button trigger rendered active-tab extraction.

**Tasks:**

- [ ] Task 4.1: Research and choose Safari bridge mechanism (REQ-004)
  - Complexity: Medium (8h)
  - Dependencies: Phase 3
  - Owner: Browser/macOS

- [ ] Task 4.2: Research and choose Chromium bridge mechanism (REQ-004)
  - Complexity: Medium (8h)
  - Dependencies: Phase 3
  - Owner: Browser

- [ ] Task 4.3: Implement browser bridge abstraction (REQ-004)
  - Complexity: Large (20h)
  - Dependencies: Tasks 4.1, 4.2
  - Owner: Browser/macOS

- [ ] Task 4.4: Add active tab capture E2E tests (REQ-004, REQ-005)
  - Complexity: Medium (10h)
  - Dependencies: Task 4.3
  - Owner: QA

**Validation Checkpoint:** Menu-bar click captures current article through extension in at least one Safari build and one Chromium build.

---

### Phase 5: Fallbacks, Quality, and Packaging (Week 6)

**Goal:** Handle real-world failures and prepare a release-ready package.

**Tasks:**

- [ ] Task 5.1: Implement selected-text fallback (REQ-007)
  - Complexity: Large (12h)
  - Dependencies: Phase 4
  - Owner: macOS

- [ ] Task 5.2: Add error taxonomy and recovery UI (REQ-004, REQ-007)
  - Complexity: Medium (8h)
  - Dependencies: Phase 4
  - Owner: Product/macOS

- [ ] Task 5.3: Add package/signing release checklist (REQ-011)
  - Complexity: Medium (8h)
  - Dependencies: Phase 2
  - Owner: Release

**Validation Checkpoint:** Supported failures produce an actionable notification/menu state and no article bodies or tokens appear in logs.

---

### Effort Estimation

- Phase 1: ~22 hours
- Phase 2: ~28 hours
- Phase 3: ~24 hours
- Phase 4: ~46 hours
- Phase 5: ~28 hours
- **Total:** ~148 hours
- **Risk Buffer:** +30% (~44 hours) for browser bridge and signing uncertainty
- **Final Estimate:** ~192 hours, approximately 5-7 calendar weeks for one engineer or 3-4 weeks for two engineers with macOS/browser-extension experience.

---

## Out of Scope

Explicitly not included in MVP:

1. **Silent browser-extension installation**
   - Reason: Browser permission prompts are intentional user consent boundaries.
   - Future: Reduce friction with signed/listed extensions, not bypassing consent.

2. **Codex Computer Use as required setup path**
   - Reason: Computer Use itself requires permissions and is unsuitable as the product’s primary consumer installer.
   - Future: Offer an optional “Codex can guide this setup” helper prompt.

3. **Windows menu-bar/system tray implementation**
   - Reason: MVP is macOS-first because the desired interaction is native Mac menu-bar capture.
   - Future: Consider Windows tray app after macOS capture loop is proven.

4. **Cloud sync of vault content**
   - Reason: Conflicts with local-first privacy posture.
   - Future: User-controlled export/share bundles remain available.

5. **Bypassing paywalls or access controls**
   - Reason: Starlee should capture what the user can access through their browser and should not defeat site controls.
   - Future: Better permission messaging and selected-text capture for ambiguous cases.

6. **Mobile capture apps**
   - Reason: Separate platform and extension/share-sheet model.
   - Future: iOS share extension after Mac workflow stabilizes.

---

## Open Questions & Risks

### Open Questions

#### Q1: Which browser bridge should Starlee use for Chromium?

- **Current Status:** Options are native messaging, extension polling a localhost queue, or browser-specific APIs.
- **Options:** (A) Native messaging, (B) localhost polling, (C) active extension toolbar only.
- **Owner:** Browser engineering.
- **Deadline:** End of Phase 4 research.
- **Impact:** High. Determines extension packaging, permissions, and reliability.

#### Q2: How should Safari extension distribution work before App Store signing?

- **Current Status:** Safari Web Extensions are usually bundled with a macOS app and require user enablement.
- **Options:** (A) Developer/local build for early users, (B) signed/notarized direct download, (C) Mac App Store.
- **Owner:** Release engineering.
- **Deadline:** Before Phase 5.
- **Impact:** High. Determines onboarding friction for Safari users.

#### Q3: Should the menu-bar click capture immediately or show a confirmation for restricted pages?

- **Current Status:** Proposed behavior captures immediately and marks ambiguous access as restricted.
- **Options:** (A) Immediate capture, (B) confirmation for restricted pages, (C) user preference.
- **Owner:** Product.
- **Deadline:** Before E2E testing.
- **Impact:** Medium. Affects trust and daily friction.

#### Q4: What is the exact activation metric source?

- **Current Status:** Local-first product avoids central analytics by default.
- **Options:** (A) Opt-in telemetry, (B) local-only usability studies, (C) no metrics beyond tests.
- **Owner:** Product/privacy.
- **Deadline:** Before beta.
- **Impact:** Medium. Affects measurement quality.

### Risks & Mitigation

| Risk | Likelihood | Impact | Severity | Mitigation | Contingency |
|------|------------|--------|----------|------------|-------------|
| Safari app-to-extension bridge is more constrained than expected | Medium | High | High | Spike in Phase 4 before broad UI work | Use Safari toolbar button for MVP and keep menu-bar fallback |
| Browser extension review/signing delays release | Medium | High | High | Support local/unpacked dev channel and direct signed app path | Release Chromium first or fallback-only beta |
| Users distrust extension permissions | High | Medium | High | Explain one-time permission plainly and keep content local | Offer selected-text and public URL modes |
| Local service port conflict | Medium | Medium | Medium | Detect occupied port and report owning process where possible | Configurable port with extension regeneration |
| Capture extracts navigation/comments instead of article body | Medium | Medium | Medium | Use extraction confidence and preview for low-confidence captures | Offer retry with selected text |
| Token leaks into logs or Codex output | Low | High | High | Redaction tests and owner-only token files | Rotate token and regenerate extension config |
| Menu-bar app increases CPU or memory at idle | Low | Medium | Medium | Health polling capped and measured in tests | Disable polling and use on-demand checks |

---

## Validation Checkpoints

### Checkpoint 1: Installer Readiness

**Criteria:**

- [ ] `./scripts/install.sh` succeeds on a clean macOS account with Codex installed.
- [ ] Second install run changes no duplicate state.
- [ ] `codex plugin list` shows `starlee@personal` installed and enabled.
- [ ] `GET /health` returns ready.
- [ ] Installer output contains no capture token.

**If Failed:** Fix installer phase or redaction before menu-bar work continues.

---

### Checkpoint 2: Menu-Bar Readiness

**Criteria:**

- [ ] Starlee.app appears in menu bar within 5 seconds.
- [ ] App shows service status accurately.
- [ ] App can restart local service.
- [ ] App sends a notification for a test capture.
- [ ] App remains under 80 MiB idle memory.

**If Failed:** Fix app/service lifecycle before browser bridge work continues.

---

### Checkpoint 3: Browser Setup Readiness

**Criteria:**

- [ ] Browser picker detects Safari and at least one Chromium browser on a test Mac.
- [ ] User can open the chosen browser’s setup path.
- [ ] Extension handshake reaches local service.
- [ ] Onboarding state records browser choice and handshake timestamp.

**If Failed:** Keep onboarding in incomplete state and show browser-specific recovery.

---

### Checkpoint 4: One-Click Capture Readiness

**Criteria:**

- [ ] Menu-bar Save Current Article captures active tab in Safari.
- [ ] Menu-bar Save Current Article captures active tab in Chrome or Arc.
- [ ] Capture appears in `starlee search` within 2 seconds for fixture payloads under 2 MiB.
- [ ] Duplicate URL updates existing Markdown record.
- [ ] Paywalled/logged-in fixture uses extension path and is marked restricted.

**If Failed:** Do not call the feature one-click; release extension-toolbar capture plus menu-bar fallback only.

---

### Checkpoint 5: Privacy and Release Readiness

**Criteria:**

- [ ] Legal/privacy invariant script passes.
- [ ] No article bodies or tokens in logs.
- [ ] Share bundle audit still strips restricted bodies.
- [ ] Extension permission copy is visible before install/enablement.
- [ ] README and Codex plugin skill describe browser permission boundary.

**If Failed:** Block release until privacy issue is fixed or documented as an explicit non-MVP limitation.

---

## Appendix: Task Breakdown Hints

### Suggested Task Structure

**Setup and Diagnostics (6 tasks, ~34 hours)**

1. Add redacted `starlee doctor --json` (8h)
2. Refactor installer into named phases (6h)
3. Add installer idempotency tests (8h)
4. Add plugin reinstall/cachebuster flow (4h)
5. Add setup docs and Codex prompt (3h)
6. Add setup failure taxonomy (5h)

**macOS App (8 tasks, ~62 hours)**

7. Build menu-bar UI and status menu (16h)
8. Add LaunchAgent/login item management (8h)
9. Add service restart controls (6h)
10. Add notifications (6h)
11. Add browser picker UI (8h)
12. Add active browser detection (8h)
13. Add selected-text fallback (12h)
14. Add global shortcut candidate (6h)

**Browser Extensions and Bridge (9 tasks, ~88 hours)**

15. Add extension handshake route (8h)
16. Add handshake implementation to Chromium extension (6h)
17. Create Safari Web Extension target (16h)
18. Port article extractor to Safari extension (8h)
19. Research Safari bridge (8h)
20. Research Chromium bridge (8h)
21. Implement bridge abstraction (20h)
22. Add YouTube transcript compatibility checks (6h)
23. Add extraction quality fixtures (8h)

**Testing and Release (7 tasks, ~48 hours)**

24. Add E2E installer test harness (8h)
25. Add active article capture fixtures (10h)
26. Add privacy redaction tests (6h)
27. Add performance benchmarks (6h)
28. Add signed package checklist (6h)
29. Add release smoke script (6h)
30. Run usability test with 5 users and record setup/capture times (6h)

**Total:** ~232 task-hours including buffer and validation.

### Parallelizable Tasks

**Can work in parallel:**

- Menu-bar UI can proceed while browser bridge research runs.
- Installer diagnostics can proceed while Safari extension target is created.
- Extraction fixtures can proceed while notification UI is implemented.
- Privacy tests can proceed once the installer and service logging paths are stable.

**Must be sequential:**

- Installer idempotency before Codex-assisted setup claims.
- Browser detection before guided extension setup.
- Extension handshake before onboarding completion.
- Bridge implementation before true menu-bar one-click capture.
- Privacy validation before beta release.

### Critical Path Tasks

1. Installer phase refactor.
2. Starlee.app menu-bar status UI.
3. Browser picker.
4. Extension handshake.
5. Safari and Chromium bridge decision.
6. Bridge implementation.
7. Active-tab capture E2E test.
8. Privacy/release validation.

**Critical path duration:** ~112 hours before buffer, approximately 3-4 weeks for one experienced engineer.

---

**End of PRD**

This PRD is structured for engineering task breakdown. Requirements include priorities, acceptance criteria, dependencies, and task estimates so the work can be planned without reinterpreting the product direction.
