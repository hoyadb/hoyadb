---
source: https://pouchdb.com/guides/ (multiple pages, see per-section headers)
source_type: doc-site
fetched: 2026-09-13
---

# PouchDB — Architecture (observed)

Observed, not accepted — this describes PouchDB's structure, not
hoyadb's.

## Document model

Source: guides/documents (fetched 2026-09-13)

- **Schemaless by design, explicitly contrasted with relational**: "you
  store unstructured *documents* rather than explicitly specifying a
  schema with rows, tables, and all that jazz." A document is any JSON
  object. This is the most document-oriented (least relational) data
  model of every source studied so far — even more so than RxDB, which at
  least has a declared JSON Schema per collection.
- **`_id`** is mandatory on every document, serving as its primary key
  (`put()` requires it; `get()` retrieves by it).
- **`_rev`** is server/PouchDB-assigned, changes on every create/update
  (e.g. `1-bea5fa18...` → `2-...`), and must be presented on the *next*
  update for it to succeed — an update without the current `_rev` fails
  with an HTTP `409` conflict. This is the same
  optimistic-concurrency-via-version-token shape as RxDB's revision check
  (see [../rxdb/architecture.md](../rxdb/architecture.md#revisions-conflict-handlers-and-transactions)),
  and is in fact very likely RxDB's direct ancestor for that mechanism,
  given PouchDB's historical role as RxDB's original storage adapter.

**Relevance to hoyadb**: confirms the `_id` + version-token
(`_rev`/revision) pattern as effectively an industry baseline for optimistic
concurrency in this whole space — now traced across RxDB, WatermelonDB,
PouchDB, and (implicitly) most others via their own version/timestamp
columns. The complete absence of any schema concept is the clearest
document-only data point in this reference set, useful as the far end of
the "how relational should hoyadb be" spectrum already being tracked in
[../comparison.md](../comparison.md) — PouchDB is the "not at all" anchor
point WatermelonDB/Realm/Zero can be compared against.
