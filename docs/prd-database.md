# PRD: Starlee Database Layer — Durable Foundation

**Author:** Christian Karren
**Date:** 2026-06-22
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

Starlee's current SQLite database has no schema versioning, no migration system, and is missing three fields that are architecturally load-bearing for the product to work well over time: `consumed_at` (distinct from `captured_at`), embedding model version per chunk, and future-proofing columns for multi-device sync. Without these, any schema change risks user data loss, model upgrades require nuking and rebuilding the entire corpus, and adding cloud sync later means a painful migration rather than a clean extension. This PRD specifies the four targeted changes that harden the database layer before the user base grows and the cost of changing it becomes prohibitive.

---

## Problem Statement

### Current Situation

The database schema is defined entirely inline in `src/index.rs::init()` as a single `CREATE TABLE IF NOT EXISTS` batch with no version tracking. The `sources` table has `captured_at` but no `consumed_at`. The `chunks` table stores embedding blobs but records no information about which model produced them. There are no `device_id` or `sync_cursor` columns anywhere. If a column needs to be added, the only options today are (a) delete and rebuild the database, losing all data, or (b) write a one-off `ALTER TABLE` migration that runs at startup with no guarantee it ran, no way to know which version a given database is at, and no rollback path.

### User Impact

- **Who is affected:** Every Starlee user on the day any schema change ships.
- **How they're affected:** Currently, a schema change in a release will silently fail or crash on startup for any user whose database was created by an older binary. There is no graceful path. The user loses their captured data or must manually delete `~/Starlee/index.db`.
- **Severity:** Critical — this is not a hypothetical risk. The MVP ships YouTube transcript capture and browser article capture, which will require schema additions. Without a migration system, the first post-MVP release breaks existing users.

### Business Impact

- **Cost of problem:** A single schema-breaking release causes every existing user to lose their captured vault. For a product whose core value proposition is "your personal knowledge base," data loss is a trust-ending event, not a UX inconvenience.
- **Opportunity cost:** Without `consumed_at` distinct from `captured_at`, queries like "articles I read last week" are impossible. Without embedding model version per chunk, upgrading to a better embedding model requires a full corpus rebuild — at 1,000 documents that takes ~10 minutes; at 10,000 it becomes a background job users will notice.
- **Strategic importance:** The database schema is the most expensive thing to change after users have real data in it. Getting this right now, before the user base grows, is the lowest-cost moment to do it.

### Why Solve This Now?

The MVP is the right moment. Zero existing users means zero migration risk for the initial schema change. Every week after launch that passes without a migration system is a week that makes the eventual migration harder and the potential for user-facing data loss higher.

---

## Goals & Success Metrics

### Goal 1: Zero data loss on schema upgrades
- **Description:** Any Starlee binary upgrade that changes the schema must migrate existing user databases automatically, without data loss, without user intervention.
- **Metric:** Number of user-reported data loss events on schema-changing releases.
- **Baseline:** 0 releases with migrations shipped to date (no existing users yet).
- **Target:** 0 data loss events across all schema-changing releases for the lifetime of the product.
- **Timeframe:** Enforced from first public release.
- **Measurement Method:** User-reported issues; `starlee doctor` reports schema version at startup.

### Goal 2: Consumed date available for retrieval and filtering
- **Description:** Every captured document must record when the user actually engaged with it, separate from when Starlee captured it.
- **Metric:** Percentage of captured documents with a non-null `consumed_at` value within 24 hours of the user engaging with them.
- **Baseline:** 0% (field does not exist).
- **Target:** 100% of documents captured via browser extension (the extension knows the engagement moment); 0% of documents captured via CLI (no engagement signal available — null is correct).
- **Timeframe:** At MVP launch.
- **Measurement Method:** SQL query on production database: `SELECT count(*) FROM sources WHERE consumed_at IS NULL AND type IN ('article','youtube_video')`.

### Goal 3: Embedding model upgrades complete without full corpus rebuild
- **Description:** When the embedding model changes, only chunks produced by the old model need re-embedding — not chunks already produced by the new model.
- **Metric:** Number of chunks re-embedded unnecessarily during a model upgrade.
- **Baseline:** 100% of chunks re-embedded on any model change (current behavior: full rebuild).
- **Target:** 0 chunks re-embedded unnecessarily. Only chunks with `embedding_model != current_model` are re-embedded.
- **Timeframe:** Before any embedding model upgrade ships.
- **Measurement Method:** `starlee reindex --model-only` reports chunks re-embedded vs. skipped.

### Goal 4: Future multi-device sync requires no schema migration
- **Description:** The columns required for encrypted cloud sync (`device_id`, `sync_cursor`) exist in the schema from day one, even though sync is not implemented yet.
- **Metric:** Boolean — does the schema at launch include `device_id` and `sync_cursor` without requiring an ALTER?
- **Baseline:** No.
- **Target:** Yes.
- **Timeframe:** At MVP launch.
- **Measurement Method:** `PRAGMA table_info(sources)` output includes both columns.

---

## User Stories

### Story 1: Schema migration on upgrade

**As a** Starlee user who has been using the product for three months,
**I want to** update to a new version of Starlee,
**So that I can** get new features without losing any of the articles and videos I have already captured.

**Acceptance Criteria:**
- [ ] Running the new binary against an older database applies all pending migrations automatically before the first read or write.
- [ ] `starlee doctor` reports the current schema version (e.g. `schema_version: 3`).
- [ ] If a migration fails partway through, the database is left in the pre-migration state (transaction rollback) and the binary exits with a clear error message rather than a corrupt database.
- [ ] A migration that has already been applied is never applied a second time.
- [ ] No user action is required — no flags, no commands, no prompts.

**Task Breakdown Hint:**
- Task 1.1: Add `schema_version` table and `get/set_version` helpers in `src/index.rs` (~2h)
- Task 1.2: Implement `run_migrations()` that applies numbered SQL scripts in order (~3h)
- Task 1.3: Wrap each migration in a transaction with rollback on failure (~2h)
- Task 1.4: Wire `run_migrations()` into `Index::init()` before any other operation (~1h)
- Task 1.5: Add schema version to `starlee doctor` output (~1h)
- Task 1.6: Write tests for migration ordering, idempotency, and rollback (~3h)

**Dependencies:** None — this is the prerequisite for all other stories.

---

### Story 2: Consumed date for retrieval and filtering

**As a** Starlee user querying my knowledge base,
**I want to** ask "what did I read last Tuesday?" and get an accurate answer,
**So that I can** recall content by when I actually engaged with it, not by when it was indexed.

**Acceptance Criteria:**
- [ ] `sources` table has a nullable `consumed_at TEXT` column (ISO 8601, UTC).
- [ ] Browser extension capture payloads include `consumed_at` set to the moment the user triggered capture.
- [ ] CLI captures (`starlee capture-url`, `starlee capture-text`) leave `consumed_at` null — no fake timestamp is invented.
- [ ] `starlee recent` and `starlee status` display `consumed_at` when non-null, `captured_at` otherwise.
- [ ] MCP `query_chunks` results include `consumed_at` so the Codex plugin can reference when content was engaged with.
- [ ] Existing rows in the database after migration have `consumed_at = NULL` — no backfill, no guessing.

**Task Breakdown Hint:**
- Task 2.1: Write migration adding `consumed_at` column to `sources` (~1h)
- Task 2.2: Add `consumed_at` to `CaptureInput` and `Record` structs in `src/model.rs` (~1h)
- Task 2.3: Pass `consumed_at` through capture → vault → index upsert path (~2h)
- Task 2.4: Update browser extension capture payload to include `consumed_at` (~1h)
- Task 2.5: Expose `consumed_at` in `recent`, `status`, and MCP `query_chunks` output (~2h)
- Task 2.6: Write tests confirming browser extension sets it, CLI leaves it null (~2h)

**Dependencies:** REQ-001 (migration system must exist before adding columns).

---

### Story 3: Selective re-embedding on model upgrade

**As a** Starlee developer upgrading the embedding model,
**I want to** re-embed only the chunks that were produced by the old model,
**So that I can** upgrade without a full corpus rebuild and without degrading search quality for already-correct chunks.

**Acceptance Criteria:**
- [ ] Every row in `chunks` has an `embedding_model TEXT NOT NULL` column populated at insert time with the model identifier (e.g. `"BAAI/bge-small-en-v1.5-quantized"`).
- [ ] `starlee reindex` without flags re-embeds all chunks (existing behavior, preserved).
- [ ] `starlee reindex --stale-embeddings-only` re-embeds only chunks where `embedding_model != current_model` and skips chunks already at the current model.
- [ ] After `--stale-embeddings-only` completes, all chunks have `embedding_model` equal to the current model.
- [ ] `starlee doctor` reports: count of chunks at current model, count of stale chunks (if any).

**Task Breakdown Hint:**
- Task 3.1: Write migration adding `embedding_model TEXT NOT NULL DEFAULT ''` to `chunks` (~1h)
- Task 3.2: Populate `embedding_model` from `embedder.name()` in `Index::upsert()` (~1h)
- Task 3.3: Implement `--stale-embeddings-only` flag on `reindex` command (~3h)
- Task 3.4: Add stale chunk count to `starlee doctor` output (~1h)
- Task 3.5: Write tests: insert chunks with model A, upgrade to model B, verify only model-A chunks are re-embedded (~3h)

**Dependencies:** REQ-001 (migration system), REQ-002 (model version column).

---

### Story 4: Schema pre-wired for future multi-device sync

**As a** Starlee developer adding cloud sync in a future release,
**I want to** find `device_id` and `sync_cursor` columns already in the schema,
**So that I can** implement sync without writing a migration against a database that already has real user data in it.

**Acceptance Criteria:**
- [ ] `sources` table has `device_id TEXT` (nullable, null on single-device installs).
- [ ] A `sync_state` table exists: `(key TEXT PRIMARY KEY, value TEXT NOT NULL)` — used to store `sync_cursor` and other sync metadata as key-value pairs, so future sync additions don't require new columns.
- [ ] Both are added in a numbered migration so they exist on fresh installs and are applied cleanly on upgrades.
- [ ] Neither column is read, written, or exposed in any CLI output or MCP tool in this release — they are schema-only placeholders.
- [ ] `starlee doctor` does not mention them (no noise for users until sync is implemented).

**Task Breakdown Hint:**
- Task 4.1: Write migration adding `device_id` to `sources` and creating `sync_state` table (~1h)
- Task 4.2: Write test asserting both exist after migration runs (~1h)

**Dependencies:** REQ-001 (migration system).

---

## Functional Requirements

### Must Have (P0) — Critical for Launch

#### REQ-001: Versioned schema migration system

**Description:** On every startup, before any read or write, Starlee checks the current schema version and applies any pending migrations in order. Each migration is a numbered SQL block. Migrations run inside a transaction; a partial failure rolls back and exits with a non-zero status code and a human-readable error. A migration that has already run is never re-run.

**Acceptance Criteria:**
- [ ] `schema_meta` table exists: `(key TEXT PRIMARY KEY, value TEXT NOT NULL)`.
- [ ] On first run against a fresh database, all migrations are applied and `schema_meta` records `('version', 'N')` where N is the highest migration number.
- [ ] On upgrade, only migrations with number > current version are applied.
- [ ] Each migration runs in a single SQLite transaction; failure rolls back the transaction, leaves the database at its prior version, and prints: `"Migration N failed: <error>. Database unchanged."`.
- [ ] `starlee doctor` output includes `schema_version: N`.
- [ ] `cargo test` includes a test that starts at version 0 and applies all migrations to a temp database without error.

**Technical Specification:**
```rust
// In src/index.rs
const MIGRATIONS: &[(&str, &str)] = &[
    ("1", "CREATE TABLE ..."),   // initial schema
    ("2", "ALTER TABLE sources ADD COLUMN consumed_at TEXT"),
    ("3", "ALTER TABLE chunks ADD COLUMN embedding_model TEXT NOT NULL DEFAULT ''"),
    ("4", "ALTER TABLE sources ADD COLUMN device_id TEXT; CREATE TABLE sync_state ..."),
];

fn run_migrations(connection: &Connection) -> Result<()> {
    // create schema_meta if missing, read current version,
    // apply pending migrations in a transaction each
}
```

**Task Breakdown:**
- Implement `schema_meta` table creation and version read/write: Small (~2h)
- Implement `run_migrations()` with transaction-per-migration: Medium (~3h)
- Wire into `Index::connection()` or `Index::init()`: Small (~1h)
- Add version to `doctor` output: Small (~1h)
- Tests: idempotency, ordering, rollback on failure: Small (~3h)

**Dependencies:** None.

---

#### REQ-002: `consumed_at` column on `sources`

**Description:** The `sources` table gains a nullable `consumed_at TEXT` column (ISO 8601, UTC). It is populated by capture paths that have an engagement signal (browser extension), and left null by capture paths that do not (CLI, background fetch). It is surfaced in `recent`, `status`, and MCP output.

**Acceptance Criteria:**
- [ ] Column exists after migration runs.
- [ ] Browser extension HTTP capture payload may include `"consumed_at": "<ISO8601>"`.
- [ ] `engine.capture()` passes `consumed_at` through to `Index::upsert()`.
- [ ] `Index::upsert()` writes it to the `sources` row.
- [ ] `starlee recent` shows `consumed: <date>` when non-null.
- [ ] `QueryChunk` returned by MCP includes `consumed_at` field (null when absent).
- [ ] Existing rows after migration have `consumed_at = NULL`.

**Technical Specification:**
```sql
-- Migration 2
ALTER TABLE sources ADD COLUMN consumed_at TEXT;
```
```rust
// CaptureInput in src/model.rs
pub struct CaptureInput {
    // existing fields ...
    pub consumed_at: Option<String>,
}

// INSERT in Index::upsert
"INSERT INTO sources(id,type,title,...,consumed_at) VALUES(...,?12)"
```

**Task Breakdown:**
- Migration SQL: Trivial (~30min)
- Model struct update (`CaptureInput`, `Record`, `SearchHit`, `QueryChunk`): Small (~1h)
- Engine capture path and HTTP handler: Small (~1h)
- Index upsert and recent query: Small (~1h)
- MCP `query_chunks` output: Small (~1h)
- Tests: Small (~2h)

**Dependencies:** REQ-001.

---

#### REQ-003: `embedding_model` column on `chunks`

**Description:** Every row in `chunks` records which embedding model produced its vector. This enables selective re-embedding: only chunks whose `embedding_model` doesn't match the current model need to be updated on a model upgrade.

**Acceptance Criteria:**
- [ ] Column `embedding_model TEXT NOT NULL DEFAULT ''` exists after migration.
- [ ] `Index::upsert()` writes `embedder.name()` into `embedding_model` for every chunk inserted.
- [ ] `starlee reindex --stale-embeddings-only` queries `SELECT rowid FROM chunks WHERE embedding_model != ?` and re-embeds only those rows, updating `embedding_model` on completion.
- [ ] `starlee doctor` reports `embedding_model_current: <name>`, `chunks_stale: <count>` (0 when all chunks are at current model).
- [ ] A `cargo test` test inserts chunks tagged with model A, then calls `reindex --stale-embeddings-only` with a model-B embedder, and asserts only the model-A chunks were updated.

**Technical Specification:**
```sql
-- Migration 3
ALTER TABLE chunks ADD COLUMN embedding_model TEXT NOT NULL DEFAULT '';
```
```rust
// Index::upsert insert into chunks
"INSERT INTO chunks(id,source_id,ord,char_start,char_end,access,text,embedding_model)
 VALUES(?1,?2,?3,?4,?5,?6,?7,?8)"
// where ?8 = embedder.name()

// Stale reindex query
"SELECT c.rowid, c.source_id, c.text FROM chunks c
 WHERE c.embedding_model != ?1"
```

**Task Breakdown:**
- Migration SQL: Trivial (~30min)
- Update `Index::upsert()` to write model name: Small (~1h)
- Implement `--stale-embeddings-only` on `reindex` command: Medium (~3h)
- Doctor stale chunk count: Small (~1h)
- Tests: Small (~3h)

**Dependencies:** REQ-001.

---

#### REQ-004: Sync-readiness columns (`device_id`, `sync_state` table)

**Description:** Add `device_id TEXT` to `sources` and create a `sync_state (key TEXT PRIMARY KEY, value TEXT NOT NULL)` key-value table. Neither is used by any code in this release — they are schema-only. Their existence means future cloud sync can be implemented without an ALTER against a live user database.

**Acceptance Criteria:**
- [ ] `device_id` column exists on `sources` after migration (nullable, always null in this release).
- [ ] `sync_state` table exists after migration.
- [ ] No code reads or writes either in this release.
- [ ] No CLI output or MCP tool exposes them.
- [ ] `cargo test` asserts both exist after all migrations run.

**Technical Specification:**
```sql
-- Migration 4
ALTER TABLE sources ADD COLUMN device_id TEXT;
CREATE TABLE IF NOT EXISTS sync_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

**Task Breakdown:**
- Migration SQL: Trivial (~30min)
- Test asserting columns/table exist: Trivial (~30min)

**Dependencies:** REQ-001.

---

### Should Have (P1) — Important but Not Blocking

#### REQ-005: `starlee migrate` CLI command

**Description:** A user-facing `starlee migrate` command that runs all pending migrations and reports what was applied. Useful for debugging and for users who want to manually trigger migration before a read-heavy session.

**Acceptance Criteria:**
- [ ] `starlee migrate` runs all pending migrations and prints each one applied: `"Applied migration N: <description>"`.
- [ ] If already at the latest version: prints `"Schema is up to date (version N)."` and exits 0.
- [ ] On failure: prints the failed migration number and error, exits non-zero.

**Task Breakdown:**
- Add `Command::Migrate` arm to `src/main.rs`: Small (~1h)
- Implement output formatting in migration runner: Small (~1h)

**Dependencies:** REQ-001.

---

### Nice to Have (P2) — Future Enhancement

#### REQ-006: Migration dry-run mode

**Description:** `starlee migrate --dry-run` prints what migrations would be applied without running them. Useful for users who want to inspect before upgrading.

**Task Breakdown:**
- Add `--dry-run` flag: Small (~1h)

**Dependencies:** REQ-005.

---

## Non-Functional Requirements

### Performance

- `run_migrations()` on a database already at the latest version must complete in under 50ms (it runs on every startup).
- Each individual migration must complete in under 5 seconds on a database with 10,000 source documents and 100,000 chunks (the realistic upper bound for a personal knowledge base in year one).
- `starlee reindex --stale-embeddings-only` on a corpus where 0% of chunks are stale must complete in under 200ms (skip path: version check only, no embedding work).
- Adding `consumed_at` to `Index::upsert()` must add no measurable latency to individual captures (single column insert, no index change required for MVP).

### Reliability

- A migration failure must leave the database in its exact pre-migration state. No partial writes. Verified by: intentionally inject a SQL error in the middle of a test migration and assert the database is unchanged afterward.
- The migration system must be idempotent: running `run_migrations()` twice against the same database produces the same result as running it once. No duplicate rows in `schema_meta`, no `table already exists` errors.
- WAL mode is already enabled; this must be preserved across all migrations.

### Compatibility

- All four schema changes must be backward-compatible additions (nullable columns, new tables). No existing column is dropped or renamed in this release.
- SQLite version bundled via `rusqlite` feature `bundled` — no system SQLite dependency. All migrations must use only SQLite-compatible SQL (no PostgreSQL-isms).
- The `sqlite-vec` virtual table is already registered; migrations must not drop or recreate `chunk_vectors` or `chunk_fts` except as part of a full rebuild path.

### Observability

- `starlee doctor` must report `schema_version: N`, `chunks_total: N`, `chunks_stale: N` (stale = embedding model mismatch) after these changes.
- Migration output must go to stderr so it doesn't pollute JSON output on stdout for callers that parse `starlee` output programmatically.

---

## Technical Considerations

### System Architecture

**Current architecture:**
- `Index::init()` in `src/index.rs` runs a single `CREATE TABLE IF NOT EXISTS` batch. No version tracking. Called lazily on first database operation.
- `Embedder` trait in `src/embedding.rs` has a `name()` method returning `"BAAI/bge-small-en-v1.5-quantized"` — this is the model identifier that will populate `embedding_model`.
- `CaptureInput` in `src/model.rs` carries all capture metadata through to vault and index.
- HTTP capture handler in `src/http.rs` deserializes the browser extension POST body into `CapturePayload`.

**Proposed changes:**
```
Index::init()  ──▶  run_migrations()  ──▶  schema_meta version check
                                      ──▶  apply pending SQL blocks in order
                                      ──▶  update schema_meta version
```

The migration runner lives in `src/index.rs` alongside the rest of the index logic. Each migration is a `(&str, &str)` tuple: `(version_string, sql)`. This keeps migrations co-located with the schema and avoids file I/O.

### Database Schema — After All Migrations

```sql
-- Core tables (unchanged from current, shown for reference)
CREATE TABLE IF NOT EXISTS sources (
    id           TEXT PRIMARY KEY,
    type         TEXT NOT NULL,
    title        TEXT NOT NULL,
    author       TEXT,
    site         TEXT,
    url          TEXT,
    captured_at  TEXT NOT NULL,
    published_at TEXT,
    access       TEXT NOT NULL,
    summary      TEXT NOT NULL,
    file_path    TEXT NOT NULL,
    -- NEW (migration 2):
    consumed_at  TEXT,
    -- NEW (migration 4):
    device_id    TEXT
);

CREATE TABLE IF NOT EXISTS chunks (
    rowid           INTEGER PRIMARY KEY,
    id              TEXT UNIQUE NOT NULL,
    source_id       TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    ord             INTEGER NOT NULL,
    char_start      INTEGER,
    char_end        INTEGER,
    t_start         REAL,
    t_end           REAL,
    access          TEXT NOT NULL,
    text            TEXT NOT NULL,
    -- NEW (migration 3):
    embedding_model TEXT NOT NULL DEFAULT ''
);

-- NEW (migration 1 — migration system itself):
CREATE TABLE IF NOT EXISTS schema_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- NEW (migration 4 — sync readiness):
CREATE TABLE IF NOT EXISTS sync_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Virtual tables and triggers unchanged
```

### Migration Runner Design

```rust
// src/index.rs
const MIGRATIONS: &[(&str, &str)] = &[
    ("1", "
        CREATE TABLE IF NOT EXISTS schema_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
        -- (initial schema inline here if starting fresh, or applied on top of existing)
    "),
    ("2", "ALTER TABLE sources ADD COLUMN consumed_at TEXT;"),
    ("3", "ALTER TABLE chunks ADD COLUMN embedding_model TEXT NOT NULL DEFAULT '';"),
    ("4", "
        ALTER TABLE sources ADD COLUMN device_id TEXT;
        CREATE TABLE IF NOT EXISTS sync_state(key TEXT PRIMARY KEY, value TEXT NOT NULL);
    "),
];

fn run_migrations(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);"
    )?;
    let current_version: i64 = connection
        .query_row(
            "SELECT COALESCE((SELECT CAST(value AS INTEGER) FROM schema_meta WHERE key='version'), 0)",
            [], |row| row.get(0)
        )?;
    for (version_str, sql) in MIGRATIONS {
        let version: i64 = version_str.parse()?;
        if version <= current_version { continue; }
        let tx = connection.unchecked_transaction()?;
        tx.execute_batch(sql)
            .with_context(|| format!("Migration {version} failed"))?;
        tx.execute(
            "INSERT OR REPLACE INTO schema_meta(key, value) VALUES('version', ?1)",
            [version_str]
        )?;
        tx.commit()?;
    }
    Ok(())
}
```

### Capture Payload Extension

The browser extension currently POSTs `CapturePayload` to `http://127.0.0.1:{port}/capture`. The payload struct in `src/http.rs` / `src/capture.rs` gains a nullable `consumed_at` field. Old extension versions that don't send it will deserialize with `None`, which is correct.

```rust
// src/capture.rs (or http.rs — wherever CapturePayload is defined)
#[derive(Deserialize)]
pub struct CapturePayload {
    // existing fields ...
    #[serde(default)]
    pub consumed_at: Option<String>,
}
```

The browser extension sets `consumed_at: new Date().toISOString()` at the moment the user triggers capture (button click or auto-capture on page completion).

---

## Implementation Roadmap

### Phase 1: Migration Infrastructure (Day 1)
**Goal:** Migration system running; schema at version 1 (or higher if re-numbering existing schema as migration 1).

**Tasks:**
- [ ] Task 1.1: Write `run_migrations()` with `schema_meta` version tracking (~3h)
- [ ] Task 1.2: Define `MIGRATIONS` slice; move existing schema creation into migration 1 (~2h)
- [ ] Task 1.3: Wire `run_migrations()` into `Index::init()` at top, before any other batch (~1h)
- [ ] Task 1.4: Pipe migration log lines to stderr (~30min)
- [ ] Task 1.5: Tests — fresh DB gets all migrations, existing DB skips applied ones, failure rolls back (~3h)

**Validation Checkpoint:** `cargo test` passes; `starlee doctor` shows `schema_version: 1`.

---

### Phase 2: Consumed Date + Embedding Model (Day 2)
**Goal:** Migrations 2 and 3 applied; all capture paths populate both fields correctly.

**Tasks:**
- [ ] Task 2.1: Write migration 2 (`consumed_at` on `sources`) (~30min)
- [ ] Task 2.2: Add `consumed_at` to `CaptureInput`, `Record`, `SearchHit`, `QueryChunk` in `src/model.rs` (~1h)
- [ ] Task 2.3: Thread `consumed_at` through `engine.capture()` → `Index::upsert()` → INSERT (~1h)
- [ ] Task 2.4: Update `CapturePayload` deserialization to accept `consumed_at` from extension (~30min)
- [ ] Task 2.5: Update browser extension to send `consumed_at: new Date().toISOString()` (~30min)
- [ ] Task 2.6: Surface `consumed_at` in `recent`, `status`, and MCP `query_chunks` output (~1h)
- [ ] Task 2.7: Write migration 3 (`embedding_model` on `chunks`) (~30min)
- [ ] Task 2.8: Populate `embedding_model` in `Index::upsert()` from `embedder.name()` (~30min)
- [ ] Task 2.9: Implement `--stale-embeddings-only` on `reindex` (~3h)
- [ ] Task 2.10: Add stale chunk count to `doctor` (~1h)
- [ ] Task 2.11: Tests for both fields (~3h)

**Validation Checkpoint:** Browser extension capture writes `consumed_at`. `starlee doctor` shows `chunks_stale: 0`. `starlee reindex --stale-embeddings-only` on a current corpus reports 0 re-embedded.

---

### Phase 3: Sync Readiness + CLI Polish (Day 3)
**Goal:** Migration 4 applied; `starlee migrate` command exists; all tests pass.

**Tasks:**
- [ ] Task 3.1: Write migration 4 (`device_id` on `sources`, `sync_state` table) (~30min)
- [ ] Task 3.2: Test asserting both exist after all migrations (~30min)
- [ ] Task 3.3: Add `Command::Migrate` and `starlee migrate` output (~1h)
- [ ] Task 3.4: Full integration test — fresh DB, all 4 migrations, verify schema matches spec (~2h)
- [ ] Task 3.5: Run `cargo test` and fix any failures (~1h)

**Validation Checkpoint:** `cargo test` passes. `starlee migrate` on a current database reports `Schema is up to date (version 4)`. `PRAGMA table_info(sources)` includes `consumed_at` and `device_id`. `PRAGMA table_info(chunks)` includes `embedding_model`.

---

### Effort Estimation

| Phase | Tasks | Estimated Hours |
|-------|-------|-----------------|
| Phase 1: Migration infrastructure | 5 | ~9.5h |
| Phase 2: Consumed date + embedding model | 11 | ~13.5h |
| Phase 3: Sync readiness + polish | 5 | ~5h |
| **Total** | **21** | **~28h** |

Risk buffer +15%: **~32h total** (~4 focused working days solo).

---

## Out of Scope

The following are explicitly NOT included in this PRD:

1. **Cloud sync implementation** — `device_id` and `sync_state` are schema-only placeholders. No sync logic, no encryption, no remote endpoint.

2. **Spotify integration** — `spotify_sync_events` table already exists and is out of scope for this PRD. No changes to it.

3. **Embedding model upgrade** — This PRD adds the `embedding_model` column and `--stale-embeddings-only` flag to support future upgrades. It does NOT change the current model (`BAAI/bge-small-en-v1.5-quantized`).

4. **Content-type-aware chunking** — The `chunk_text()` function remains a fixed character window. Semantic or type-aware chunking is a separate PRD.

5. **Multi-user or multi-account support** — This is a single-user local product. `device_id` is a sync helper, not a user identifier.

6. **Database encryption at rest** — Out of scope. The database lives in `~/Starlee/` and inherits macOS filesystem permissions.

7. **Full-text search improvements** — FTS5 configuration, tokenizers, and ranking weight tuning are separate work.

8. **Vault (markdown file) changes** — The dual-storage model (SQLite + vault files) is unchanged. This PRD touches only the SQLite layer.

---

## Open Questions & Risks

### Open Questions

#### Q1: Should migration 1 be the full current schema or just `schema_meta`?
- **Options:**
  - (A) Migration 1 = create `schema_meta` only; remaining tables created by existing `CREATE TABLE IF NOT EXISTS` batch. Simpler refactor.
  - (B) Migration 1 = full schema; `init()` becomes just `run_migrations()`. Cleaner long-term, larger refactor.
- **Recommendation:** Option A for this release — lower risk, existing tests continue to pass without rewrite. Option B in a follow-up.
- **Owner:** Implementer.
- **Impact:** Medium — affects how much existing `init()` code changes.

#### Q2: What should `consumed_at` mean for YouTube captures?
- **Current thinking:** The browser extension sets it at the moment the user triggers capture (e.g. presses the extension button). For YouTube this is "when the user captured the video," not "when they finished watching."
- **Alternative:** Leave null for YouTube and only set it via a future "mark as watched" gesture.
- **Recommendation:** Set it at capture time for now. A "watched" distinction can be a separate field later.
- **Impact:** Low.

---

### Risks & Mitigation

| Risk | Likelihood | Impact | Mitigation | Contingency |
|------|------------|--------|------------|-------------|
| `ALTER TABLE` on SQLite fails if column already exists (e.g. user ran a partial migration manually) | Low | Medium | Wrap each `ALTER` in a `BEGIN`/`ROLLBACK` check; use version gating so it never runs twice | Detect and skip if column already exists via `PRAGMA table_info` |
| `sqlite-vec` virtual table behavior changes on schema version bump | Low | High | Run full search integration test after all migrations; do not touch virtual table DDL | Drop and recreate virtual table as a last resort; requires full re-embed |
| Browser extension update lags behind binary update — old extension sends no `consumed_at` | High | Low | `consumed_at` is nullable and defaults to null on missing field; no breakage | Document in release notes |
| Developer adds a migration without incrementing version string — silent no-op | Medium | Medium | `cargo test` asserts that `MIGRATIONS` slice is sorted and has no duplicate version strings | Code review checklist item |

---

## Validation Checkpoints

### Checkpoint 1: Migration infrastructure
**Before any other changes merge:**
- [ ] `cargo test` passes including new migration tests.
- [ ] `starlee doctor` on a fresh database reports `schema_version: 1` (or whatever the first migration number is).
- [ ] `starlee doctor` on an existing pre-migration database reports the version after migration runs on startup.
- [ ] Intentional SQL error mid-migration leaves database unchanged (test asserts this).

### Checkpoint 2: Consumed date
- [ ] Browser extension capture POST with `consumed_at` field → `starlee recent` shows it.
- [ ] `starlee capture-url <url>` → `starlee recent` shows `consumed_at: null` (CLI capture, no engagement signal).
- [ ] MCP `query` tool returns `consumed_at` in chunk results.

### Checkpoint 3: Embedding model versioning
- [ ] After `cargo build` and a fresh `starlee capture-url`, `SELECT DISTINCT embedding_model FROM chunks` returns `"BAAI/bge-small-en-v1.5-quantized"` (not empty string).
- [ ] `starlee doctor` reports `chunks_stale: 0`.
- [ ] `starlee reindex --stale-embeddings-only` on a current corpus completes in under 200ms and reports 0 chunks re-embedded.

### Checkpoint 4: Sync readiness
- [ ] `PRAGMA table_info(sources)` includes `consumed_at` and `device_id`.
- [ ] `PRAGMA table_info(chunks)` includes `embedding_model`.
- [ ] `SELECT name FROM sqlite_master WHERE type='table'` includes `sync_state` and `schema_meta`.
- [ ] `starlee migrate` on a fully migrated database prints `Schema is up to date (version 4).` and exits 0.

### Checkpoint 5: Full regression
- [ ] `cargo test` passes with no skipped tests.
- [ ] `starlee serve` starts without error against a migrated database.
- [ ] A browser extension capture round-trip (POST → `starlee recent` → `starlee search`) works end-to-end.
- [ ] `starlee doctor` output is clean: no unexpected errors, schema version present, stale chunk count present.

---

## Appendix: Task Breakdown Hints

### Full Task List (Ordered)

**Phase 1 — Migration infrastructure:**
1. `run_migrations()` + `schema_meta` + version read/write (~3h)
2. Define `MIGRATIONS` slice, fold existing schema into migration 1 or keep as Option A (~2h)
3. Wire `run_migrations()` into `Index::init()` (~1h)
4. Stderr logging for migrations (~30min)
5. Tests: fresh DB, upgrade DB, rollback on failure (~3h)

**Phase 2 — Consumed date:**
6. Migration 2 SQL (~30min)
7. Model struct updates (`CaptureInput`, `Record`, `SearchHit`, `QueryChunk`) (~1h)
8. Engine + HTTP handler threading (~1h)
9. Browser extension `consumed_at` send (~30min)
10. CLI output and MCP output updates (~1h)
11. Tests (~2h)

**Phase 2 — Embedding model:**
12. Migration 3 SQL (~30min)
13. `Index::upsert()` writes `embedder.name()` (~30min)
14. `--stale-embeddings-only` flag on `reindex` (~3h)
15. `doctor` stale chunk count (~1h)
16. Tests (~3h)

**Phase 3 — Sync readiness + CLI:**
17. Migration 4 SQL (~30min)
18. Test asserting columns/table exist (~30min)
19. `Command::Migrate` and output (~1h)
20. Full integration test (~2h)
21. Final `cargo test` pass (~1h)

**Total: 21 tasks, ~28 hours.**

### Critical Path
Migration infrastructure (1–5) → consumed_at (6–11) & embedding model (12–16) in parallel → sync readiness + CLI (17–21).

Tasks 6–11 and 12–16 can be worked in parallel once Phase 1 is complete.

---

*This PRD covers only the database layer. Chunking strategy, embedding model selection, search reranking, and cloud sync are separate PRDs.*
