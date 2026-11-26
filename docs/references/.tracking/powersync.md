---
source: https://docs.powersync.com/
source_type: doc-site
fetched: 2026-09-13
updated: 2026-09-13
status: done
---

# Import: powersync

Cited in web research this session as the most production-mature/
battle-tested local-first sync engine, especially for mobile, with full
bidirectional Postgres/MySQL/MongoDB/SQL-Server↔SQLite sync via "buckets."
Page list discovered via `docs.powersync.com/llms.txt`, scoped to hoyadb's
open questions.

## Units

| Unit | Kind | Status | Output | Adopted by |
| --- | --- | --- | --- | --- |
| architecture/architecture-overview | doc-page | imported | powersync/overview.md | — |
| sync/rules/organize-data-into-buckets | doc-page | imported | powersync/architecture.md#sync-scoping-sync-rules-and-buckets | — |
| handling-writes/writing-client-changes | doc-page | imported | powersync/sync.md#upload-queue-crud-operation-shape | — |
| handling-writes/handling-update-conflicts | doc-page | imported | powersync/sync.md#conflict-handling-default-behavior | — |
| handling-writes/custom-conflict-resolution | doc-page | imported | powersync/sync.md#custom-conflict-resolution | — |
| client-sdks/watch-queries | doc-page | imported | powersync/architecture.md#reactivity-three-tiers-of-watch-query | — |

## Notes

- Excluded on purpose (not relevant to hoyadb's open questions in this
  pass): intro/setup-guide, examples, powersync-philosophy, ai-tools, all
  per-language client SDK install pages, per-backend-database
  (Postgres/MongoDB/MySQL/SQL-Server/Convex) connection setup pages,
  sync/types.md and sync/supported-sql.md (type-mapping/SQL-syntax
  reference detail, not architecture), sync/rules/global-buckets.md
  (a variant of the bucket concept already covered by
  organize-data-into-buckets), handling-writes/handling-write-validation-errors
  (operational error-handling detail).
- architecture-overview.md was a thin source (mostly a diagram, little
  prose) — recorded honestly with only what text was actually present,
  not reconstructed from the diagram.
- All 6 units imported 2026-09-13. Consolidated candidate behaviors written
  to `powersync/requirements.md`. Notable finding: the `trackMetadata`/
  `_metadata` mechanism is the clearest working precedent found anywhere in
  this reference set for attaching context to a synced write short of a
  fully command-based protocol. Import complete for the scoped unit set
  above.
