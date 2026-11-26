---
source: https://github.com/Nozbe/WatermelonDB (multiple pages — see overview.md/architecture.md/sync.md for per-page sources)
source_type: source
fetched: 2026-09-13
---

# WatermelonDB — Candidate behaviors observed

All observed — not accepted as hoyadb requirements. Each entry cites the
overview.md/architecture.md/sync.md section it's drawn from.

- **Lazy loading as the core performance strategy.** Nothing loads into JS
  until explicitly queried/fetched, specifically to keep app launch fast
  regardless of local data volume. (overview.md)
- **Genuinely relational local storage** (SQLite), not a document store with
  relations layered on — contrast with RxDB. (overview.md,
  architecture.md#schema--data-model)
- **Sync bookkeeping is baked into the schema itself** (`_status`,
  `_changed` reserved columns on every table), not a separate subsystem.
  (architecture.md#schema--data-model)
- **Relation loading is lazy and dual-mode** (Promise fetch vs. Observable
  observe), matching the query-level duality.
  (architecture.md#relations, architecture.md#query-execution-and-observing)
- **Only belongs-to is a first-class relation construct**; has-many and
  many-to-many are conventions built from queries + (for many-to-many) a
  manually-modeled pivot table. (architecture.md#relations)
- **Query observation defaults to set-membership changes only**; per-field
  change observation on already-matched records is a separate, more
  expensive opt-in (`observeWithColumns`). Count observation is throttled
  by default. (architecture.md#query-execution-and-observing)
- **Sync unit is create/update/delete on entities**, batched per table, not
  a command/intent/operation log. (sync.md#frontend..., sync.md#backend...)
- **Sync contract is a strict bidirectional MUST/SHOULD spec**, not just a
  suggested shape — idempotent retries, transactional push, explicit
  handling of every "record already exists/already deleted/no longer
  exists" edge case. (sync.md#backend...)
- **Conflict resolution is per-column, content-based**: take the remote
  record wholesale, then re-apply only the specific local columns changed
  since last sync — distinct from whole-document last-write-wins.
  (sync.md#sync-algorithm-internals)
- **Server only needs a per-record modified-timestamp/version**; all
  detailed change tracking (`_status`/`_changed`, per-column diffing) is
  client-side only. (sync.md#backend..., sync.md#sync-algorithm-internals)
- **Sync is designed to be crash/interruption-safe by construction**: any
  step can fail partway and the next sync attempt still converges, without
  a special recovery path. (sync.md#sync-algorithm-internals)
- **Schema changes require an explicit version bump + migration**; without
  one, a schema change wipes local data. Sync additionally needs to know a
  client's schema-version-at-last-sync to backfill data from newly-added
  tables/columns after a migration. (architecture.md#schema--data-model,
  sync.md#frontend...)
- **A last-resort non-incremental "replacement sync"** exists for cases an
  incremental diff can't be computed server-side (e.g. complex permission
  changes), at a real performance cost. (sync.md#frontend...)
