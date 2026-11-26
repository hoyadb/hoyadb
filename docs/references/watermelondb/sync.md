---
source: https://github.com/Nozbe/WatermelonDB (docs-website/docs/docs/Sync/, docs/docs/Implementation/SyncImpl.md — see per-section headers)
source_type: source
fetched: 2026-09-13
---

# WatermelonDB — Sync (observed)

Observed, not accepted — this describes WatermelonDB's sync mechanism, not
hoyadb's.

## Frontend: `synchronize()` and the pull/push contract

Source: Sync/Frontend.md (fetched 2026-09-13)

- App code calls `synchronize({ database, pullChanges, pushChanges, ... })`
  whenever/however often it decides is appropriate — WatermelonDB does not
  schedule sync itself. A helper `hasUnsyncedChanges({ database })` lets the
  app check for pending local changes (e.g. to decide whether to sync).
- **`pullChanges({ lastPulledAt, schemaVersion, migration })`**: app-supplied
  function that must call the real backend and return
  `{ changes, timestamp }` — `changes` being "all changes in all collections"
  since `lastPulledAt` (or everything, on first sync when `lastPulledAt` is
  `null`), `timestamp` being the server's current time (becomes the next
  `lastPulledAt`). Documented as a strict contract with "MUST" requirements:
  must be async/Promise-returning, must forward `lastPulledAt`/
  `schemaVersion`/`migration` to a backend that implements "Watermelon Sync
  Protocol," must not retain/mutate the returned object afterward (treated
  as consumable, mutated internally for performance).
- **`pushChanges({ changes, lastPulledAt })`**: app-supplied function that
  sends local changes to the backend. Must resolve **only after** the
  backend confirms it applied them, must reject on backend failure, must
  not resolve early or on failure, must not mutate the input.
- **Security boundary named explicitly**: "You MUST NOT connect to backend
  endpoints you don't control" — WatermelonDB assumes pull/push handlers are
  trustworthy and does not validate/sanitize what they return.
- **Concurrency guards**: calling `synchronize()` while one is already
  running safely aborts the new call; resetting the local DB mid-sync can
  leave it inconsistent (documented as unsafe, not prevented).
- **Migration Sync**: schema migrations need `migrationsEnabledAtVersion`
  set so the backend can be told (via the `migration` argument to
  `pullChanges`) which tables/columns a client needs backfilled after
  updating — otherwise a client that migrated forward can silently miss
  data added to new tables/columns before it updated. Documented as an
  easy-to-get-wrong area (several numbered caveats about apps that shipped
  before this existed).
- **Turbo Login** (v0.23+): initial-sync-only optimization that skips
  JSON-parsing the pulled payload in JS entirely (raw JSON text passed
  straight to native code) for up to ~5x faster first sync; explicitly
  incompatible with incremental syncs or non-empty local databases, and
  marked "unsafe" (API not yet stable).
- **Replacement Sync** (v0.25+, opt-in, `experimentalStrategy: 'replacement'`
  on the `pullChanges` response): backend sends the *entire* accessible
  dataset instead of a delta; the client reconciles by replacing its DB with
  that dataset while preserving any locally unpushed
  created/updated/deleted records. Documented as a last-resort escape hatch
  for cases where the backend can't compute an incremental diff (e.g.
  complex permission changes) — explicitly much slower than normal sync,
  offered as an alternative to "wipe local DB and do a fresh login sync"
  because it avoids having to tear down all UI observation.
- Other configuration knobs: a `conflictResolver(table, local, remote,
  resolved)` callback to customize per-record merge behavior;
  `onWillApplyRemoteChanges`/`onDidPullChanges` hooks around applying a
  pulled batch; built-in `SyncLogger` for diagnostics (explicitly warned to
  handle carefully since logs can contain user data).

**Relevance to hoyadb**: confirms — like RxDB — that WatermelonDB's sync
unit is **changed entity/record state**, not a command/intent; "changes"
here means create/update/delete records, referenced generically as "a valid
changes object" (exact shape covered in the Backend unit below). This is
the second of two strong existing precedents for the entity/diff-based
*fallback* option named in NEEDS.md, and further evidence that a
command-based-with-context sync isn't how either major reference project
approaches it — worth treating as a real design risk for hoyadb's preferred
approach, to be surfaced explicitly in `legacy/references/comparison.md`. Separately,
the strict MUST/MUST-NOT contract style (resolve only on confirmed
persistence, reject on failure, no early resolution) and the named
concurrency/security guardrails are good raw material for hoyadb's own sync
contract regardless of state-based vs. command-based.

## Backend contract: the `changes` object and pull/push endpoint rules

Source: Sync/Backend.md (fetched 2026-09-13)

**Data shape** — the entire wire format is one `Changes` object, grouped by
table, each table split into `created`/`updated` (raw records, using schema
column names like `is_favorite` not `isFavorite`) and `deleted` (an array of
bare IDs, never full records):

```js
Changes = {
  [table_name]: { created: RawRecord[], updated: RawRecord[], deleted: string[] }
}
```

**Pull endpoint rules** (selected, the doc has ~13 numbered MUST/SHOULD
rules): return every create/update/delete across all tables since
`lastPulledAt` (or everything, if `null`/first sync); the returned
`timestamp` must be usable as the next `lastPulledAt` with no gaps or
double-counted changes; the read must be internally consistent (all queried
under one lock/transaction, or timestamped defensively if not, so a write
mid-pull can't be silently dropped); a `migration` argument tells the
backend which tables/columns are new to this client since its last sync so
it can backfill them; server responses must never leak the internal
`_status`/`_changed` bookkeeping fields, must whitelist table/column names
(explicitly to prevent e.g. `__proto__`/`constructor` injection), and must
restrict record IDs to a safe character set.

**Push endpoint rules** (selected): apply creates/updates/deletes
idempotently — re-pushing an already-applied create must update rather than
error, deleting an already-deleted record must silently no-op, updating a
record the server no longer has must either recreate it or (if it's known
deleted) force a conflict by erroring; **a record modified server-side after
the client's `lastPulledAt` MUST abort the whole push with an error**,
forcing the client to pull-then-retry rather than silently overwriting a
newer server value — this is the actual conflict-detection mechanism,
implemented entirely as a timestamp comparison, not a diff/merge; the push
must be fully transactional (all-or-nothing); the server must ignore/never
trust `_status`/`_changed` from the client; invalid-but-not-malformed data
should be sanitized/corrected rather than rejected, with an explicit
rationale that hard-failing sync on a stale app-version bug can leave users
permanently unable to sync.

**Recommended server-side implementation** (not part of the protocol, but
telling as a worked example): a monotonic `last_modified` timestamp column
on every table (with a DB-level uniqueness/monotonicity guarantee, since
naive wall-clock timestamps can collide or go backward), soft-deletes (a
`deleted` flag or a separate tombstone table) since the protocol needs to
report deletions as bare IDs, and — notably — **the same tracking mechanism
extends to permission changes**: granting access to a previously-invisible
record is reported as a `created`, revoking access is reported as a
`deleted`, treating access-control changes as just another kind of "record
changed" event.

**Relevance to hoyadb**: this is the single most concrete, load-bearing
artifact for hoyadb's "define the data contract, leave transport open" goal
— an entity-diff wire format precise enough that ~13 independent backend
implementations exist for different languages/frameworks (see "Existing
Backend Implementations" in the source) without WatermelonDB needing to
prescribe HTTP/REST/GraphQL/anything. Note: the *push-abort-on-stale-write*
rule described here is only the trigger for a conflict, not the resolution —
see the per-column client-wins merge in the "Sync algorithm internals"
section below for how a conflict actually gets resolved. Treating
permission/visibility changes as synthetic created/deleted events (per
"Tips on implementing server-side changes tracking" above) is a pattern
hoyadb should consider for its own model regardless of sync style, since
"record newly visible to me" / "no longer visible to me" are real events any
sync design has to handle somehow.

## Sync algorithm internals

Source: Implementation/SyncImpl.md (fetched 2026-09-13)

This is WatermelonDB's own reference implementation of the protocol above —
useful as a fully worked algorithm, not just a wire contract.

**Design choices stated up front:**
- **Master/replica, not peer-to-peer**: server is the single source of
  truth; clients each hold a full local replica and sync only client↔server.
- **Two-phase**: pull (apply remote → local) fully completes before push
  (send local → remote) begins, every sync round.
- **Conflict resolution happens on the client**, not the server, and is
  **content-based (per-column), not time-based**: on conflict, the
  algorithm takes the *entire remote record* and then re-applies only the
  *specific local columns* that changed since the last sync — i.e.
  "server wins" as a starting point, "client wins" per individual changed
  field on top of that. This is a real third option distinct from both
  RxDB's whole-document conflict handler (see
  [../rxdb/architecture.md](../rxdb/architecture.md)) and a naive
  last-write-wins.
- **Change tracking is local-only**: the client stores `_status`
  (synced/created/updated/deleted) and `_changed` (which columns changed
  since last sync) per record; the **server only needs a timestamp/version
  per record**, no per-column change tracking server-side.
- Sync covers the whole local database per round, not per-collection.
- **Non-blocking**: local writes are only locked for the brief "apply
  changes" step of each phase; reads are never blocked, and the user can
  keep writing throughout a sync.

**Pull-phase algorithm** (server response applied locally): for each
incoming record — insert if new (recovering gracefully if it turns out to
already exist or was locally soft-deleted); for updates, if the local copy
is already fully synced just overwrite it, but if the local copy has
unsynced local edits, run the per-column conflict merge described above and
keep it marked `updated` (still needs pushing); a locally-deleted record
receiving a remote update is simply ignored (the deletion will be pushed
next); destroys apply immediately regardless of local edit state, with
children left for the server to schedule.

**Push-phase algorithm**: gather every locally-dirty record, strip the
internal `_status`/`_changed` fields before sending, push; on success, mark
those exact records synced — **except** any record the user edited again
*during* the network round-trip, which stays dirty and gets pushed again
next round rather than being incorrectly marked synced.

**Crash/interruption safety** is a named, deliberate property: the whole
procedure is idempotent enough that a sync interrupted at any point still
converges to a consistent state on the *next* sync attempt — re-pulling the
same range is safe, re-pushing not-yet-acknowledged local changes is safe,
and a mid-flight remote change either gets picked up next pull or triggers
the push-abort conflict path, never silently lost.

**Migration syncs**: because "which schema version did this client last
sync at" wasn't tracked from WatermelonDB's first version, the doc includes
a full decision table reconciling `lastPulledAt`/`migrationsEnabledAtVersion`
/`lastSyncedSchemaVersion`/`currentSchemaVersion` to decide whether it's
safe to mark a client as caught-up-through-the-latest-migration — flagged
explicitly as backwards-compatibility complexity that a fresh design
wouldn't need if schema-version-at-last-sync were tracked from day one.

**Prior art cited by WatermelonDB itself**: Kinto/Kinto.js and a cited
academic reference ("Histo" / a sync thesis) — signals this is a
well-trodden problem space with existing literature worth a further look if
hoyadb's own protocol design stalls.

**Relevance to hoyadb**: the per-column client-wins-on-top-of-server-state
merge is a genuinely different, more nuanced conflict strategy than either
RxDB's document-level handler or a naive LWW, and is probably the single
most reusable concrete algorithm from either reference project for hoyadb's
own conflict resolution — worth deep-diving before designing hoyadb's
approach, independent of the state-vs-command sync question. The explicit
idempotency/interruption-safety property is a good model for how hoyadb
should specify its own sync procedure: not just "what data moves" but "what
happens if this step fails halfway."
