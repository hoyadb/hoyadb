---
source: https://github.com/Nozbe/WatermelonDB (docs-website/docs/docs, master branch)
source_type: source
fetched: 2026-09-13
updated: 2026-09-13
status: done
---

# Import: watermelondb

Docs are Markdown files inside the WatermelonDB repo itself
(`docs-website/docs/docs/`), not a separately hosted doc-site with its own
crawlable nav — confirmed by checking the repo tree via the GitHub API
after `watermelondb.dev/docs/` 404'd. Fetched as raw source files (`source`
type), scoped to the pages relevant to hoyadb's open questions: overview,
schema/relations (relational-ish model), reactive observation, and sync
(both the app-facing API and what a backend must implement).

## Units

| Unit | Kind | Status | Output | Adopted by |
| --- | --- | --- | --- | --- |
| docs/README.md | doc-page | imported | watermelondb/overview.md | — |
| Implementation/Architecture.md | doc-page | imported | watermelondb/architecture.md#base-objects | — |
| Schema.md | doc-page | imported | watermelondb/architecture.md#schema--data-model | — |
| Relation.md | doc-page | imported | watermelondb/architecture.md#relations | — |
| Advanced/Flow.md | doc-page | ignored | — | — |
| Query.md | doc-page | imported | watermelondb/architecture.md#query-execution-and-observing | — |
| Sync/Frontend.md | doc-page | imported | watermelondb/sync.md#frontend-synchronize-and-the-pullpush-contract | — |
| Sync/Backend.md | doc-page | imported | watermelondb/sync.md#backend-contract-the-changes-object-and-pullpush-endpoint-rules | — |
| Implementation/SyncImpl.md | doc-page | imported | watermelondb/sync.md#sync-algorithm-internals | — |

## Notes

- **Correction (2026-09-13)**: `Advanced/Flow.md` was fetched under the
  assumption it covered reactive dataflow/observing. It's actually about
  using Facebook's Flow type-checker with WatermelonDB — unrelated to
  reactivity. Marked `ignored` (not `failed` — it fetched fine, it just
  wasn't the right unit). Replaced with `Query.md`, which is where
  `observe()`/`observeCount()` are actually documented (confirmed via
  Implementation/Architecture.md referencing `query.observe()`).
- Excluded on purpose (not relevant to hoyadb's open questions in this
  pass): CRUD.md, Components.md, Installation.mdx, Setup.md,
  Writers.md, CHANGELOG.md, CONTRIBUTING.md, Roadmap.md, all of
  Advanced/ except Flow.md (AdvancedFields, CreateUpdateTracking,
  LocalStorage, Logging, Migrations, Performance, ProTips,
  SharingDatabaseAcrossTargets), Sync/Contribute.md, Sync/FAQ.md,
  Sync/Troubleshoot.md, Implementation/DatabaseAdapters.md,
  Implementation/Publishing.md.
- Sync/Limitations.md and Sync/Intro.md considered but folded into the
  Frontend.md/Backend.md/SyncImpl.md units rather than tracked separately —
  will pull from them inline if Frontend/Backend/SyncImpl cross-reference
  them.
- All 8 units (7 planned + 1 substitution) resolved 2026-09-13: 7 imported,
  1 ignored (Advanced/Flow.md, see correction note above). Consolidated
  candidate behaviors written to `watermelondb/requirements.md`. Import
  complete for the scoped unit set above.
