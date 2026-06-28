# PRD: Starlee Library Organization and Marketplace

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

Starlee's Library should become the user's digital brain: a place where captured articles, notes, videos, documents, and imported archives can be organized by time, topic, company, source, course, author, and shared collection. This PRD specifies a first-class Library navigation and organization system, plus a Marketplace where people, teachers, authors, institutions, and friends can publish or share curated libraries while preserving the local-first privacy model.

The expected impact is to move Starlee from a capture-and-search utility into a knowledge operating system. Success means users can find saved material through multiple mental models in under 10 seconds, imported/shared libraries can be queried without polluting My Library, and paywalled article bodies are omitted from shared or marketplace exports by default.

---

## Problem Statement

### Current Situation

Starlee can capture and display a user's saved items, but the Library still behaves like a flat shelf with a month selector and free-text search. That is not enough for the product vision. A user's knowledge base may include years of reading, high school or university course material, Substack archives, company research, topic maps, YouTube transcripts, personal notes, PDFs, and libraries shared by friends. The current navigation model cannot represent that scale or those mental models.

Today, the Library lacks:

- A persistent navigation hierarchy with "My Library" as the primary personal corpus and external/shared libraries beneath it.
- Nested organization by year, month, topic, subtopic, company, author, source, class, collection, or library publisher.
- Automatic world-class tagging at capture/import time with confidence, provenance, and user correction paths.
- A marketplace model for publishing, installing, updating, searching, and querying third-party libraries.
- Privacy and licensing controls that omit paywalled or restricted article bodies from shared bundles.
- A UI architecture that can support multiple libraries without turning the sidebar into a long unstructured list.

### User Impact

- **Who is affected:** Daily Starlee users with growing personal libraries; students using course material; researchers tracking companies and topics; teachers publishing class packets; writers publishing archives; friends sharing collections.
- **How they're affected:** Users must remember exact titles or use free-text search, even when they naturally think "January 2026," "AI Infrastructure," "Paul Graham essays," "APUSH primary sources," or "NVIDIA / semiconductors."
- **Severity:** Critical for retention. A digital brain that cannot be organized becomes harder to trust as it grows.

### Business Impact

- **Cost of problem:** Capture volume can grow faster than retrieval value. That produces library clutter, lower revisit frequency, and weaker evidence that Starlee compounds over time.
- **Opportunity cost:** Starlee cannot support paid/community libraries, class libraries, friend-sharing, or author archives without library boundaries, taxonomy, publishing, and privacy rules.
- **Strategic importance:** Library navigation and marketplace libraries are the product's core surface. They define whether Starlee feels like a personal brain, a reading archive, a course companion, or a shared knowledge network.

### Why Solve This Now?

The desktop app now has a coherent Library visual direction and basic card/search/sort controls. The next product-defining step is not another button pass; it is the information architecture that determines how every future capture, import, search, marketplace install, and shared library appears.

### PACE Brief and Assumptions

- **Purpose:** Define an engineer-ready plan for Starlee's Library organization, navigation, tagging, and Marketplace library system.
- **Audience:** Product, design, and engineering contributors building the macOS app, Rust core, local index, export/import model, and future marketplace service.
- **Constraints:** Preserve local-first personal content; omit paywalled article bodies from shared/marketplace exports; keep My Library visually primary; support nested navigation; avoid collapsing personal, friend, and marketplace content into one unbounded list.
- **Examples:** APUSH course libraries, Paul Graham essay archive, Substack author archive, friend-shared research library, company-tracking collections, nested topics such as `Tech -> Semiconductors -> AI Accelerators`.
- **Assumption:** Marketplace discovery will require a remote catalog service, but installed library content and personal corpus indexing remain local by default.
- **Assumption:** This PRD defines product requirements; implementation should land in smaller PRDs/tasks after review.

---

## Goals & Success Metrics

### Goal 1: Make Library retrieval match user mental models

- **Description:** Users can navigate by time, topic, company, source, author, library, and collection without relying on exact title recall.
- **Metric:** Median time to find a known saved item in a 1,000-item corpus.
- **Baseline:** Not measured; assumed >30 seconds when title is not remembered because only search/month navigation exists.
- **Target:** <10 seconds median and <20 seconds p90 in usability tests with 1,000 mixed items.
- **Timeframe:** Within 30 days of beta release.
- **Measurement Method:** Local usability scripts with seeded corpora and task timing.

### Goal 2: Improve capture-time organization quality

- **Description:** Starlee assigns useful metadata at capture/import time while allowing user correction.
- **Metric:** Percentage of captures with at least one correct topic, one source type, and extracted entity/company when present.
- **Baseline:** Current captures have basic metadata but no reliable topic/company taxonomy.
- **Target:** ≥85% correct topic assignment and ≥80% correct company/entity assignment on a 200-item labeled evaluation set.
- **Timeframe:** Before marketplace beta.
- **Measurement Method:** Human-labeled evaluation set with precision/recall reports.

### Goal 3: Support installed and shared libraries without corrupting My Library

- **Description:** Marketplace and friend libraries are browseable/queryable as distinct libraries while My Library remains first in navigation.
- **Metric:** Percentage of queries/results that preserve library attribution and access boundaries.
- **Baseline:** No marketplace library model exists.
- **Target:** 100% of search results show source library; 0 personal records are moved into external libraries without explicit import.
- **Timeframe:** At marketplace alpha.
- **Measurement Method:** Integration tests for library-scoped search, install, uninstall, and import flows.

### Goal 4: Enforce paywall and restricted-content sharing rules

- **Description:** Shared libraries omit article bodies that are paywalled, restricted, private, or not redistributable.
- **Metric:** Number of restricted bodies included in exported public/friend bundles.
- **Baseline:** Starlee already has restricted-body removal in share bundles; marketplace rules are not defined.
- **Target:** 0 restricted bodies exported across 1,000 automated fixture exports.
- **Timeframe:** Before any friend-sharing or marketplace release.
- **Measurement Method:** Privacy invariant tests and bundle inspection.

### Goal 5: Establish marketplace activation

- **Description:** Users can install a library from a marketplace listing and query it alongside My Library.
- **Metric:** Install-to-first-query completion rate.
- **Baseline:** 0%; marketplace does not exist.
- **Target:** ≥70% of test users complete install and run a query within 3 minutes.
- **Timeframe:** Within 45 days of marketplace beta.
- **Measurement Method:** Local event counters and moderated user tests.

---

## User Stories

### Story 1: Navigate My Library by nested structure

**As a** Starlee user with hundreds of saved items,  
**I want to** browse My Library by year, month, topic, subtopic, company, source, and author,  
**So that I can** find material using the structure I remember instead of exact keywords.

**Acceptance Criteria:**

- [ ] My Library is always the top navigation group.
- [ ] User can expand/collapse Year, Month, Topics, Companies, Sources, Authors, and Collections sections.
- [ ] Topic navigation supports at least 3 nested levels, for example `Tech -> Semiconductors -> AI Accelerators`.
- [ ] Selecting a navigation node filters the card grid and updates the search scope label.
- [ ] A selected scope can be combined with search text and sort order.
- [ ] Empty sections are hidden unless the user enables an "show empty groups" developer/debug mode.
- [ ] Navigation state persists across app relaunch.

**Task Breakdown Hint:**

- Task 1.1: Define local navigation tree data contract (~6h)
- Task 1.2: Add topic/company/source grouping in Rust library payload (~10h)
- Task 1.3: Build expandable macOS sidebar model and renderer state (~16h)
- Task 1.4: Add persistence for expanded sections and selected scope (~6h)
- Task 1.5: Add UI and integration tests with a 1,000-item fixture corpus (~10h)

**Dependencies:** REQ-001, REQ-002, REQ-003.

### Story 2: Automatically tag new captures

**As a** reader saving an article,  
**I want** Starlee to tag it with topics, companies, source, author, date, and confidence,  
**So that** the article appears in the right places without manual filing.

**Acceptance Criteria:**

- [ ] Each capture receives source type, canonical source name, author when available, publication date when available, topics, and detected organizations/companies.
- [ ] Each generated topic/entity stores a confidence score from 0.00 to 1.00 and a provenance string: `metadata`, `content`, `user`, `import`, or `publisher`.
- [ ] Tags below a configurable confidence threshold are stored as suggestions, not committed taxonomy.
- [ ] Users can accept, remove, rename, or merge tags from the reader or card detail surface.
- [ ] User corrections update future suggestions for that user's local corpus.
- [ ] The tagging system handles article, note, YouTube, PDF, Markdown, Word, and imported marketplace records.

**Task Breakdown Hint:**

- Task 2.1: Extend capture schema/frontmatter for taxonomy provenance (~8h)
- Task 2.2: Implement deterministic metadata extraction for source/date/author (~12h)
- Task 2.3: Implement local classification interface with pluggable model/provider (~18h)
- Task 2.4: Build tag correction UI and merge/rename commands (~16h)
- Task 2.5: Create labeled evaluation dataset and reporting command (~12h)

**Dependencies:** REQ-004, REQ-005, REQ-006, model/provider decision.

### Story 3: Install a marketplace library

**As a** student, teacher, researcher, or reader,  
**I want to** install a curated library from the marketplace,  
**So that I can** query and browse material someone else prepared.

**Acceptance Criteria:**

- [ ] Marketplace listings show title, publisher, description, item count, topics, update date, access level, and whether full bodies are included.
- [ ] Installing a library downloads a signed manifest and allowed content bundle.
- [ ] Installed libraries appear beneath My Library in the sidebar.
- [ ] User can search within one external library or across selected libraries.
- [ ] Uninstall removes local marketplace content and index rows without deleting My Library records.
- [ ] Updates preserve user annotations unless the user chooses to remove them.

**Task Breakdown Hint:**

- Task 3.1: Define marketplace library manifest format (~10h)
- Task 3.2: Add install/uninstall/update commands in Rust core (~20h)
- Task 3.3: Extend index schema for multi-library ownership and attribution (~16h)
- Task 3.4: Build Marketplace browsing/install UI shell (~20h)
- Task 3.5: Add signed fixture libraries and install tests (~14h)

**Dependencies:** REQ-007, REQ-008, REQ-009, remote catalog decision.

### Story 4: Publish a library

**As a** teacher, author, institution, or user,  
**I want to** publish a curated library,  
**So that** others can install a coherent knowledge bundle.

**Acceptance Criteria:**

- [ ] Publisher can create a library from selected records, imported documents, or an external archive.
- [ ] Publisher can add title, description, topic hierarchy, cover/icon, intended audience, license/access metadata, and update policy.
- [ ] Export pipeline omits restricted/paywalled bodies by default and records citation metadata instead.
- [ ] Public bundle validation reports included body count, omitted body count, license warnings, and broken sources.
- [ ] A library cannot be published if required manifest fields are missing.
- [ ] Private/friend-only library publishing requires an explicit recipient or access list.

**Task Breakdown Hint:**

- Task 4.1: Define publish workflow and manifest validation rules (~12h)
- Task 4.2: Add library-builder commands for selected records/import folders (~20h)
- Task 4.3: Extend privacy-audited export for marketplace manifests (~16h)
- Task 4.4: Build publish review UI with body omission report (~18h)
- Task 4.5: Add validation tests for paywall/restricted fixtures (~12h)

**Dependencies:** REQ-010, REQ-011, existing audited export code.

### Story 5: Share a library with friends

**As a** Starlee user,  
**I want to** share a library with a friend through a Friends area in the marketplace,  
**So that** they can browse or query my curated collection without receiving content I cannot redistribute.

**Acceptance Criteria:**

- [ ] User can choose a friend or invite link as the recipient.
- [ ] Share preview lists records with full body included, body omitted, and metadata-only entries.
- [ ] Paywalled/restricted article bodies are omitted even for friend shares unless a future explicit rights model allows them.
- [ ] Recipient sees the shared library under a Friends section, separate from public marketplace listings.
- [ ] Sender can revoke a share; recipient loses future updates but retains any locally imported public records according to access rules.
- [ ] Friend shares can include user notes only when explicitly selected.

**Task Breakdown Hint:**

- Task 5.1: Define friend-share access model and recipient identity flow (~16h)
- Task 5.2: Build share preview and omission report (~14h)
- Task 5.3: Add friend library install/update/revoke commands (~20h)
- Task 5.4: Add Friends section in Marketplace navigation (~12h)
- Task 5.5: Add privacy regression tests (~10h)

**Dependencies:** REQ-012, REQ-013, identity/access decision.

---

## Functional Requirements

### Must Have (P0) - Required for Library Organization Beta

#### REQ-001: Persistent Multi-Library Navigation Model

**Description:** Starlee must represent My Library, installed marketplace libraries, friend libraries, and future institutional libraries as separate top-level library scopes.

**Acceptance Criteria:**

- [ ] My Library is always first in the sidebar.
- [ ] Other libraries appear below My Library in user-defined or last-used order.
- [ ] Each library has stable `library_id`, display name, type (`personal`, `marketplace`, `friend`, `course`, `author_archive`), publisher, installed version, and local path/index namespace.
- [ ] Search results include `library_id` and display name.
- [ ] Removing an external library cannot delete personal captures.

**Technical Specification:**

```sql
CREATE TABLE libraries (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('personal','marketplace','friend','course','author_archive')),
  title TEXT NOT NULL,
  publisher TEXT,
  version TEXT,
  installed_at TEXT NOT NULL,
  updated_at TEXT,
  access_level TEXT NOT NULL CHECK (access_level IN ('private','friends','public','licensed')),
  manifest_json TEXT NOT NULL
);
```

**Task Breakdown:**

- Add migration and data model: Medium (6-8h)
- Add library-aware payloads: Medium (6-8h)
- Add namespace-aware delete/uninstall tests: Small (3-4h)

**Dependencies:** Existing SQLite index and vault path conventions.

#### REQ-002: Sidebar Information Architecture

**Description:** The desktop navigation must support nested filters beneath each library, including time, topics, companies, sources, authors, and collections.

**Acceptance Criteria:**

- [ ] Navigation supports at least 1,000 visible nodes without p95 expand/collapse latency above 100ms.
- [ ] Nodes can be expanded/collapsed with mouse and keyboard.
- [ ] Selected node applies a filter scope to cards and reader context.
- [ ] Sidebar visually distinguishes My Library, Marketplace libraries, and Friends libraries.
- [ ] UI supports at least 3 nested topic levels.

**Technical Specification:**

```json
{
  "libraryId": "my-library",
  "sections": [
    {"kind": "time", "children": [{"label": "2026", "children": [{"label": "June 2026"}]}]},
    {"kind": "topic", "children": [{"label": "Tech", "children": [{"label": "Semiconductors"}]}]},
    {"kind": "company", "children": [{"label": "NVIDIA"}]}
  ]
}
```

**Task Breakdown:**

- Define JSON contract: Small (3h)
- Generate tree from index metadata: Medium (8h)
- Build native/sidebar renderer: Large (16h)
- Add accessibility tests: Medium (6h)

**Dependencies:** REQ-001, REQ-004.

#### REQ-003: Scoped Search and Sort

**Description:** Search, sort, and filters must operate inside the selected library/navigation scope.

**Acceptance Criteria:**

- [ ] Search within My Library excludes marketplace/friend libraries unless "All selected libraries" is chosen.
- [ ] Search within a topic node filters to that topic and descendants.
- [ ] Sort modes include newest, oldest, title A-Z, source A-Z, and relevance when search text exists.
- [ ] Card counts show `visible of total in scope`.
- [ ] Clearing scope returns to My Library default view.

**Task Breakdown:**

- Extend query API with `library_id` and `scope_path`: Medium (8h)
- Update renderer state model: Medium (6h)
- Add fixture tests for nested topic queries: Medium (6h)

**Dependencies:** REQ-001, REQ-002.

#### REQ-004: Capture-Time Metadata Extraction

**Description:** Every new capture/import must receive normalized metadata for navigation and marketplace readiness.

**Acceptance Criteria:**

- [ ] Metadata fields include source type, canonical source, author, publication date, capture date, topics, companies/entities, language, access classification, and confidence values.
- [ ] Missing author/date values are represented as null, not guessed.
- [ ] User-entered tags override machine-generated suggestions.
- [ ] Metadata extraction completes within 1.5 seconds p95 for article bodies under 50,000 characters on a 2024 MacBook Air class device.

**Task Breakdown:**

- Extend frontmatter schema: Medium (6h)
- Add deterministic metadata normalizer: Medium (8h)
- Add classifier interface: Large (18h)
- Add performance benchmark fixtures: Small (4h)

**Dependencies:** Existing capture pipeline.

#### REQ-005: Taxonomy Editing and Provenance

**Description:** Users must be able to edit topics/entities and see whether each tag came from metadata, content analysis, publisher, import, or user correction.

**Acceptance Criteria:**

- [ ] User can add, remove, rename, merge, and nest topics.
- [ ] User can accept or reject suggested tags.
- [ ] Each tag stores provenance and last edited timestamp.
- [ ] Merging a topic updates all affected records atomically.
- [ ] Undo is available for one taxonomy operation during the current app session.

**Task Breakdown:**

- Add taxonomy tables/commands: Large (20h)
- Build tag edit UI in reader/card detail: Large (16h)
- Add merge/rename transaction tests: Medium (8h)

**Dependencies:** REQ-004.

#### REQ-006: Privacy and Access Classification

**Description:** Starlee must classify records by shareability and enforce body omission rules for paywalled, restricted, private, or licensed content.

**Acceptance Criteria:**

- [ ] Each record has `access_policy`: `private`, `metadata_only_export`, `public_body_allowed`, or `publisher_managed`.
- [ ] Browser-captured paid/logged-in pages default to `metadata_only_export`.
- [ ] User notes default to `private` unless explicitly selected for sharing.
- [ ] Export validation reports every omitted body and reason.
- [ ] Automated export fixtures produce 0 restricted bodies in public/friend bundles.

**Task Breakdown:**

- Define access policy schema: Medium (6h)
- Integrate with capture/import/export: Large (18h)
- Add invariant tests: Medium (8h)

**Dependencies:** Existing audited export path.

### Must Have (P0) - Required for Marketplace Alpha

#### REQ-007: Marketplace Library Manifest

**Description:** Marketplace libraries must use a signed manifest that describes content, publisher, topics, access, version, and update behavior.

**Acceptance Criteria:**

- [ ] Manifest includes title, publisher, version, item count, topic tree, license/access summary, content hashes, created/updated timestamps, and minimum Starlee version.
- [ ] Manifest validation fails on missing required fields.
- [ ] Installed library hash verification runs before indexing.
- [ ] Manifest supports metadata-only records.

**Task Breakdown:**

- Define JSON schema: Medium (8h)
- Implement manifest validator: Medium (8h)
- Add signed fixture manifests: Small (4h)

**Dependencies:** REQ-001, REQ-006.

#### REQ-008: Marketplace Install, Update, and Uninstall

**Description:** Users must be able to install, update, and uninstall marketplace libraries without affecting My Library.

**Acceptance Criteria:**

- [ ] Install creates library namespace, stores manifest, imports allowed records, and indexes searchable chunks.
- [ ] Update applies version changes and preserves user annotations.
- [ ] Uninstall removes external library records and index rows.
- [ ] Failed install rolls back to pre-install state.
- [ ] Installed libraries can be queried offline.

**Task Breakdown:**

- Add install/update/uninstall core commands: Large (24h)
- Add transaction rollback tests: Medium (8h)
- Add desktop install UI: Large (16h)

**Dependencies:** REQ-007, REQ-009.

#### REQ-009: Marketplace Discovery UI

**Description:** Starlee must provide a Marketplace surface for discovering public, course, author, and friend libraries.

**Acceptance Criteria:**

- [ ] User can browse marketplace categories and search listings.
- [ ] Listings show title, publisher, description, item count, topics, update date, access summary, and install status.
- [ ] Installed libraries appear below My Library in the main navigation.
- [ ] Friend libraries appear in a Friends subsection.
- [ ] Marketplace UI handles remote catalog unavailable state with cached installed libraries still accessible.

**Task Breakdown:**

- Define catalog API or local mock contract: Medium (8h)
- Build listing/search UI: Large (20h)
- Add install status and error states: Medium (8h)

**Dependencies:** REQ-007, remote catalog decision.

#### REQ-010: Library Publishing Workflow

**Description:** Publishers must be able to build a library bundle with manifest validation and access-policy enforcement.

**Acceptance Criteria:**

- [ ] Publisher can choose records/imports to include.
- [ ] Publishing review shows included bodies, omitted bodies, and metadata-only entries.
- [ ] Bundle generation fails if restricted body omission rules are violated.
- [ ] Manifest includes publisher and license/access fields.
- [ ] Generated bundle can be installed by a clean Starlee profile.

**Task Breakdown:**

- Add bundle builder command: Large (18h)
- Add publish validation report: Medium (10h)
- Add clean-profile install test: Medium (8h)

**Dependencies:** REQ-006, REQ-007.

### Should Have (P1)

#### REQ-011: Company and Entity Tracking Views

**Description:** Users following companies must be able to browse and filter by company/entity across personal and installed libraries.

**Acceptance Criteria:**

- [ ] Companies/entities appear as a navigation section.
- [ ] Aliases merge into canonical entities, for example `Meta`, `Facebook`, and `Meta Platforms`.
- [ ] User can split incorrectly merged entities.
- [ ] Entity view shows related topics, sources, and timeline.

**Task Breakdown:** Entity resolver (16h), entity UI (12h), alias tests (6h).

**Dependencies:** REQ-004, REQ-005.

#### REQ-012: Friend Library Sharing

**Description:** Users must be able to share curated libraries with friends through a Friends marketplace area.

**Acceptance Criteria:**

- [ ] Sender can create friend-only bundle from selected records.
- [ ] Share preview reports omitted bodies and reasons.
- [ ] Recipient installs shared library under Friends.
- [ ] Sender can revoke future updates.

**Task Breakdown:** Identity/access design (16h), share UI (14h), friend install flow (20h).

**Dependencies:** REQ-006, REQ-010.

#### REQ-013: Course and Classroom Library Mode

**Description:** Teachers must be able to publish structured class libraries for students.

**Acceptance Criteria:**

- [ ] Course library supports units/modules, required readings, optional readings, and primary/secondary source labels.
- [ ] Student can query within a course, unit, or reading set.
- [ ] Publisher can update syllabus/library version without breaking installed student annotations.

**Task Breakdown:** Course manifest extensions (10h), course navigation UI (12h), update tests (8h).

**Dependencies:** REQ-007, REQ-008.

### Nice to Have (P2)

#### REQ-014: Marketplace Ratings and Curation

**Description:** Marketplace listings may include ratings, editorial collections, and trusted publisher badges after the install/update model is stable.

**Acceptance Criteria:**

- [ ] Ratings are separated from local library content.
- [ ] Trusted publisher badge requires a signed publisher identity.
- [ ] Editorial collections link to existing marketplace listings.

**Task Breakdown:** Ratings service (16h), trust badge design (12h), editorial UI (10h).

**Dependencies:** Marketplace service and abuse policy.

---

## Non-Functional Requirements

### Performance

- Sidebar initial render for 10,000 records and 1,000 navigation nodes: <500ms p95 after index payload is loaded.
- Expand/collapse navigation node: <100ms p95.
- Scoped search over 10,000 local records: <250ms p95 for text-only queries; <500ms p95 when merging vector/relevance results.
- Metadata extraction for article captures under 50,000 characters: <1.5s p95.
- Marketplace library install of 1,000 metadata records: <30s p95 on 2024 MacBook Air class hardware.

### Privacy and Security

- Public/friend exports must include 0 restricted/paywalled article bodies across automated fixtures.
- Installed marketplace manifests must pass hash verification before indexing.
- Personal library records must never be uploaded to the marketplace catalog without an explicit publish/share action.
- Friend shares must require explicit recipient or invite-link creation.
- Marketplace catalog requests must not include personal query text unless the user searches the remote marketplace tab.

### Reliability

- Install/uninstall/update operations are transactional with rollback on failure.
- Corrupt marketplace bundles are rejected before index writes.
- Search remains usable for installed libraries when the remote marketplace catalog is unavailable.
- Local index rebuild restores library attribution for 100% of records with valid manifests/frontmatter.

### Accessibility

- Sidebar, marketplace listings, tag editors, and publish review are navigable by keyboard.
- Active library/scope has a programmatic label for screen readers.
- Color contrast for text and icons meets WCAG 2.1 AA ratio of 4.5:1.
- Expand/collapse controls expose `expanded` state.

### Compatibility

- macOS desktop app is the primary UI for this milestone.
- Rust core commands must work without the GUI for install, export, publish, and validation flows.
- Marketplace libraries are versioned so older app versions can reject unsupported manifests with a readable error.

---

## Technical Considerations

### System Architecture

```text
macOS Desktop App
  -> Library Sidebar / Marketplace UI
  -> StarleeClient bridge
  -> Rust Core Commands
      -> Vault Markdown / Library Bundles
      -> SQLite FTS5 + vector index
      -> Taxonomy + library namespace tables
      -> Privacy-audited export/import
  -> Optional remote Marketplace Catalog
```

### Proposed Data Model Changes

- Add `libraries` table for installed library scopes.
- Add `record_libraries` or library namespace column for ownership/attribution.
- Add `taxonomy_terms` table for topics/entities/companies.
- Add `record_terms` table with confidence and provenance.
- Add `access_policy` field to record metadata/frontmatter.
- Add `marketplace_manifests` table or manifest JSON storage for installed external libraries.

### API and Command Surfaces

```bash
starlee library list
starlee library tree --library my-library
starlee library install <manifest-or-url>
starlee library uninstall <library-id>
starlee library update <library-id>
starlee library publish --input <path> --output <bundle>
starlee taxonomy merge <from> <to>
starlee taxonomy rename <term-id> <name>
starlee export --library <library-id> --privacy-audit
```

### Marketplace Manifest Sketch

```json
{
  "schema": "starlee.library.v1",
  "id": "paul-graham-essays",
  "title": "Paul Graham Essays",
  "publisher": {"name": "Example Publisher", "id": "publisher_123"},
  "version": "2026.06.01",
  "access": "public",
  "item_count": 218,
  "body_policy": "public_body_allowed",
  "topics": [{"path": ["Startups"], "count": 84}],
  "records": [
    {
      "id": "record_001",
      "title": "Example Essay",
      "url": "https://example.com/essay",
      "body_included": true,
      "sha256": "..."
    }
  ]
}
```

### Migration Strategy

1. Create `my-library` as the default library for all existing records.
2. Backfill source, author, date, and topic suggestions from existing metadata/content.
3. Add access policies using conservative defaults: browser captures from unknown paid/logged-in pages become `metadata_only_export`.
4. Build navigation tree from backfilled metadata.
5. Keep previous month-only navigation behind a debug flag until beta signoff.

### Testing Strategy

- Unit tests for taxonomy merge/rename, access-policy classification, manifest validation, and library namespace operations.
- Integration tests for install, update, uninstall, publish, export, and index rebuild.
- Renderer/native UI tests for sidebar scope, nested topics, library switching, marketplace listings, and publish preview.
- Privacy fixture suite with public pages, paywalled pages, user notes, PDFs, friend shares, and marketplace bundles.
- Usability script with 1,000 seeded records and 10 retrieval tasks.

---

## Implementation Roadmap

### Phase 1: Library Foundations (Weeks 1-3)

**Goal:** Represent My Library and navigation scopes in data and UI.

- [ ] Add library namespace schema and migration (REQ-001) (~12h)
- [ ] Add navigation tree payload and sidebar state model (REQ-002) (~24h)
- [ ] Add scoped search/sort support (REQ-003) (~14h)
- [ ] Add 1,000-record fixture corpus (REQ-002, REQ-003) (~8h)

**Validation Checkpoint:** Existing records appear under My Library; time/topic/source scopes filter correctly.

### Phase 2: Tagging and Taxonomy (Weeks 4-6)

**Goal:** New captures and imports receive editable topic/entity metadata.

- [ ] Extend frontmatter and index schema (REQ-004) (~12h)
- [ ] Implement metadata extraction/classification pipeline (REQ-004) (~30h)
- [ ] Build taxonomy edit/merge UI and commands (REQ-005) (~36h)
- [ ] Build evaluation dataset and precision report (REQ-004) (~12h)

**Validation Checkpoint:** 200-item labeled set reaches ≥85% topic correctness and ≥80% company/entity correctness.

### Phase 3: Privacy-Audited Library Bundles (Weeks 7-8)

**Goal:** Publishable/shareable bundles enforce access rules.

- [ ] Add access-policy classification and frontmatter (REQ-006) (~14h)
- [ ] Extend export audit for library bundles (REQ-006, REQ-010) (~18h)
- [ ] Build publish review report (REQ-010) (~18h)
- [ ] Add restricted-body fixture tests (REQ-006) (~10h)

**Validation Checkpoint:** 1,000 fixture exports include 0 restricted bodies.

### Phase 4: Marketplace Install and Discovery Alpha (Weeks 9-12)

**Goal:** Users can install and query external libraries.

- [ ] Define and validate marketplace manifest (REQ-007) (~16h)
- [ ] Implement install/update/uninstall commands (REQ-008) (~32h)
- [ ] Build Marketplace discovery/listing UI with mocked catalog (REQ-009) (~28h)
- [ ] Add installed-library search and uninstall tests (REQ-008, REQ-009) (~16h)

**Validation Checkpoint:** Clean Starlee profile installs a fixture library, queries it, updates it, and uninstalls it.

### Phase 5: Publishing, Friends, and Course Pilots (Weeks 13-16)

**Goal:** Validate high-value publishing use cases before broad marketplace launch.

- [ ] Build library publish workflow (REQ-010) (~30h)
- [ ] Build friend-share preview/install path (REQ-012) (~34h)
- [ ] Add course-library manifest extensions (REQ-013) (~18h)
- [ ] Run APUSH/course and author-archive pilot fixtures (REQ-013) (~16h)

**Validation Checkpoint:** Pilot libraries install and query with correct attribution and privacy reports.

---

## Out of Scope

1. **Payments and marketplace monetization**  
   Reason: Discovery, install, privacy, and publishing must work before pricing.

2. **DRM circumvention or paywall bypass**  
   Reason: Starlee must not redistribute content the user or publisher does not have rights to share.

3. **Mobile marketplace UI**  
   Reason: macOS desktop navigation is the first-class surface for this milestone.

4. **Collaborative live editing of libraries**  
   Reason: Versioned publish/update is enough for initial marketplace and course use cases.

5. **Global remote search across personal libraries**  
   Reason: Personal content remains local unless explicitly published/shared.

6. **Final visual design specification for sidebar chrome**  
   Reason: This PRD defines product behavior and information architecture; final pixel treatment should follow in design QA.

---

## Open Questions & Risks

### Open Questions

#### Q1: What is the first marketplace pilot?

- **Options:** APUSH course library, Paul Graham essay archive, Substack author archive, friend-shared research collection.
- **Owner:** Product.
- **Deadline:** Before Phase 4.
- **Impact:** High; determines manifest fields and UI examples.

#### Q2: Which classification provider is allowed for capture-time tagging?

- **Options:** Local heuristic-only MVP, local model, OpenAI API with explicit opt-in, hybrid.
- **Owner:** Product/Engineering.
- **Deadline:** Before Phase 2 implementation.
- **Impact:** High; affects privacy, accuracy, latency, and cost.

#### Q3: How are friends identified?

- **Options:** Invite link, email identity, Starlee account, local file transfer.
- **Owner:** Product/Engineering.
- **Deadline:** Before REQ-012.
- **Impact:** Medium; marketplace Friends area depends on identity/access model.

#### Q4: What rights model applies to author archives?

- **Options:** Metadata-only by default, publisher-provided full body, public-domain/open-license full body.
- **Owner:** Product/Legal.
- **Deadline:** Before public marketplace launch.
- **Impact:** Critical; affects body inclusion.

### Risks & Mitigations

| Risk | Likelihood | Impact | Severity | Mitigation | Contingency |
|------|------------|--------|----------|------------|-------------|
| Taxonomy precision drops below 85% | Medium | High | High | Use labeled eval set and confidence thresholds | Store suggestions without committing them |
| Sidebar becomes visually cluttered | Medium | High | High | Limit default expanded sections and persist state | Add command palette/search-first navigation |
| Restricted bodies leak into bundles | Low | Critical | Critical | Privacy invariant tests and publish audit | Block publishing until fixed |
| Marketplace catalog introduces account complexity | Medium | Medium | Medium | Start with signed static catalog fixtures | Delay Friends until identity model is ready |
| Large libraries push local search above 500ms p95 | Medium | High | High | Namespace-aware indexes and performance tests | Add background indexing and partial install |

---

## Validation Checkpoints

### Checkpoint 1: Library Foundation

**Criteria:**

- [ ] Existing corpus migrates into My Library.
- [ ] Sidebar tree renders time/topic/source/company sections.
- [ ] Scoped search and sort work with 1,000 fixture records.
- [ ] p95 expand/collapse latency <100ms.

**If Failed:** Do not proceed to marketplace work; fix navigation data model first.

### Checkpoint 2: Tagging Quality

**Criteria:**

- [ ] 200-item labeled set reaches ≥85% topic correctness.
- [ ] 200-item labeled set reaches ≥80% company/entity correctness.
- [ ] User correction flow updates record metadata and taxonomy.
- [ ] Metadata extraction p95 <1.5s for article bodies under 50,000 characters.

**If Failed:** Lower auto-commit threshold and show more suggestions until quality improves.

### Checkpoint 3: Privacy Bundle Audit

**Criteria:**

- [ ] 1,000 fixture exports include 0 restricted/paywalled bodies.
- [ ] Publish preview reports every omitted body and reason.
- [ ] Clean-profile install can query metadata-only records without body text.

**If Failed:** Block publish/share UI behind feature flag.

### Checkpoint 4: Marketplace Alpha

**Criteria:**

- [ ] User can install, query, update, and uninstall a fixture library.
- [ ] Search results show source library for 100% of external records.
- [ ] Remote catalog unavailable state preserves installed library access.
- [ ] Install-to-first-query completed in <3 minutes by ≥70% of test users.

**If Failed:** Keep marketplace private and continue with fixture/pilot libraries only.

### Checkpoint 5: Friends and Course Pilot

**Criteria:**

- [ ] Friend share preview omits restricted bodies.
- [ ] Course library supports units/modules and reading sets.
- [ ] Recipient/student can query within shared/course scope.
- [ ] Revoked friend share stops future updates.

**If Failed:** Ship marketplace public libraries before Friends.

---

## Appendix: Task Breakdown Hints

### Suggested Task Groups

**Library Data Model and Migration (~34h)**

1. Add `libraries` schema and My Library migration (~8h)
2. Add library-aware record/index ownership (~10h)
3. Add migration tests for existing vaults (~8h)
4. Add rebuild-index attribution tests (~8h)

**Navigation and Query UI (~62h)**

5. Define navigation tree payload (~6h)
6. Generate time/topic/company/source/author trees (~14h)
7. Build expandable sidebar UI (~20h)
8. Add scoped search/sort/filter state (~10h)
9. Add keyboard/screen-reader support (~6h)
10. Add 1,000-record UI performance tests (~6h)

**Tagging and Taxonomy (~78h)**

11. Extend metadata/frontmatter schema (~8h)
12. Implement source/author/date extraction (~10h)
13. Implement topic/entity classifier interface (~18h)
14. Build taxonomy editor (~16h)
15. Implement merge/rename/split commands (~14h)
16. Build labeled evaluation report (~12h)

**Privacy and Publishing (~70h)**

17. Define access policies (~6h)
18. Integrate access policy with capture/import (~12h)
19. Extend audited export (~16h)
20. Define marketplace manifest schema (~8h)
21. Implement publish bundle builder (~18h)
22. Add privacy fixture tests (~10h)

**Marketplace and Friends (~100h)**

23. Build install/update/uninstall commands (~32h)
24. Build Marketplace listing UI with mocked catalog (~20h)
25. Build installed-library search/uninstall UI (~14h)
26. Define friend-share identity/access model (~16h)
27. Build Friends install/share preview (~18h)

**Total Estimate:** ~344 engineering hours before design QA and pilot content preparation.

### Critical Path

1. Library namespace schema
2. Navigation tree contract
3. Scoped query engine
4. Capture-time metadata schema
5. Access-policy enforcement
6. Manifest format
7. Install/update/uninstall commands
8. Marketplace UI
9. Publish/friend-share privacy audit

### Parallelizable Work

- Visual sidebar design can run in parallel with library schema after REQ-002 contract is drafted.
- Marketplace listing UI can use mocked manifests while install/update commands are built.
- Tagging evaluation set can be created before classifier implementation.
- Privacy fixture suite can be built before publish UI exists.

---

**End of PRD**
