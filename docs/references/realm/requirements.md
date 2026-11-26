---
source: github.com/realm/realm-js (community branch) + web.archive.org (historical Device Sync docs)
source_type: source
fetched: 2026-09-13
---

# Realm — Candidate behaviors observed

All observed — not accepted as hoyadb requirements. Each entry cites the
overview.md/architecture.md/sync-historical.md section it's drawn from.
Sync-related entries are explicitly historical (deprecated product) — see
[sync-historical.md](sync-historical.md).

- **Embedded/owned objects as a first-class schema concept**, distinct from
  both a plain relation and a denormalized blob — lifecycle tied to a
  single parent, no independent primary key. (architecture.md#object-schema)
- **To-many and inverse relationships are native schema constructs**, not
  something the app builds out of queries — the most relationally rich of
  the three references. (architecture.md#relationships)
- **Every reference is inherently live**; explicit observation is opt-in
  only for getting *notified*, not for data staying current — a stronger
  reactivity default than RxDB/WatermelonDB's opt-in-observable-per-query
  model. (architecture.md#live-objects-and-reactivity)
- **Three-tier change listener granularity** (whole-store / query-result
  diff / single-object diff), with structured insertion/deletion/
  modification index diffs rather than a fresh full result set.
  (architecture.md#live-objects-and-reactivity)
- **Reactive notifications have an accepted depth limit** (4 levels of
  nesting) with no fix, only workarounds (flatten schema, manual refresh).
  (architecture.md#live-objects-and-reactivity)
- *(Historical, deprecated)* **Conflict resolution via operational
  transformation** with four fixed rules (deletes win, last-update-wins
  per-property, time-ordered list inserts, primary-key-based identity).
  (sync-historical.md)
- *(Historical, deprecated)* **Counters needed an explicit
  increment/decrement operation**, not a plain value write, to merge
  correctly under concurrent edits — direct evidence pure state-diffing
  can't correctly resolve some conflicts without an operation/intent
  concept. (sync-historical.md)
- *(Historical, deprecated)* **Fixed conflict rules had accepted data-loss
  edge cases** (e.g. concurrently-created nested collections under the same
  parent — only one survives) and internal inconsistency (dictionary key
  deletion followed "last-update-wins," not "deletes-win," unlike every
  other delete). (sync-historical.md)
- *(Historical, deprecated)* **No pluggable conflict resolution** — achieving
  custom merge semantics required remodeling data (e.g. value → list of
  updates) rather than configuring the sync engine, unlike RxDB's/
  WatermelonDB's pluggable conflict handlers. (sync-historical.md)
