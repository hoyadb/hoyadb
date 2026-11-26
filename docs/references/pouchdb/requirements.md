---
source: https://pouchdb.com/guides/ (multiple pages — see overview.md/architecture.md/sync.md for per-page sources)
source_type: doc-site
fetched: 2026-09-13
---

# PouchDB — Candidate behaviors observed

All observed — not accepted as hoyadb requirements. Each entry cites the
overview.md/architecture.md/sync.md section it's drawn from.

- **Fully schemaless document model** — the least relational data model of
  any source in this reference set, explicitly contrasted with "rows,
  tables, and all that jazz." (architecture.md#document-model)
- **`_id` + server-assigned `_rev` version token**, required on every
  update, with a `409` on a stale `_rev` — an early/ancestral form of the
  same optimistic-concurrency pattern seen in RxDB and (likely,
  historically) inherited from PouchDB by RxDB directly.
  (architecture.md#document-model)
- **Multi-master replication**: no node is authoritative by design; any
  peer can replicate with any other peer, in any topology (cache↔local,
  many-remotes↔one-local, local↔backup), not just client↔server.
  (sync.md#one-way-replication-vs-two-way-sync, sync.md#multi-master-architecture)
- **Bidirectional "sync" is just two one-way replications**, not a
  structurally distinct mechanism. (sync.md#one-way-replication-vs-two-way-sync)
- **Live replication with automatic reconnection** (`live: true, retry:
  true`), exposing explicit `paused`/`active` connectivity-state events.
  (sync.md#one-way-replication-vs-two-way-sync)
- **Conflicts resolved via a deterministic, coordination-free winner-pick
  over a revision tree — but the losing revision is preserved and
  app-retrievable**, not silently discarded. A genuinely distinct third
  conflict philosophy vs. "auto-merge" and "auto-replace-no-trace."
  (sync.md#conflict-resolution-revision-trees)
- **Three named app-level conflict-handling patterns**: manual
  inspect-and-remove, a delta-function "upsert" that avoids the conflict
  ever occurring, and an append-only "ledger" design that makes update
  conflicts structurally impossible by never updating anything.
  (sync.md#conflict-resolution-revision-trees)
- **Local reactivity and remote sync appear to share one underlying
  primitive** (the changes feed) — flagged as a strong inference from
  near-identical documented mechanics, not a confirmed architectural claim
  from what was fetched. (sync.md#reactivity-the-changes-feed)
