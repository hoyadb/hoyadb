---
source: https://vlcn.io/docs/cr-sqlite/
source_type: doc-site
fetched: 2026-09-13
updated: 2026-09-13
status: done
---

# Import: cr-sqlite

cr-sqlite (vlcn.io) is a SQLite extension adding CRDT-based multi-writer
replication directly at the SQL layer — the only CRDT-native, genuinely
relational (real SQLite tables/SQL) source in this reference set, distinct
from the document-CRDT libraries (Automerge/Yjs) and from every
entity-diff or command-based sync engine already imported. Page list
discovered via vlcn.io's docs nav, scoped to hoyadb's open questions.

## Units

| Unit | Kind | Status | Output | Adopted by |
| --- | --- | --- | --- | --- |
| cr-sqlite/crdts/table-crdts | doc-page | imported | cr-sqlite/overview.md | — |
| cr-sqlite/crdts/column-crdts | doc-page | imported | cr-sqlite/architecture.md#column-level-crdts | — |
| appendix/causal-consistency | doc-page | imported (via GitHub README fallback — see Notes) | cr-sqlite/sync.md | — |
| cr-sqlite/js/reactivity | doc-page | imported | cr-sqlite/architecture.md#reactivity-js-bindings | — |
| cr-sqlite/networking/whole-crr-sync | doc-page | imported | cr-sqlite/sync.md#whole-crr-sync-protocol | — |

## Notes

- **appendix/causal-consistency is an unpublished stub** ("To be
  published") on the official docs site as of 2026-09-13 — fell back to
  the `vlcn-io/cr-sqlite` GitHub README for real content, flagged clearly
  in `cr-sqlite/sync.md` as a secondary source.
- **Discrepancy found and left unresolved**: the docs site's column-CRDTs
  page says counter CRDTs are "not yet supported"; the GitHub README lists
  counters as supported ("sums values across replicas"). Recorded in
  `cr-sqlite/sync.md` as an open contradiction between two primary
  sources, not silently resolved either way.
- Excluded on purpose (not relevant to hoyadb's open questions in this
  pass): background, getting-started, cr-sqlite/intro (general onboarding),
  crdts/about (general CRDT primer, likely covered implicitly by the more
  specific table/column CRDT pages), crdts/sequence-crdts (ordered-list
  CRDT — a narrower case not core to hoyadb's relational-model question),
  api-methods/* (crsql_db_version, crsql_fract_as_ordered, crsql_changes —
  API reference detail, not architecture), js/networking (generic overview,
  superseded for this pass by the more specific whole-crr-sync page),
  networking/partial-crr-sync (a variant of whole-crr-sync not pursued in
  this first pass).
- All 5 units imported 2026-09-13 (1 via a fallback source due to an
  unpublished docs stub, see above). Consolidated candidate behaviors
  written to `cr-sqlite/requirements.md`. Import complete for the scoped
  unit set above.
