---
source: https://vlcn.io/docs/cr-sqlite/crdts/ (multiple pages — see per-section headers)
source_type: doc-site
fetched: 2026-09-13
---

# cr-sqlite — Architecture: CRDT model (observed)

Observed, not accepted — this describes cr-sqlite's structure, not
hoyadb's.

## Column-level CRDTs

Source: cr-sqlite/crdts/column-crdts (fetched 2026-09-13)

- Each column can be assigned its own CRDT type controlling how concurrent
  writes to that column merge — a per-column choice, layered under the
  per-table CRDT-type choice from [overview.md](overview.md).
- **Last-Write-Wins register is the default for every column**: "all
  columns in a `crr` are treated as last write wins registers" unless
  configured otherwise — same conceptual default as PowerSync's per-field
  LWW (see [../powersync/sync.md](../powersync/sync.md#conflict-handling-default-behavior)),
  arrived at independently. Simultaneous writes are broken by numerically
  comparing the values themselves (largest wins) — a concrete, fully
  deterministic tiebreak not seen specified this precisely in any other
  source.
- **Fractional Index** is a distinct, named alternative CRDT type for
  columns holding a manually-maintained sort order — dividing the
  numeric space between existing rows to let concurrent inserts/reorders
  from different devices interleave without central coordination. The
  docs note it handles precision-overflow and conflicting-assignment edge
  cases automatically.
- **Counter CRDTs are explicitly unsupported** ("not yet supported") —
  directly notable given Realm's historical Device Sync required a special
  increment/decrement operation specifically to make counters merge
  correctly (see
  [../realm/sync-historical.md](../realm/sync-historical.md)). cr-sqlite's
  purely value-based LWW/state-CRDT model has no operation/intent concept
  to lean on, so — consistent with that finding — it can't yet solve the
  same problem Realm's OT-based approach handled via an explicit operation.
- No customization mechanism beyond picking from the available per-column
  types was found on this page.

**Relevance to hoyadb**: cr-sqlite's counter gap is a second, independent
confirmation of the same structural limit Realm's historical docs
surfaced: **pure state/value-based CRDTs cannot correctly merge counters
without an explicit operation concept** — reinforcing that this is a real,
recurring edge case (now seen in two unrelated systems) rather than a
one-off design quirk, and further strengthening the case for hoyadb's
command-based sync direction being able to solve something entity-diff/
value-CRDT approaches structurally can't.

## Reactivity (JS bindings)

Source: cr-sqlite/js/reactivity (fetched 2026-09-13)

- The `@vlcn.io/rx-tbl` package wraps a connection (`tblrx()`) and exposes
  three subscription granularities, built on SQLite's native update hooks:
  `onRange(["table1", ...], cb)` (table-level — fires on any change to any
  of the named tables), `onPoint(rowid, cb)` (single-row, keyed by SQLite's
  internal `rowid`, not necessarily the primary key — the docs note these
  coincide only "for tables that have an integer primary key column"), and
  `onAny(cb)` (whole-database, unfiltered). Every subscription returns a
  dispose function. Callbacks receive an array of `UpdateType` values
  (insert/update/delete).
- **No query-result-level reactivity today** — the finest built-in
  granularity is single-row, not "this query's result set changed." The
  docs state this is a known gap with a named future plan: "rx-query will
  be implemented as a native extension ... such that it is available on
  all platforms" — i.e. query-level live subscriptions are roadmap, not
  yet shipped, as of this fetch.

**Relevance to hoyadb**: this is the **coarsest** reactivity granularity of
any source in this reference set — table/row/whole-db, with query-result
reactivity explicitly still unbuilt. Every other source studied (RxDB,
WatermelonDB, Realm, Zero, PowerSync) treats "observe a specific query's
result set" as a base-level, already-shipped feature. Useful negative data
point: it suggests query-level reactivity is a genuinely harder engineering
problem than table/row-level (cr-sqlite has had CRDT merge semantics
working for a while but still lists this as future work), so hoyadb should
budget real effort for it rather than assume it's a thin layer over
row-level change events.
