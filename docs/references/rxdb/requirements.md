---
source: https://rxdb.info/ (multiple pages — see architecture.md/overview.md for per-page sources)
source_type: doc-site
fetched: 2026-09-13
---

# RxDB — Candidate behaviors observed

All observed — not accepted as hoyadb requirements. Each entry cites the
architecture.md/overview.md section it's drawn from.

- **Pluggable storage per platform.** A database created against one narrow
  storage interface can run on different concrete engines (IndexedDB in
  browser, SQLite on mobile/desktop, in-memory, etc.) without changing
  app-facing code. (architecture.md#storage-the-rxstorage-interface)
- **Live query subscriptions with a one-shot escape hatch.** The same query
  object supports both a one-time read (Promise) and a live subscription
  (Observable) that re-emits on relevant underlying changes.
  (architecture.md#query-layer-rxquery)
- **Incremental live-query updates, not full recompute.** An algorithm
  (EventReduce) updates an already-observed result set based on the write
  that happened, rather than re-running the query from scratch on every
  write. (architecture.md#query-layer-rxquery)
- **Reactive primitive is swappable per UI framework.** The default
  observable type (RxJS) can be swapped for Signals/refs/state-library
  bindings via a factory, without changing how documents/queries are
  observed conceptually. (architecture.md#reactivity-model-general)
- **Observation exists at multiple granularities**: single field, whole
  document, query result set, collection-wide events.
  (architecture.md#reactivity-model-general)
- **Sync protocol is defined independently of transport.** Pull/push
  handlers exchange JSON and checkpoints; any transport that can move JSON
  can carry it. (architecture.md#replication-protocol)
- **Sync unit is document state, not a command/intent.** Push sends
  before/after state pairs; pull returns raw documents since a checkpoint.
  No operation log or command concept observed in the protocol.
  (architecture.md#replication-protocol)
- **Conflict detection is a state-equality check gated by a revision
  token**, not a semantic/business-logic check — a write is rejected if the
  revision it was based on is stale, then a pluggable conflict handler
  merges/chooses between the two states. (architecture.md#revisions-conflict-handlers-and-transactions)
- **No cross-document transactions**, by explicit design choice, to keep
  offline writes uncoordinated/non-blocking.
  (architecture.md#revisions-conflict-handlers-and-transactions)
