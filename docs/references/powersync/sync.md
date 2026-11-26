---
source: https://docs.powersync.com/handling-writes/ (multiple pages — see per-section headers)
source_type: doc-site
fetched: 2026-09-13
---

# PowerSync — Client writes and conflict resolution (observed)

Observed, not accepted — this describes PowerSync's sync mechanism, not
hoyadb's.

## Upload queue: CRUD operation shape

Source: handling-writes/writing-client-changes (fetched 2026-09-13)

- Every local SQLite write is captured automatically into an **upload
  queue** as one of three operation types, derived directly from the SQL
  statement that caused it:
  - **PUT** (from `INSERT`) — full non-null column values for the new row.
  - **PATCH** (from `UPDATE`) — row ID plus *only the changed columns*.
  - **DELETE** (from `DELETE`) — row ID only.
  This is essentially the same create/update/delete-with-minimal-payload
  shape as WatermelonDB's `Changes` object (see
  [../watermelondb/sync.md](../watermelondb/sync.md#backend-contract-the-changes-object-and-pullpush-endpoint-rules)),
  independently arrived at.
- **No prescribed backend transport or API style**: the docs explicitly
  say the app has "full control over how to structure your backend
  application API to accept write operations" — single batch endpoint,
  per-operation-type endpoints, REST/GraphQL/gRPC, all treated as
  equally valid, reinforcing (like RxDB/WatermelonDB/Zero) that the wire
  format/contract is what's specified, not the transport.
- **Explicit synchronous-processing requirement**: the docs state
  uploads "don't place writes into something like a queue for processing
  later — process them immediately" server-side, with a stated concrete
  failure mode if violated: if the server advances its sync checkpoint
  before an async-queued write is actually applied, the client can
  observe that write *disappear* from the UI temporarily (because the
  next checkpoint doesn't yet reflect it) — a specific, named class of bug
  this synchronous requirement exists to prevent.
- **Deliberately unusual error-handling convention**: use `5xx` responses
  *only* for transient backend failures worth retrying; for validation
  or conflict errors, return a `2xx` (success) with the error detail
  carried in the response body or written to a synced "error table"
  instead — explicitly to avoid a permanently-failing write blocking the
  entire upload queue behind it. This is a different philosophy from
  WatermelonDB's push contract, which uses real error codes (e.g. forcing
  an abort-and-repull on a stale write) rather than treating
  validation/conflict failures as "successful, but flagged."

## Conflict handling: default behavior

Source: handling-writes/handling-update-conflicts (fetched 2026-09-13)

- PowerSync itself doesn't resolve conflicts — it processes the upload
  queue sequentially against whatever backend API the app wrote, so
  conflict *behavior* falls out of how that backend is implemented. In "the
  simplest backend implementation," the resulting behavior is **per-field
  last-write-wins**: "the last update (as received by the server) to each
  individual field wins" — same granularity (per-column) as WatermelonDB's
  merge, but a simpler rule (no "reapply local changes since sync" step,
  just whichever write reaches the server last).
- **Deletes always win**: a deleted row ignores any later update to it —
  the same named rule seen in Realm's (deprecated) OT rules and implied by
  WatermelonDB's delete handling.
- Operations must be **idempotent**, since the backend may receive
  duplicates (e.g. on retry) — a per-client incrementing operation/client
  ID is the suggested deduplication key.
- The docs mention **CRDTs (citing Yjs)** as an alternative for automatic
  merge in collaborative apps, as an escape hatch beyond the default LWW —
  not detailed further on this page.

## Custom conflict resolution

Source: handling-writes/custom-conflict-resolution (fetched 2026-09-13)

- The customization point is entirely on the **backend**, inside the
  app-authored `uploadData` handler that receives the upload queue — there
  is no client-side conflict-resolution hook. Each queued operation
  (`CrudEntry`) carries: operation type (`PUT`/`PATCH`/`DELETE`), row `id`,
  changed-column data (`opData`), a `clientId`, a `transactionId` grouping
  related ops, optional prior-state (`trackPrevious`), and — notably — an
  optional **`metadata` field for application-specific context**.
- **This `metadata`/`_metadata` mechanism is a direct, concrete precedent
  for hoyadb's "sync commands with context" idea**: the docs show
  `trackMetadata: true` on a table definition, then writing arbitrary JSON
  context alongside a column update (e.g.
  `UPDATE tasks SET title=?, _metadata=? WHERE id=?` with
  `{source: 'mobile_app', priority: 'high'}`), which the backend's
  `uploadData` can read to change how it handles the write — e.g. routing
  or prioritizing conflict resolution differently based on where/why a
  change happened. It's still attached to an entity-diff write (a
  PATCH), not a domain command like `add-product-to-basket`, but it's the
  clearest existing example of "carry intent/context alongside a
  synced write" found across every source in this reference set.
- Documented resolution strategies (all backend-authored, not built into
  PowerSync): timestamp comparison against server state; a sequence/version
  counter for whole-record conflict detection; per-field timestamp
  tracking (a DIY version of the default per-field LWW, made explicit and
  customizable); domain/business-rule validation (e.g. reject edits to a
  `shipped` order); and recording conflicting versions into a dedicated
  table for manual/human resolution rather than resolving automatically.

**Relevance to hoyadb**: the CRUD-with-minimal-payload wire shape is now
confirmed independently by two different production systems
(WatermelonDB, PowerSync) — good evidence it's close to a "converged on"
right answer for an entity-diff sync contract if hoyadb goes that route.
The synchronous-processing requirement and its named checkpoint-mismatch
failure mode is a concrete pitfall worth designing against explicitly in
hoyadb's own contract, regardless of sync style. The "never hard-fail the
queue, report errors out-of-band instead" convention is a real, opinionated
trade-off (robustness of the sync pipeline vs. losing the "this write was
outright rejected" signal at the transport level) worth deciding on
deliberately rather than defaulting to either extreme. Separately, the
`metadata`/`trackMetadata` mechanism is worth studying closely regardless of
which overall sync style hoyadb picks: it shows a working, minimal way to
attach hoyadb-style "context" to a write without going all the way to a
fully command-based protocol — a possible middle ground between the
pure-entity-diff sync every other source in this set uses and the
pure-command sync Zero uses.
