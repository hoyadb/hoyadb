---
source: https://docs.powersync.com/ (multiple pages — see overview.md/architecture.md/sync.md for per-page sources)
source_type: doc-site
fetched: 2026-09-13
---

# PowerSync — Candidate behaviors observed

All observed — not accepted as hoyadb requirements. Each entry cites the
overview.md/architecture.md/sync.md section it's drawn from.

- **A dedicated, centralized sync service sits between client and backend
  database**, rather than sync logic being a library the backend author
  wires up directly. (overview.md)
- **Declarative, server-computed sync scoping via "buckets"**: a client
  syncs whatever its parameter queries resolve to, independent of which
  queries the UI currently has mounted — with an explicit, named scale cap
  (1,000 buckets/client by default). (architecture.md#sync-scoping...)
- **Upload queue captures PUT/PATCH/DELETE with minimal payload per
  operation type** (full row / changed columns only / id only) — converges
  independently with WatermelonDB's wire shape.
  (sync.md#upload-queue-crud-operation-shape)
- **Server-side write processing must be synchronous** relative to
  checkpoint advancement, with a named failure mode (temporarily
  "disappearing" synced data) if violated.
  (sync.md#upload-queue-crud-operation-shape)
- **Validation/conflict errors are reported out-of-band (2xx + error
  detail), not as hard transport failures** — only genuinely transient
  backend failures use 5xx/retry. (sync.md#upload-queue-crud-operation-shape)
- **Default conflict resolution is per-field last-write-wins, with deletes
  always winning** — conflict handling isn't part of PowerSync itself, it
  falls out of how the app-authored backend endpoint is written.
  (sync.md#conflict-handling-default-behavior)
- **Arbitrary per-write metadata/context can be attached to a queued
  operation** (`trackMetadata`/`_metadata`) and read by the backend to
  change how it processes that write — the clearest working precedent
  across the whole reference set for "carry context alongside a synced
  write" without going fully command-based. (sync.md#custom-conflict-resolution)
- **Conflict resolution customization is entirely backend-side** — no
  client-side conflict hook exists; the backend's upload handler decides
  everything (timestamp/version comparison, business-rule checks, or
  recording conflicts for manual resolution).
  (sync.md#custom-conflict-resolution)
- **Three explicit, opt-in tiers of query reactivity** (full re-emit /
  diff-gated re-emit / explicit added-removed-updated delta) — the third
  independent source (after RxDB, Realm) to treat incremental-update-vs-
  full-recompute as a named, first-class API concern.
  (architecture.md#reactivity-three-tiers-of-watch-query)
