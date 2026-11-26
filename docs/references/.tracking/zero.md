---
source: https://zero.rocicorp.dev/docs/
source_type: doc-site
fetched: 2026-09-13
updated: 2026-09-13
status: done
---

# Import: zero

Zero (Rocicorp) is the actively-maintained successor to Replicache
(archived/sunset June 2026, per web search performed this session).
Highest-priority addition to hoyadb's reference set because it's a
**command/mutation-based** sync engine — the closest existing precedent to
NEEDS.md's preferred "sync commands, not just resulting state" approach,
unlike RxDB/WatermelonDB/Realm (all entity-diff-based). Page list
discovered via `zero.rocicorp.dev/llms.txt`, scoped to hoyadb's open
questions.

## Units

| Unit | Kind | Status | Output | Adopted by |
| --- | --- | --- | --- | --- |
| docs/sync ("What is Sync?") | doc-page | imported | zero/overview.md | — |
| docs/schema | doc-page | imported | zero/architecture.md#schema-and-relationships | — |
| docs/queries | doc-page | imported | zero/architecture.md#queries-language-sync-model-and-reactivity | — |
| docs/mutators | doc-page | imported | zero/sync.md#mutators-definition-and-dual-execution | — |
| docs/server-zql | doc-page | imported | zero/sync.md#server-side-execution-details | — |

## Notes

- Excluded on purpose (not relevant to hoyadb's open questions in this
  pass): introduction, when-to-use, install, tutorial, quickstart, samples,
  react/solidjs/react-native framework-integration pages, zql (full query
  language reference — syntax detail, not architecture), debug/* (slow
  queries, analyze-query-cli, replication debugging), connection, auth.
- All 5 units imported 2026-09-13. Consolidated candidate behaviors written
  to `zero/requirements.md`. The "sync-page" claim of server-reconciliation
  handling conflicts during mutator sync was not fully spelled out in the
  docs found — noted as an unverified gap in `zero/sync.md` rather than
  guessed at. Import complete for the scoped unit set above.
