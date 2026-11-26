---
source: https://vlcn.io/docs/cr-sqlite/ + github.com/vlcn-io/cr-sqlite README — see overview.md/architecture.md/sync.md for per-page sources
source_type: doc-site
fetched: 2026-09-13
---

# cr-sqlite — Candidate behaviors observed

All observed — not accepted as hoyadb requirements. Each entry cites the
overview.md/architecture.md/sync.md section it's drawn from.

- **CRDT-ness is opt-in per table** (`crsql_as_crr`) on top of an ordinary
  SQLite table — the only source in this set adding CRDT semantics
  directly at the SQL layer of a real relational engine, rather than a
  document store or a general-purpose CRDT library. (overview.md)
- **Per-table CRDT type controls row survival on merge**; default
  ("causal length set") allows a deleted row to be resurrected by a later
  re-insert — a real exception to the "deletes always win" convention seen
  everywhere else in this reference set. (overview.md)
- **Per-column CRDT type, defaulting to numeric-tiebreak LWW register**,
  with a distinct Fractional Index type for conflict-free manual ordering.
  Counter CRDT support status is **disputed between the project's own two
  primary sources** (docs site says unsupported, GitHub README says
  supported) — flagged, not resolved. (architecture.md#column-level-crdts,
  sync.md)
- **Fully automatic conflict resolution, no manual merge hook** — the
  opposite pole from RxDB/WatermelonDB/PowerSync's pluggable
  conflict-handler approach. (sync.md)
- **Sync bookkeeping is exposed as queryable SQL** (`crsql_changes`,
  `crsql_tracked_peers`, `crsql_site_id`), not hidden internal state —
  distinctive relative to every other source's approach to change tracking.
  (sync.md#whole-crr-sync-protocol)
- **The changeset relation itself is the wire format** — no separate
  serialization step between "the local change log" and "what gets sent to
  a peer." (sync.md#whole-crr-sync-protocol)
- **Reactivity today tops out at single-row granularity**
  (table/row/whole-db subscriptions); query-result-level reactivity is
  explicitly future work, not yet shipped — the coarsest granularity found
  across all sources studied, and a useful signal that query-level
  reactivity is nontrivial engineering effort.
  (architecture.md#reactivity-js-bindings)
