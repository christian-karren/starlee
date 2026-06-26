# PRD: End-to-End-Encrypted Multi-Device Sync

**Author:** Christian Karren
**Date:** 2026-06-25
**Status:** Draft (implementation in progress)
**Version:** 1.0
**Quality-Validated:** Yes (prd-writer: EXCELLENT, 57/57)

---

## Implementation Status (updated 2026-06-26)

Scope decisions locked: **cross-platform** target (neutral encrypted object store;
plan for Windows/Linux/iOS, not iCloud-only), **multi-tenant product** (accounts +
per-user keys), **passphrase + one-time recovery code** key model.

| Requirement | Status | Notes |
|-------------|--------|-------|
| REQ-001 stable content-addressed IDs | ✅ Done | `src/identity.rs`, URL normalization + content hash |
| REQ-002 VaultBackend trait | ✅ Done | `src/vault_backend.rs`; `Vault` API unchanged |
| REQ-003 relative file_path | ⏭️ Descoped | `file_path` is never synced (not in canonical markdown) and desktop "Reveal in Finder" needs absolute paths; relative storage added risk for ~no portability gain |
| REQ-004 key hierarchy | ✅ Done | `src/sync/crypto.rs` — Argon2id + XChaCha20-Poly1305 key wrap |
| REQ-005 per-record encryption | ✅ Done | `src/sync/crypto.rs` — HKDF per-record keys, AEAD blobs, opaque object keys |
| REQ-006 encrypted manifest | ⬜ Pending | Needs transport |
| REQ-007 conflict resolution | ⬜ Pending | Needs manifest |
| REQ-008 zero-knowledge server | ⬜ Pending | Needs deployed multi-tenant backend + infra decisions |
| REQ-009 background sync / offline queue | ⬜ Pending | Needs server |
| REQ-010 macOS sync status UI | ⬜ Pending | Needs sync engine |
| REQ-011 selective sync | ⬜ Pending | P2 |
| Phase 5 iOS | ⬜ Gated | Blocked on CoreML BGE-small embedding spike |

Phases 1 (portability foundation) and 2 (crypto core) are complete and tested
locally. Phases 3+ require a deployed server and platform work and are tracked
below as written.

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

---

## Executive Summary

Starlee is a local-first digital brain where Markdown in `vault/{year}/{id}-{slug}.md`
is canonical and the SQLite index (`index.db`) is a disposable cache rebuilt from
the vault. That design protects paywalled and restricted content by keeping captured
bodies on one machine — but it also strands the user's brain on that one machine, so
a work computer and a home computer hold two divergent vaults and a future iOS app
has nothing to read. This PRD specifies a zero-knowledge sync layer: each device
encrypts its vault into per-file blobs with a key derived from a user passphrase,
uploads those blobs plus a small manifest to a multi-tenant object store the server
cannot decrypt, and every other device pulls, decrypts locally, and rebuilds its own
index. The vault becomes "the same brain everywhere"; the index stays per-device and
disposable. Target outcome: a capture made on one enrolled device is decryptable and
searchable on a second enrolled device within 60 seconds, with the server provably
unable to read any captured body.

---

## Problem Statement

### Current Situation

Starlee writes captures as Markdown into a single local vault root and indexes them
into a local `index.db`. Invariants (per `docs/architecture.md`): Markdown is
canonical, the index can always be rebuilt, captured bodies stay local. Two facts
blocked portability and are now addressed:

- **`file_path` was stored absolute** — resolved via the `VaultBackend` abstraction
  (REQ-002); `file_path` is intentionally kept absolute for external consumers since
  it is never synced (REQ-003 descoped).
- **Record IDs were timestamp-derived, so the same content forked** — fixed by
  content-addressed IDs (REQ-001).

### User Impact

- **Who:** every user with more than one computer, plus prospective iOS users.
- **How:** a capture saved at work is invisible at home; a future iOS app cannot
  launch without a vault to read.
- **Severity:** High — multi-device continuity is table stakes for a "second brain."

### Business Impact

The single-device ceiling is the primary blocker to a production product and the iOS
roadmap. Sync converts Starlee's local-first design from a limitation into the
differentiator: "your data and inference stay yours," now portable.

### Why Solve This Now?

The architecture is positioned for it: Markdown is already canonical and the index
disposable, so sync is a transport-and-identity problem, not a data rewrite. The
identity fix (REQ-001) is cheap pre-launch and migration-grade expensive later.

---

## Goals & Success Metrics

### Goal 1: Cross-device capture propagation
- **Metric:** median propagation latency, capture on A → query-hit on B (both online).
- **Baseline:** impossible today.
- **Target:** median ≤ 60s; p95 ≤ 180s.
- **Timeframe:** end of Phase 3.
- **Method:** two headless clients against a staging object store.

### Goal 2: Zero-knowledge guarantee
- **Metric:** fraction of stored objects/fields readable without the user's key.
- **Target:** 0 readable bytes server-side; automated audit, 0 plaintext hits.
- **Timeframe:** end of Phase 2 (crypto core), re-verified each release.
- **Method:** plaintext-leak audit (mirrors the bundle restricted-body audit). A unit
  test (`ciphertext_leaks_no_plaintext_or_id`) already enforces this on blobs.

### Goal 3: Index reconstruction from synced vault
- **Metric:** cold-start rebuild time for 5,000 records; correctness vs origin.
- **Target:** ≤ 10 min; ≥ 95% top-5 query overlap on a 20-query set.
- **Timeframe:** end of Phase 4.

### Goal 4: Conflict-free convergence
- **Metric:** duplicate-record rate after bidirectional sync of overlapping captures.
- **Baseline:** 100% duplication under the old ID scheme.
- **Target:** ≤ 1% over a 500-capture overlap test (enabled by REQ-001 IDs).
- **Timeframe:** end of Phase 3.

---

## User Stories

### Story 1: Enroll a second device
**As a** user with a vault on my work Mac, **I want to** enroll my home computer with
my passphrase, **so that** I access the same brain on both.

**Acceptance Criteria:**
- [ ] User enters account email + passphrase on the new device.
- [ ] The device derives the vault key locally; the passphrase is never transmitted.
- [ ] On success it pulls the encrypted manifest and downloads blobs.
- [ ] Progress shows records downloaded / total and index-rebuild status.
- [ ] A wrong passphrase yields a decryption failure (distinct from a network error) and 0 records written.

### Story 2: Capture on one device, find on another
**As a** user with two enrolled devices, **I want** a capture at work to appear at
home automatically, **so that** I never think about which machine holds what.

**Acceptance Criteria:**
- [ ] After a capture commits on A, its blob + manifest entry upload without user action.
- [ ] B detects the manifest change on its next tick (≤ 60s online) and downloads the blob.
- [ ] B decrypts, writes the Markdown, and upserts it into its local index.
- [ ] A query on B returns the new capture after sync.
- [ ] If B is offline, it syncs on its next online tick with no data loss.

### Story 3: Recover after losing all devices
**As a** user whose laptop was wiped, **I want to** recover with my passphrase or a
recovery code, **so that** I don't permanently lose my brain.

**Acceptance Criteria:**
- [ ] At setup, a one-time recovery code is shown and confirmed before sync activates.
- [ ] Correct passphrase on a new device reconstructs the vault key.
- [ ] The recovery code reconstructs the vault key if the passphrase is forgotten.
- [ ] Losing both is reported as unrecoverable (server cannot reset it).
- [ ] Changing the passphrase re-wraps the key without re-encrypting blobs.

*(Crypto for this story is implemented in `sync::crypto`: `enroll`,
`unwrap_with_passphrase`, `unwrap_with_recovery`, `change_passphrase`.)*

### Story 4: Share a bundle without exposing the synced vault
**As a** user sharing part of my brain, **I want** the existing public-only share
bundle to keep working alongside sync. The `Access::Public/Restricted` split governs
bundles only; the whole vault syncs encrypted regardless of access level.

---

## Functional Requirements

### Must Have (P0)

#### REQ-001: Stable content-addressed record identity — ✅ Done
ID derived from the canonical key (normalized URL, else title+body hash), never from
time. `src/identity.rs`. Same URL on any device → identical ID.

#### REQ-002: Vault storage backend abstraction — ✅ Done
`VaultBackend` trait + `LocalFsBackend` over root-relative `VaultPath`. `src/vault_backend.rs`.
`Vault`'s public API unchanged.

#### REQ-003: Vault-relative file paths — ⏭️ Descoped
`file_path` lives only in the runtime `Record` and disposable `index.db`, never in
canonical markdown, so it never travels between machines; the desktop "Reveal in
Finder" requires absolute paths. Relative storage added risk for ~no portability gain.

#### REQ-004: Passphrase-derived key hierarchy — ✅ Done
Random 256-bit master key wrapped under passphrase-KEK and recovery-KEK (Argon2id
64 MiB/t=3 + XChaCha20-Poly1305). Server stores only wrapped copies + salts + params.
`src/sync/crypto.rs`.

#### REQ-005: Per-record blob encryption — ✅ Done
Per-record keys via HKDF-SHA256(master, record_id); XChaCha20-Poly1305 with record_id
as AAD; opaque base32 object keys. Fails closed on tamper. `src/sync/crypto.rs`.

#### REQ-006: Encrypted sync manifest — ⬜ Pending
Per-account manifest `record_id → {object_key, content_hash, logical_version, deleted}`,
itself encrypted; server exposes a monotonic version for delta computation without
decrypting. Tombstones propagate deletions.

#### REQ-007: Conflict resolution and convergence — ⬜ Pending
Last-writer-wins by logical clock; materially-different losers preserved as conflict
siblings; order-independent convergence. Dedup ≤ 1% via REQ-001 IDs.

#### REQ-008: Multi-tenant zero-knowledge sync server — ⬜ Pending
Account auth scoped per namespace; stores only ciphertext (wrapped keys, manifest,
blobs); S3/R2-compatible `ObjectStore` interface; server never holds an unwrapped key.

### Should Have (P1)

#### REQ-009: Background sync scheduler with offline queue — ⬜ Pending
Sync on capture + ≤60s interval; durable offline queue with exponential backoff;
no capture lost across offline→online; status via `starlee doctor`.

#### REQ-010: Sync status surfacing in macOS app — ⬜ Pending
Menu-bar/status-window synced/syncing/error state; last-success time; pending count.

### Nice to Have (P2)

#### REQ-011: Selective / partial sync — ⬜ Pending
Date/type filters to bound local storage on constrained devices (iOS).

---

## Non-Functional Requirements

- **Performance:** propagation median ≤60s/p95 ≤180s; per-capture upload overhead ≤1.5s p95 (50KB/10Mbit); rebuild ≤10min/5,000 records; manifest diff ≤500ms/10,000 records.
- **Security:** Argon2id ≥64 MiB/≥3 iters/≈≥250ms; AEAD with unique nonce, fail-closed; 0 readable server-side bytes (CI audit); master key never leaves a device unwrapped; cross-account isolation 100%; TLS 1.3.
- **Reliability:** 0 lost captures offline→online; order-independent convergence; server uptime ≥99.5%/month; idempotent uploads.
- **Scalability:** 100,000 records / 5GB per account within the diff budget; stateless server behind object store; encrypted manifest <5MB at 100k records (shard above).
- **Compatibility:** macOS 13+, Windows 10+, Linux x86-64; iOS 16+ (Phase 5); any S3-compatible store (R2 + S3); no-sync vault behaves exactly as today.

---

## Technical Considerations

### System Architecture

`engine.rs` orchestrates capture → `Vault` (now via `VaultBackend`) → `Index`
(FTS5 + sqlite-vec). A `sync` module adds crypto (done) + manifest + scheduler and
talks to a zero-knowledge server via an object-store abstraction. The index stays
per-device and is always rebuilt locally — never synced.

```
   Device A (work)                 Zero-knowledge server          Device B (home)
 ┌────────────────────┐           ┌───────────────────┐        ┌────────────────────┐
 │ engine ─ VaultBackend          │ account auth      │        │ engine ─ VaultBackend
 │   ├ LocalFsBackend │           │ manifest (cipher) │        │   ├ LocalFsBackend │
 │   └ SyncBackend ───┼──encrypt─▶│ blobs (cipher)    │◀───────┼── SyncBackend      │
 │ index.db (local)   │  blobs +  │ wrapped keys      │  pull  │ index.db (local,   │
 │ (rebuilt locally)  │  manifest │ (R2 / S3)         │ decrypt│  rebuilt locally)  │
 └────────────────────┘           └───────────────────┘        └────────────────────┘
            ▲                         server cannot decrypt
            │ master_key = unwrap(passphrase) — never leaves device
```

### Migration Strategy
Existing single-device vaults migrate IDs (REQ-001) before sync is enabled. Because
Markdown is canonical and the index disposable, the migration's worst case is
"rebuild the index," already supported.

### Testing Strategy
- **Unit:** crypto vectors (done: 17 tests), URL normalization + ID determinism (done), manifest diff (pending).
- **Integration:** two-client propagation, cold-start rebuild, dedup/overlap, offline→online (pending — need server).
- **Security:** server-side plaintext-leak audit (blob-level done), cross-account isolation (pending).

### iOS Feasibility Note (Phase 5 gate)
The Rust core compiles to an iOS static lib; the gating risk is on-device embedding
(BGE-small via ONNX is unvalidated on iOS). Phase 5 is gated on a CoreML spike:
success = cosine ≥0.99 parity vs desktop + ≤50ms/chunk on an A16-class device.

---

## Implementation Roadmap

- **Phase 1 — Portability foundation:** REQ-001 ✅, REQ-002 ✅, REQ-003 descoped.
- **Phase 2 — Crypto core:** REQ-004 ✅, REQ-005 ✅, plaintext-leak audit ✅ (unit-level).
- **Phase 3 — Sync engine + server:** REQ-006, REQ-007, REQ-008. *(Requires a deployed multi-tenant backend; infra/provider decisions open — see Q1–Q3.)*
- **Phase 4 — Resilience + UX:** REQ-009, REQ-010, cold-start verification, recovery flows end-to-end.
- **Phase 5 — iOS:** gated on the embedding spike.

---

## Out of Scope
1. Real-time collaborative editing. 2. Server-side search/indexing (violates zero-knowledge). 3. Plaintext cloud backup / web reader. 4. Changes to the existing public-only share bundle. 5. SSO/enterprise identity. 6. Android. 7. Cross-account vault merging.

---

## Open Questions & Risks

### Open Questions
- **Q1 — Object-store provider/region:** R2 (no egress) vs S3 vs both via the `ObjectStore` trait. Owner: founder/infra. Before Phase 3.
- **Q2 — Manifest scaling beyond 100k records:** shard by year vs chunked manifest vs defer. Before Phase 3 design freeze.
- **Q3 — Account auth provider:** email+password vs magic link vs OAuth (independent of the vault key). Phase 3.
- **Q4 — iCloud as Apple-only fast path:** deferred; the `ObjectStore` trait can host a CloudKit impl later.

### Risks
- Lost passphrase **and** recovery code → unrecoverable by design (forced recovery-code confirmation at setup; documented, no false reset promise).
- Crypto flaw → vetted libraries only (Argon2id, XChaCha20-Poly1305), no custom primitives; external review before launch.
- Cross-device duplication >1% → content-addressed IDs + URL normalization; manual merge tool fallback.
- On-device iOS embedding infeasible → Phase 5 gated on spike; read-only/deferred contingency.

---

## Validation Checkpoints

- **Checkpoint 1 (Phase 1):** existing vault migrates with 0 record loss; same URL → identical IDs; all prior tests pass. ✅
- **Checkpoint 2 (Phase 2):** vault round-trips encrypt→decrypt byte-identical; tamper fails closed; plaintext-leak audit = 0; wrong passphrase distinguishable. ✅ (unit-level)
- **Checkpoint 3 (Phase 3):** median propagation ≤60s; dup ≤1%; convergence order-independent; cross-account 403 in 100% of tests. ⬜
- **Checkpoint 4 (Phase 4):** cold-start ≤10min/5,000 records, ≥95% top-5 overlap; 0 lost captures offline→online; recovery restores a wiped device. ⬜
- **Checkpoint 5 (iOS gate):** on-device BGE-small parity ≥0.99 + ≤50ms/chunk. ⬜

---

**End of PRD**
