---
source: https://zero.rocicorp.dev/ (multiple pages — see overview.md/architecture.md/sync.md for per-page sources)
source_type: doc-site
fetched: 2026-09-13
---

# Zero — Candidate behaviors observed

All observed — not accepted as hoyadb requirements. Each entry cites the
overview.md/architecture.md/sync.md section it's drawn from.

- **Sync unit is a named, validated command (mutator) with arguments**, not
  a data diff — the closest match across all sources studied to hoyadb's
  `add-product-to-basket`-style preference. (sync.md#mutators...)
- **Optimistic client execution + authoritative server re-execution of the
  same named function**, with the client's local effect discarded/replaced
  once the server's own execution result arrives — not merged with it.
  (sync.md#mutators...)
- **A server-controlled context object, separate from client-supplied
  arguments**, explicitly recommended for trusted values like the
  authenticated user id that the client cannot override. (sync.md#mutators...)
- **Client and server may run different implementations of the same named
  operation** (both mutators and queries) — the server version can add
  permission/audit logic the client version skips. (architecture.md#queries...,
  sync.md#mutators...)
- **Sync is scoped to the union of currently active queries**, not a full
  replicated dataset — client holds "a consistent partial replica," with
  per-query TTLs governing how long synced data survives after a query
  deactivates. (architecture.md#queries...)
- **The authoritative store is an ordinary Postgres database** accessed via
  the same query language (ZQL) client and server both use — no bespoke
  sync-only backend store, and no baked-in sync-tracking columns on every
  table (contrast with WatermelonDB's `_status`/`_changed`).
  (sync.md#server-side...)
- **Schema is declared against an existing external database** (with
  column/schema remapping via `.from()`), rather than the sync engine
  owning a canonical schema of its own. (architecture.md#schema...)
- **Native to-many and many-to-many relationships** at the schema level
  (with a stated 2-level many-to-many chaining limit), not just
  query-pattern conventions. (architecture.md#schema...)
- **Conflict resolution is (at least partly) sidestepped rather than
  merged**: because the sync unit is a re-runnable function, the server's
  fresh execution is simply authoritative — no document/field merge step
  was found described for the mutator path. Flagged as not fully verified
  in this pass — see sync.md's "Conflict resolution" section for the exact
  quote and its limits.
