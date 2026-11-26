---
source: https://electric.ax/docs/sync (note: electric-sql.com permanently redirects here as of 2026-09-13)
source_type: doc-site
fetched: 2026-09-13
updated: 2026-09-13
status: done
---

# Import: electricsql

Cited in web research this session as taking a distinctly different
architectural split from every other source in this reference set: a
**read-path-only** sync engine — it syncs Postgres data out to clients, but
writes go back through the app's own API, not through Electric itself.
Page list discovered via electric.ax's docs nav (after a domain redirect
from electric-sql.com), scoped to hoyadb's open questions.

## Units

| Unit | Kind | Status | Output | Adopted by |
| --- | --- | --- | --- | --- |
| docs/sync (overview) | doc-page | imported | electricsql/overview.md | — |
| docs/sync/guides/shapes | doc-page | imported | electricsql/architecture.md | — |
| docs/sync/guides/writes | doc-page | imported | electricsql/sync.md | — |
| docs/sync/api/http | doc-page | imported | electricsql/architecture.md#http-protocol-mechanics | — |

## Notes

- Domain migrated: electric-sql.com now 301-redirects to electric.ax.
  Using electric.ax throughout.
- Excluded on purpose (not relevant to hoyadb's open questions in this
  pass): quickstart, stacks, sync/postgres-sync and sync/pglite (backend/
  local-storage setup detail), sync/tanstack-db (a separate project's
  integration, not Electric's own architecture), auth/security/
  postgres-permissions guides (access-control detail, not sync mechanics),
  TypeScript client API reference, framework/platform integration pages.
- No dedicated reactivity/live-query concept distinct from Shape
  subscription itself was found in this pass — a Shape subscription *is*
  Electric's reactivity primitive (initial snapshot + live stream), there
  isn't a separate observe/watch API layered on top the way RxDB/
  WatermelonDB/Realm/Zero/PowerSync each have. Not pursued further since
  this appears to be a real architectural fact, not a gap in the search.
- All 4 units imported 2026-09-13. Consolidated candidate behaviors written
  to `electricsql/requirements.md`. Import complete for the scoped unit set
  above.
