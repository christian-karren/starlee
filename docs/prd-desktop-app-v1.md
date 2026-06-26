# PRD: Starlee Desktop App v1 — Functional Upgrade

**Author:** Christian Karren
**Date:** 2026-06-25
**Status:** Draft
**Version:** 1.0
**Quality-Validated:** Yes

> **Branch & workflow:** All implementation for this PRD MUST happen on the
> `feature/desktop-app-v1` branch (already created off `main`). Do not commit
> feature work to `main`. Land via PR with the full test suite (`make test`) and
> `./scripts/legal-invariants.sh` green. This is PRD 1 of 2; the UI polish pass
> (`docs/prd-desktop-app-polish.md`) is a sequential follow-on milestone that runs only
> after the surfaces defined here exist.

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

Starlee's macOS desktop app currently captures and lists items but is a read-only
shell: clicking a Library card does nothing, all three Library header buttons are
inert, hover feedback is unreliable (~1000ms or only after a click), there is no
way to delete, filter, organize, or import content, and there is no first-run
onboarding. This PRD upgrades the desktop app (`gui/`) plus the Rust core paths it
depends on (`src/`) into a usable personal-knowledge tool: an in-app reader on
card click, Filter / Edit (permanent delete) / Upload header actions, a
user-managed topic taxonomy, bulk document import (PDF, Word, text, Markdown), a
redesigned Settings information architecture, and a three-step onboarding flow. The
target outcome is a daily-usable v1 in which a new user can install via
onboarding, capture from the menu bar, and then browse, read, organize, import,
and delete their knowledge base — all local-first, with ≥85% test coverage on new
backend code and zero P0/P1 security findings.

---

## Problem Statement

### Current Situation

The desktop app ([`gui/DesktopWindowController.swift`](../gui/DesktopWindowController.swift),
renderer in [`gui/Resources/renderer/`](../gui/Resources/renderer/)) renders a
month-grouped Library backed by the Rust capture service over a loopback HTTP +
CLI bridge ([`gui/StarleeClient.swift`](../gui/StarleeClient.swift)). The
underlying Rust core is strong (hybrid FTS5 + vector search, audited share
bundles, atomic Markdown vault), but the desktop surface exposes almost none of
it. Concretely, today:

- **Library cards are not clickable** — selecting a card yields no detail or
  reading experience.
- **The three Library header buttons render but have no behavior.**
- **Hover feedback is broken** — the selection/hover border on a card appears
  after ~1s, intermittently, or only after a click.
- **No delete** — there is no way to remove a capture from the vault or index from
  the app (no GUI and no backend delete command exist; the engine only upserts).
- **No structured filter** — only free-text search within the selected month
  exists ([`applyFilters()`](../gui/DesktopWindowController.swift), ~line 1168;
  [`main.js`](../gui/Resources/renderer/main.js) `matchesQuery`).
- **No topics/categories** — captures cannot be organized by user-defined subject.
- **No import** — users cannot add their own documents (PDFs, lecture notes).
- **Settings is mostly cosmetic** — `settings.html`/`settings.js` is dominated by
  animated-background shader knobs and lacks real app configuration.
- **No onboarding** — the app opens to an empty Library with no guidance on
  installing the browser extension or Codex plugin (confirmed: zero matches for
  any onboarding/first-run/welcome pattern in `gui/`).

### User Impact

- **Who is affected:** Every desktop user — both first-run installers (no setup
  guidance) and daily users (cannot read, organize, prune, or import).
- **How they're affected:** The app can capture but cannot be *used* as a brain.
  Items accumulate with no reader, no organization, no cleanup, and no import
  path, so the corpus is write-mostly and low-value to revisit.
- **Severity:** High — the desktop app is the primary browse/library surface, and
  its core interactions (click, organize, delete, import) are absent or broken.

### Business Impact

- **Cost of problem:** The product's core promise — "build a lifelong, searchable
  repository of everything you've learned" — is undeliverable from the desktop
  app today. Captured content cannot be read or curated, undermining retention.
- **Opportunity cost:** High-value use cases (e.g., a student bulk-importing a
  semester of materials) are impossible without upload + topics.
- **Strategic importance:** This is the gating milestone between a capture demo
  and a daily-driver knowledge tool, and the prerequisite layer for the polish
  pass and any future multi-device story.

### Why Solve This Now?

The Rust core and capture pipeline are stable (87 commits, CI green, audited
bundles). The bottleneck to a usable product is the desktop interaction layer.
Building it now also establishes the data model (topics in Markdown frontmatter)
and backend operations (delete, ingest) that later milestones depend on.

---

## Goals & Success Metrics

### Goal 1: Make the Library interactive and responsive

- **Description:** Every card opens a reader; every header button works; hover/
  selection feedback is immediate.
- **Metric:** (a) Count of non-functional Library controls; (b) hover-feedback
  latency.
- **Baseline:** (a) 4 of 4 primary interactions dead (card click + 3 header
  buttons); (b) hover border appears in ~1000ms or only on click.
- **Target:** (a) 0 dead interactions; (b) hover/selection visual state applied in
  <100ms on 100% of hovers.
- **Timeframe:** End of this milestone.
- **Measurement Method:** Manual QA script (Appendix) + automated renderer tests
  asserting state classes apply on `mouseenter`.

### Goal 2: Deliver knowledge-management capability (read, organize, prune, import)

- **Description:** Users can read any item in-app, create topics, filter by type/
  author/date/topic, permanently delete, and bulk-import documents.
- **Metric:** Count of the six target capabilities (reader, topics, filter,
  delete, single upload, bulk upload) that are fully functional and tested.
- **Baseline:** 0 of 6.
- **Target:** 6 of 6, each with acceptance criteria met and ≥85% line coverage on
  the new Rust paths.
- **Timeframe:** End of this milestone.
- **Measurement Method:** Acceptance-criteria checklist + `make test` coverage
  report on new modules.

### Goal 3: Activate new users via onboarding

- **Description:** First run guides browser-extension and Codex-plugin setup and
  explains the menu-bar capture model.
- **Metric:** Share of fresh installs that complete browser-extension setup
  (extension checks in to `/extension/hello`) within the first app session.
- **Baseline:** 0% (no onboarding exists).
- **Target:** ≥80% of fresh installs reach a healthy `browser_bridge` /
  `extension` doctor check within the first session.
- **Timeframe:** Measured over the first 2 weeks after release.
- **Measurement Method:** Local `starlee doctor` / bridge-health check-in state
  recorded at session end (no remote telemetry; redacted local counters only).

### Goal 4: Ship with rigorous quality and no security regressions

- **Description:** New backend and GUI code is thoroughly tested and reviewed.
- **Metric:** (a) Line coverage on new Rust modules (delete, ingest/import,
  topics, document parsing); (b) count of P0/P1 security findings at review; (c)
  GUI/renderer automated test count.
- **Baseline:** (a) 0% (code does not exist); (b) n/a; (c) 0 GUI/renderer tests.
- **Target:** (a) ≥85%; (b) 0 P0/P1 findings open at merge; (c) ≥20 new renderer/
  bridge tests covering the new flows.
- **Timeframe:** At merge of this milestone.
- **Measurement Method:** Coverage tooling, `/security-review` output,
  `node --test` (sensor) + new renderer test suite counts.

---

## User Stories

### Story 1: Read a captured item in-app (REQ-002)

**As a** Starlee user,
**I want to** click any Library card and read the full captured content with its
metadata inside the app,
**So that I can** revisit what I saved without leaving Starlee or hunting for the
vault file.

**Acceptance Criteria:**
- [ ] Clicking a card opens a full in-app reader (detail view) for that record.
- [ ] The reader shows the captured body: article text rendered readably, or
      timestamped `[MM:SS]` YouTube transcript lines.
- [ ] The reader shows all metadata: title, author, site/source, URL, type,
      captured date, consumed date, word count, YouTube transcript status/source,
      vault file path, and assigned topics.
- [ ] Reader actions present and working: Open original URL, Reveal in Finder,
      Edit topics/tags, Delete.
- [ ] Edge cases handled: empty/missing body, `[Transcript unavailable]` records,
      bodies ≥100k characters (no UI freeze), and `restricted`-access records
      (body shown locally but never exported).
- [ ] Closing the reader returns to the same Library scroll position and selected
      month.

**Task Breakdown Hint:**
- Task 2.1: Add a `record get` JSON path the GUI can call (reuse engine
  `get`/recent) returning full body + metadata (~4h)
- Task 2.2: Build the reader view in the WKWebView renderer (HTML/CSS/JS) (~10h)
- Task 2.3: Wire Swift ↔ renderer message passing for open/close + actions (~5h)
- Task 2.4: Renderer + bridge tests for body/metadata rendering and edge cases (~5h)

**Dependencies:** None (read-only; foundation for REQ-003 actions).

### Story 2: Permanently delete an item (REQ-003)

**As a** Starlee user,
**I want to** enter an edit mode and permanently delete a capture with one action,
**So that I can** keep my brain free of mistakes and content I no longer want.

**Acceptance Criteria:**
- [ ] An **Edit** header button toggles edit mode; a minus (–) control appears in
      the top-right of every card while active.
- [ ] Clicking a card's minus control (or Delete in the reader) prompts a single
      confirmation that states the action is permanent and irreversible.
- [ ] On confirm, the record is removed from the Markdown vault file **and** all
      index rows (sources, chunks, FTS5, vectors) in one atomic operation.
- [ ] After delete, the item disappears from the Library without a full reload;
      if it was the last item in a month, that month's sidebar button is removed.
- [ ] Deleting the record currently open in the reader closes the reader cleanly.
- [ ] A new `starlee delete <id>` CLI command and MCP `delete` tool exist and are
      covered by tests; deletion is rejected if the id resolves outside the vault.
- [ ] After delete, `starlee reindex` produces an index identical to one built
      from the post-delete vault (vault/index consistency invariant holds).

**Task Breakdown Hint:**
- Task 3.1: Engine + vault delete (remove Markdown file atomically) (~5h)
- Task 3.2: Index delete (sources/chunks/FTS/vectors) in a transaction (~5h)
- Task 3.3: CLI `delete` + MCP `delete` tool + auth on loopback delete (~4h)
- Task 3.4: Edit-mode UI, minus controls, confirmation dialog (~6h)
- Task 3.5: Tests: delete round-trip, reindex-equivalence, path-safety,
      delete-open-record, last-in-month (~6h)

**Dependencies:** REQ-002 (reader Delete action shares the path).

### Story 3: Filter the Library (REQ-004)

**As a** Starlee user with many captures,
**I want to** filter my Library by type, author, date ingested, and topic,
**So that I can** quickly narrow to exactly the subset I care about.

**Acceptance Criteria:**
- [ ] A **Filter** header button opens filter controls for: type (article /
      YouTube / note / uploaded document), author, date-ingested range, and topic.
- [ ] Filters compose with AND semantics and coexist with the existing free-text
      search and month selection (e.g., type=article AND topic="CS 101" AND
      ingested in the selected range).
- [ ] Author and topic options are populated from the actual corpus, not
      hardcoded.
- [ ] Active filters are visibly indicated and clearable in one action.
- [ ] An empty filter result shows a clear empty state, not a blank table.
- [ ] Filtering 1,000 records updates the visible list in <150ms.

**Task Breakdown Hint:**
- Task 4.1: Extend the library payload with author/type/topic/ingested fields and
      a corpus facet summary (distinct authors, topics) (~5h)
- Task 4.2: Filter UI (controls, active-filter chips, clear) in renderer (~8h)
- Task 4.3: Compose filters with search + month in `applyFilters()` (~4h)
- Task 4.4: Tests for each filter dimension and combinations (~4h)

**Dependencies:** REQ-005 (topic filter needs the taxonomy).

### Story 4: Create and assign topics (REQ-005)

**As a** Starlee user,
**I want to** create my own topics/categories and assign them to captures,
**So that I can** organize my brain around subjects that matter to me (e.g., a
class, a project, a theme).

**Acceptance Criteria:**
- [ ] Users can create, rename, and delete topics from a topic-management surface
      (in the reader's Edit and in Settings → Topics).
- [ ] A record can have zero or more topics; assignment/removal works from the
      reader and is reflected immediately in filters.
- [ ] **Topics persist in the canonical layer:** they are written to the Markdown
      frontmatter (`topics:` list) in the vault, and mirrored into the index for
      low-latency filtering — so `starlee reindex` (which rebuilds the index from the
      vault) preserves all topic assignments with zero loss.
- [ ] Topic names are validated and sanitized (length ≤64 chars, no control
      characters, no YAML/SQL-breaking content) before being written to
      frontmatter or the index.
- [ ] Deleting a topic removes it from all records' frontmatter and the index but
      never deletes the records themselves.

**Task Breakdown Hint:**
- Task 5.1: Add `topics` to the record/frontmatter model + vault read/write
      ([`src/model.rs`](../src/model.rs), [`src/vault.rs`](../src/vault.rs)) (~6h)
- Task 5.2: Mirror topics into the index schema + reindex preservation
      ([`src/index.rs`](../src/index.rs)) (~6h)
- Task 5.3: CLI/MCP/loopback ops: list topics, add/remove topic on a record,
      rename/delete topic (~6h)
- Task 5.4: Topic management UI in reader + Settings (~8h)
- Task 5.5: Tests: frontmatter round-trip, reindex preservation, name
      sanitization, rename/delete propagation (~6h)

**Dependencies:** None (foundational; unblocks REQ-004 topic filter and REQ-006
batch tagging).

### Story 5: Bulk-import my own documents (REQ-006)

**As a** student (or any user) starting a new project,
**I want to** upload many of my own documents (PDF, Word, text, Markdown) into my
brain at once and tag them to a topic,
**So that I can** seed my knowledge base with everything for a class or project in
one step.

**Acceptance Criteria:**
- [ ] An **Upload** header button accepts file selection of `.pdf`, `.docx`,
      `.txt`, and `.md` files.
- [ ] Multiple files can be selected and imported in one action (bulk), with a
      per-batch topic assignment applied to all imported records.
- [ ] Each file is parsed to text **locally** (no external service), normalized
      into the same Markdown + frontmatter contract as captures, written to the
      vault, and indexed (chunked + embedded) like any other record.
- [ ] Text extraction handles PDF and Word; unsupported or corrupt files are
      skipped with a per-file error report, and a partial-batch failure does not
      abort the whole batch.
- [ ] Duplicate detection: re-importing a file whose normalized content hash
      matches an existing uploaded record updates that record instead of creating
      a duplicate (analogous to canonical-URL recapture).
- [ ] Per-file size cap (≥50MB) and a batch of ≥200 files / ≥500MB total complete
      without exhausting memory (streaming/bounded parsing).
- [ ] Progress feedback is shown for batches, including per-file success/skip.

**Task Breakdown Hint:**
- Task 6.1: Choose + integrate local PDF and DOCX text-extraction crates
      (MIT/Apache-compatible), behind a `DocumentParser` abstraction (~10h)
- Task 6.2: `starlee import <paths...> [--topic T]` CLI + MCP `import` tool +
      bulk ingest path in the engine ([`src/engine.rs`](../src/engine.rs)) (~8h)
- Task 6.3: Content-hash dedupe + frontmatter for uploaded source type (~5h)
- Task 6.4: Upload UI: file picker, batch topic, progress, per-file results (~8h)
- Task 6.5: Tests: each format parses, corrupt/unsupported skip, dedupe, large
      batch bounded-memory, partial-failure (~8h)

**Dependencies:** REQ-005 (batch topic assignment).

### Story 6: Configure the app from a coherent Settings page (REQ-007)

**As a** Starlee user,
**I want** a clearly organized Settings page covering setup, integrations, data,
topics, and appearance,
**So that I can** manage Starlee without the menu-bar option-click maze and find
real configuration instead of only background-shader knobs.

**Acceptance Criteria:**
- [ ] Settings is reorganized into labeled sections: General/About, Capture,
      Browser Extension, Codex Plugin, Topics, Data (import/export + reindex +
      diagnostics), Appearance, and Privacy.
- [ ] The existing **audited share-bundle export/ingest** ([`src/bundle.rs`](../src/bundle.rs))
      is surfaced in Data (Export brain / Ingest a friend's brain) — it currently
      has no GUI.
- [ ] Appearance keeps the background presets but collapses the per-shader sliders
      under an "Advanced" disclosure so they no longer dominate the page.
- [ ] Browser Extension and Codex Plugin sections show live status (installed /
      checked-in / out-of-date) and a setup/reinstall action, reusing
      `starlee doctor` checks.
- [ ] Every control persists and reflects current state on reopen (no
      write-only/no-op controls).
- [ ] A "Re-run onboarding" action is available here.

**Task Breakdown Hint:**
- Task 7.1: Restructure `settings.html`/`settings.js`/`settings.css` into
      sections; move shader knobs under Advanced (~8h)
- Task 7.2: Wire status panels to doctor/bridge-health JSON (~5h)
- Task 7.3: Surface export/ingest + reindex + diagnostics actions (~6h)
- Task 7.4: Tests for settings state round-trip and status rendering (~4h)

**Dependencies:** REQ-005 (Topics section), REQ-008 (re-run onboarding hook).

### Story 7: Onboard on first launch (REQ-008)

**As a** new Starlee user,
**I want** a short first-run flow that helps me install the browser extension and
Codex plugin and explains the menu-bar capture model,
**So that I can** start building my brain immediately and understand how the
pieces fit.

**Acceptance Criteria:**
- [ ] On first launch (no prior onboarding state), a multi-step onboarding flow
      appears; it is skippable and re-runnable from Settings.
- [ ] Step 1 (Browser extension): the user chooses Safari, Chrome, or Firefox.
      **Chrome** opens the working install/load path; **Safari** and **Firefox**
      render a clear "Coming soon" state (not a broken link) and let the user
      proceed. A "Set up later" option defers to Settings while encouraging
      setup now.
- [ ] Step 2 (Codex plugin): shows install status and an install/encourage action.
- [ ] Step 3 (Orientation): explains that the Starlee icon lives in the macOS
      menu bar (with a visual pointer/callout to it), that clicking it captures
      the current article/video, that querying happens via Codex, and that the
      Library lives in this app; closes with the lifelong-brain framing.
- [ ] First-run detection is reliable across relaunches; completing or skipping
      onboarding records durable state so it does not reappear unprompted.

**Task Breakdown Hint:**
- Task 8.1: Onboarding state model + first-run detection (~3h)
- Task 8.2: Step 1 browser chooser with Chrome-live / Safari+Firefox "coming
      soon"; deep links to install/load (~6h)
- Task 8.3: Step 2 Codex plugin status/install (~3h)
- Task 8.4: Step 3 orientation with menu-bar callout (~5h)
- Task 8.5: Tests for first-run detection + step gating (~3h)

**Dependencies:** REQ-007 (re-run entry point).

### Story 8: Reliable hover/selection feedback (REQ-001)

**As a** Starlee user,
**I want** the card hover/selection border to appear instantly and consistently,
**So that** the Library feels responsive instead of laggy or broken.

**Acceptance Criteria:**
- [ ] Hovering a card applies the hover visual state in <100ms, every time, with
      no click required.
- [ ] Selection state is distinct from hover state and is keyboard-navigable.
- [ ] The fix is verified against a list of ≥500 rows without per-frame jank.

**Task Breakdown Hint:**
- Task 1.1: Diagnose the current hover delay (event wiring vs. layout/`:hover`
      vs. table-view redraw) (~3h)
- Task 1.2: Implement immediate hover/selection state (~4h)
- Task 1.3: Renderer test asserting state class on `mouseenter` (~2h)

**Dependencies:** None.

---

## Functional Requirements

| ID | Requirement | Priority |
| --- | --- | --- |
| REQ-001 | Library cards apply hover/selection visual state in <100ms reliably, without requiring a click. | P0 (Must) |
| REQ-002 | Clicking a Library card opens a full in-app reader showing the captured body and all metadata, with Open original / Reveal in Finder / Edit topics / Delete actions. | P0 (Must) |
| REQ-003 | An Edit mode exposes per-card delete; confirmed delete permanently removes the record from the Markdown vault and all index rows atomically, exposed via GUI, `starlee delete`, and an MCP `delete` tool. | P0 (Must) |
| REQ-004 | A Filter control filters the Library by type, author, date-ingested range, and topic, composing with search and month selection. | P0 (Must) |
| REQ-005 | Users can create/rename/delete topics and assign them to records; topics persist in Markdown frontmatter and mirror into the index, surviving reindex with zero loss. | P0 (Must) |
| REQ-006 | An Upload control bulk-imports `.pdf`/`.docx`/`.txt`/`.md` files, parsed locally into the vault+index, with per-batch topic, dedupe, and per-file error reporting. | P0 (Must) |
| REQ-007 | Settings is reorganized into General/Capture/Extension/Codex/Topics/Data/Appearance/Privacy sections, surfaces export/ingest, and collapses shader knobs under Advanced. | P1 (Should) |
| REQ-008 | A skippable, re-runnable first-run onboarding flow covers browser-extension choice (Chrome live; Safari/Firefox "coming soon"), Codex plugin, and menu-bar orientation. | P0 (Must) |
| REQ-009 | The Rust core exposes the new operations (delete, bulk import with PDF/DOCX parsing, topic CRUD) over CLI, MCP, and authenticated loopback, preserving the privacy/loopback/bearer-token invariants. | P0 (Must) |
| REQ-010 | The `StarleeClient` Swift bridge must not block the main thread on capture/detail/import calls (replace the synchronous `DispatchSemaphore` pattern with async callbacks). | P1 (Should) |

---

## Non-Functional Requirements

**Performance**
- Reader (detail view) opens in <150ms for a record already in the local index.
- Library renders and filters 1,000 records in <500ms initial paint and <150ms
  per filter/search keystroke; lists >500 rows must virtualize or paginate
  (current code loads a hardcoded 500 rows fully into memory at
  [`DesktopWindowController.swift`](../gui/DesktopWindowController.swift) ~line
  1061 — revisit).
- Permanent delete completes in <200ms and leaves the index queryable
  immediately.
- Bulk import processes ≥200 files / ≥500MB total without exceeding 500MB
  resident memory (bounded/streaming parse); per-file hard cap ≥50MB.
- The main thread is never blocked for >16ms by a bridge call (REQ-010).

**Security & Privacy**
- All new loopback endpoints (delete, import, topic ops) require the existing
  bearer token and bind to `127.0.0.1` only; unauthenticated calls return 401.
- Delete must resolve ids to paths strictly inside the vault root; any path that
  would escape the vault is rejected (no path traversal). Verified by test.
- Upload must validate file type by content sniffing (not just extension), bound
  memory on large/malicious files, and never execute imported content.
- Topic names are sanitized before frontmatter/SQL use (length ≤64, no control
  chars, no YAML/SQL-injection content).
- The privacy boundary is preserved: `restricted` bodies remain local and are
  still stripped from export bundles; the existing `legal-invariants.sh` check
  must stay green.
- Document parsing is fully local; no document content leaves the device.

**Reliability & Data Integrity**
- The vault remains canonical; the index remains fully rebuildable. After any
  delete, import, or topic change, `starlee reindex` must yield an index
  equivalent to one freshly built from the vault (asserted by test).
- Bulk import is resilient to partial failure: a corrupt file in a batch does not
  abort or corrupt the batch or the index.

**Accessibility**
- New interactive controls have ≥40×40px hit areas, visible keyboard focus, and
  `aria-label`s; the reader is keyboard-navigable and screen-reader-labeled.

**Test Coverage**
- New Rust modules (delete, import/parsing, topics) reach ≥85% line coverage with
  meaningful assertions (round-trip, idempotency, failure paths).
- ≥20 new renderer/bridge tests cover reader rendering, filter combinations,
  edit/delete flow, topic assignment, and onboarding step-gating.

---

## Technical Considerations

### Architecture

- **Surfaces touched:** Swift/AppKit shell and the WKWebView renderer in
  [`gui/`](../gui/); Rust core in [`src/`](../src/). Several "desktop" features are
  full-stack — delete, import, and topics require new engine/vault/index code, not
  just UI.
- **Bridge:** [`StarleeClient.swift`](../gui/StarleeClient.swift) talks to the
  Rust capture service over authenticated loopback HTTP and via CLI subprocess.
  New operations should be exposed consistently across CLI ([`src/main.rs`](../src/main.rs)),
  MCP ([`src/mcp.rs`](../src/mcp.rs)), and the loopback server
  ([`src/http.rs`](../src/http.rs)). Fix the main-thread-blocking
  `DispatchSemaphore` (REQ-010) while adding the new request paths.
- **Engine decomposition:** [`src/engine.rs`](../src/engine.rs) is a ~2.3k-LOC
  god-module. New capabilities (delete, import, topics) should land as cohesive
  submodules (e.g., `engine/delete.rs`, `engine/import.rs`, `engine/topics.rs`)
  rather than swelling the monolith further; opportunistically extract Spotify/
  bridge/diagnostics where these changes touch them.
- **Topic data model:** topics are canonical in Markdown frontmatter
  ([`src/vault.rs`](../src/vault.rs), [`src/model.rs`](../src/model.rs)) and
  mirrored into the index ([`src/index.rs`](../src/index.rs)) for low-latency filtering;
  `reindex` rebuilds the mirror from frontmatter, guaranteeing no topic loss.
- **Document parsing:** add a local `DocumentParser` abstraction with PDF and
  DOCX backends behind a trait, mapping every format to the existing
  `CaptureInput`/Markdown contract so chunking/embedding/search are unchanged.

### Tech Stack

- Rust (edition 2024), rusqlite + FTS5 + sqlite-vec, fastembed (BGE-small),
  tiny_http loopback server, MCP stdio.
- Swift/AppKit desktop app + WKWebView HTML/CSS/JS renderer.
- Node `--test` for sensor; a renderer test harness (jsdom-style) to be added for
  the new GUI flows.

### Integrations & External Dependencies

- New crates for PDF and DOCX text extraction (must be local, no network,
  license MIT/Apache-2.0 compatible). Candidate evaluation is an open question.
- No new external/network services; document parsing and embedding stay on-device.

---

## Implementation Roadmap

All work on branch `feature/desktop-app-v1`; land via PR with `make test` and
`legal-invariants.sh` green.

- **Phase 0 — Foundations (backend-first):** REQ-005 topic model in
  frontmatter+index; REQ-009/REQ-003 delete in engine/vault/index/CLI/MCP/loopback;
  REQ-010 async bridge. Ship with tests before UI.
- **Phase 1 — Library interactivity:** REQ-001 hover fix; REQ-002 reader; REQ-003
  edit-mode delete UI on top of the Phase 0 backend.
- **Phase 2 — Organize & import:** REQ-004 filters; REQ-005 topic UI; REQ-006
  upload + document parsing.
- **Phase 3 — Settings & onboarding:** REQ-007 settings redesign; REQ-008
  onboarding.
- **Phase 4 — Hardening:** full test sweep, `/security-review`, performance
  validation against the NFR numbers, refactor pass on touched modules.

---

## Out of Scope

- Cloud sync, multi-device, and iOS (future milestone). Do not block it, but do
  not build sync here. Note: making `file_path` relative and abstracting `Vault`
  behind a trait are *welcome* if cheap, but are not requirements of this PRD.
- Building or hardening the Safari or Firefox extensions. Onboarding presents all
  three browsers, but only Chrome is functional; Safari/Firefox show "Coming
  soon."
- A new Claude Code plugin.
- A knowledge-graph / "related items" view, starred/favorites, read/unread state,
  and date-based smart collections beyond the existing month buckets.
- URL/HTML bulk ingest in Upload (v1 accepts files only: PDF/DOCX/TXT/MD).
- The visual/animation polish pass — that is PRD 2
  (`docs/prd-desktop-app-polish.md`), run sequentially after this.

---

## Open Questions & Risks

- **PDF/DOCX crate choice (Risk: medium).** Pure-Rust PDF text extraction quality
  varies; scanned/image PDFs have no text layer (OCR is out of scope). Decision +
  fallback messaging needed.
- **Delete confirmation UX (Open).** Single confirm vs. type-to-confirm given
  permanence — recommend a single explicit confirm with an "irreversible" warning.
- **Uploaded-doc dedupe key (Open).** Normalized content hash vs. filename+size —
  recommend content hash to mirror canonical-URL recapture semantics.
- **Reader rendering approach (Open).** Reuse the existing WKWebView renderer
  (consistent, lower build cost) vs. native AppKit (more native feel) — recommend
  WKWebView for v1, polished in PRD 2.
- **Virtualization scope (Risk: low/medium).** The 500-row cap must become
  pagination or virtualization to meet the 1,000-record performance target.

---

## Validation Checkpoints

- **Checkpoint A (end Phase 0):** Delete and topic operations pass round-trip and
  reindex-equivalence tests; path-traversal and auth tests green; coverage ≥85% on
  new backend modules. Gate before any UI work.
- **Checkpoint B (end Phase 1):** Card click opens reader for article + YouTube +
  empty-body records; hover <100ms verified; delete-from-UI removes from vault and
  index and updates month buttons. Manual QA script passes.
- **Checkpoint C (end Phase 2):** All four filter dimensions and combinations
  work; topics survive a real reindex; a ≥200-file mixed-format bulk import
  succeeds with correct skips and bounded memory.
- **Checkpoint D (end Phase 3):** Fresh-profile first run shows onboarding; Chrome
  path works end-to-end to a healthy bridge check; Settings sections persist and
  surface export/ingest.
- **Checkpoint E (end Phase 4):** `/security-review` shows 0 open P0/P1; all NFR
  numbers met; `make test` + `legal-invariants.sh` green; PR approved.

---

## Appendix: Task Breakdown Hints

Rough estimates aggregate to ~200 engineering hours. Suggested execution order:

1. REQ-005 topics backend (24h) → REQ-003/009 delete backend (19h) → REQ-010 async
   bridge (6h) — *foundations, tested first.*
2. REQ-001 hover (9h) → REQ-002 reader (24h) → REQ-003 delete UI (folded above).
3. REQ-004 filter (21h) → REQ-005 topic UI (folded above) → REQ-006 upload +
   parsing (39h).
4. REQ-007 settings (23h) → REQ-008 onboarding (20h).
5. Phase 4 hardening: security review, performance validation, refactor (≈15h).

Each task closes only with: unit tests with real assertions, an entry in the
manual QA script, and (for backend) a reindex-equivalence assertion where data
changes. Manual QA script lives in `docs/` alongside existing
`youtube-capture-qa.md` patterns.
