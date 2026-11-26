---
source: https://pouchdb.com/guides/
source_type: doc-site
fetched: 2026-09-13
updated: 2026-09-13
status: done
---

# Import: pouchdb

PouchDB is a long-standing (pre-dates every other source in this reference
set) JS embedded database implementing the **CouchDB replication
protocol** — multi-master, revision-tree-based sync where conflicts are
detected deterministically but surfaced to the app rather than silently
auto-merged. Historically influential: RxDB's early versions were built on
top of PouchDB as a storage adapter. Page list discovered via
`pouchdb.com/guides/`, scoped to hoyadb's open questions.

## Units

| Unit | Kind | Status | Output | Adopted by |
| --- | --- | --- | --- | --- |
| guides/databases | doc-page | imported | pouchdb/overview.md | — |
| guides/documents | doc-page | imported | pouchdb/architecture.md#document-model | — |
| guides/replication | doc-page | imported | pouchdb/sync.md#one-way-replication-vs-two-way-sync, pouchdb/sync.md#multi-master-architecture | — |
| guides/changes | doc-page | imported | pouchdb/sync.md#reactivity-the-changes-feed | — |
| guides/conflicts | doc-page | imported | pouchdb/sync.md#conflict-resolution-revision-trees | — |

## Notes

- Excluded on purpose (not relevant to hoyadb's open questions in this
  pass): local-documents (non-synced local-only docs — a narrower
  variant), updating-deleting and bulk-operations (CRUD API mechanics, not
  architecture), attachments (binary blob handling), mango-queries and
  queries (map/reduce) — query language syntax detail, not the
  reactivity/sync architecture this pass is after.
- All 5 units imported 2026-09-13. One inference flagged rather than
  stated as fact: whether the changes feed literally underlies live
  replication was not explicitly confirmed by either fetched page (noted
  in `pouchdb/sync.md`). Consolidated candidate behaviors written to
  `pouchdb/requirements.md`. Import complete for the scoped unit set
  above.
