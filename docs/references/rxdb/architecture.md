---
source: https://rxdb.info/ (multiple pages, see per-section headers)
source_type: doc-site
fetched: 2026-09-13
---

# RxDB — Architecture (observed)

Observed, not accepted — this describes RxDB's structure, not hoyadb's.

## Storage: the `RxStorage` interface

Source: https://rxdb.info/rx-storage.html (fetched 2026-09-13)

RxDB explicitly separates database logic from persistence: "RxDB is not a
self-contained database. Instead the data is stored in an implementation of
the RxStorage interface," letting the underlying engine be swapped "depending
on the JavaScript environment and performance requirements."

- The interface is defined once (`src/types/rx-storage.interface.d.ts` in
  RxDB's source — not fetched as part of this doc-site pass); any conforming
  implementation can be plugged into `createRxDatabase()` via a `storage`
  parameter.
- Implementations observed: Memory, LocalStorage, IndexedDB, OPFS (browser);
  Filesystem Node (Node/Electron); SQLite (Electron/React
  Native/Capacitor); Expo Filesystem; Dexie.js (IndexedDB wrapper);
  MongoDB, Deno KV, FoundationDB (server-side).
- "Storage wrappers" can wrap another `RxStorage` to add cross-cutting
  concerns (encryption, compression, validation, worker-thread offloading,
  sharding) without changing the underlying engine or the app-facing API.
- Net effect: one core API, storage engine chosen per platform, wrappers
  composed independently of which engine is chosen.

**Relevance to hoyadb**: this is close to precisely the "storage engine may
change across environment" need from [NEEDS.md](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/NEEDS.md) — a single
narrow storage contract, engine chosen per platform (their SQLite-on-mobile /
IndexedDB-on-browser split mirrors hoyadb's own "probably SQLite or
equivalent" expectation), with optional wrapper composition for concerns
like encryption bolted on separately from the engine choice.

## Query layer: `RxQuery`

Source: https://rxdb.info/rx-query.html (fetched 2026-09-13)

- Query construction uses a MongoDB-like "Mango Query" JSON selector syntax
  (e.g. `myCollection.find({ selector: { age: { $gt: 18 } } })`), chosen
  because it's composable/transformable as data and TypeScript-friendly; a
  separate query-builder plugin adds a chained `.where('age').gt(18)` style
  on top of the same underlying query object.
- Three operation shapes: `find()` (array of documents), `findOne()` (single
  document or null), `count()`.
- **Two ways to consume a query's result**, both against the *same* RxQuery
  object:
  - `.exec()` — one-shot, returns a Promise.
  - `.$` — an Observable (BehaviorSubject) that emits the current
    result-set immediately and **re-emits whenever the underlying data
    changes**, explicitly framed as enabling "UIs that should always show
    the same state as what is written in the database."
- Performance mechanisms named (not deeply detailed on this page): result
  caching/de-duplication, and an "EventReduce" algorithm that reportedly
  updates an existing observed result set incrementally rather than
  re-running the full query on every underlying write.
- RxQuery objects are immutable — `.sort()`/etc. return a new RxQuery rather
  than mutating in place.

**Relevance to hoyadb**: this is the clearest existing precedent for the
"subscription/observer on an entity or query" requirement in
[NEEDS.md](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/NEEDS.md) — same query object, promise for a one-shot read
vs. observable for a live subscription, with an explicit incremental-update
optimization (EventReduce) called out as the thing that makes observing
queries *not* slow. Worth studying EventReduce specifically before designing
hoyadb's own subscription mechanism, since "don't recompute the whole query
on every write" is directly hoyadb's performance concern too.

## Reactivity model (general)

Source: https://rxdb.info/reactivity.html (fetched 2026-09-13)

- Foundation is RxJS Observables, but the observable type is pluggable: a
  `reactivity` factory passed to `createRxDatabase()` can swap in Angular
  Signals, React Signals, Vue refs, MobX/Redux state, etc. — apps consume
  whichever reactive primitive fits their UI framework, via a `$$` ("double
  dollar") convention distinct from the always-available RxJS `$`.
- Granularity of what can be observed: a single document field
  (`RxDocument.fieldName$`), a whole document (`RxDocument.$`), a query's
  result set (`RxQuery.$`), a local (non-synced) document, and
  collection-wide change events.

**Relevance to hoyadb**: confirms observation should exist at more than one
granularity (field / document / query), not just "subscribe to a query" —
worth deciding explicitly whether hoyadb needs single-field-level
observation or document+query is enough. The pluggable-observable-type
approach (RxJS by default, swappable per UI framework) is a pattern worth
considering for hoyadb's own wrappers layer rather than forcing every
consumer onto one reactive library.

## Replication protocol

Source: https://rxdb.info/replication.html (fetched 2026-09-13)

RxDB frames this as *the* protocol — transport-specific plugins (HTTP,
WebSocket, GraphQL, ...) are thin bindings on top of it, which is why this
project's import intentionally skipped those transport-specific pages (see
tracker Notes).

- **Pull handler** (client → asks server): given the last checkpoint (or
  `null`), returns all documents written after it, plus the new checkpoint.
  A **pull stream** (observable) variant pushes new master writes and
  checkpoints in real time instead of polling.
- **Push handler** (client → sends to server): sends each changed document
  as a pair — `assumedMasterState` (what the client believed the server had
  last) and `newForkState` (the client's new local value) — and gets back
  only the subset that are in conflict.
- **Checkpoint**: a small subset of fields from the last-pulled document
  (typically `updatedAt` + primary key) sufficient to resume pulling
  deterministically; requires documents to be deterministically sortable by
  last-write-time. Once a pull returns fewer documents than the batch size,
  RxDB treats history as caught up and switches to live event observation.
- **Transport-agnostic by design**: stated rationale is that all the hard
  parts (conflict handling, offline/online switching) live inside RxDB
  itself, so *any* transport that can move JSON (HTTP, WebSocket, long
  polling, SSE) can carry the protocol — the backend just has to implement
  pull/push, not RxDB's internal logic.
- **Data shape exchanged**: JSON documents — id/content fields, an
  `updatedAt` timestamp, a `_deleted` boolean (soft delete, not physical
  deletion), optional `_attachments` metadata. Batched for transfer/
  compression efficiency.
- **Conflict detection**: purely state-based — a push is in conflict when
  the server's actual current state for a document differs from the
  `assumedMasterState` the client thought it was based on; the conflict
  handler then resolves client-side using both states.
- **Commands/intent**: nothing on this page describes carrying anything
  other than resulting document state — no operation log, command, or
  intent concept in the protocol as documented. State-based reconciliation
  (before/assumed-state vs. new-state, resolved via a conflict handler) is
  the whole model; *why* a document changed is not part of what's
  transmitted, only *what* it changed to.

**Relevance to hoyadb**: this is the key reference point for the
command-based-sync-with-context question in [NEEDS.md](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/NEEDS.md).
RxDB's protocol is deliberately **entity-state-based**, not command-based —
it syncs "what changed," with conflict resolution keyed on
before/after-state comparison, and explicitly keeps all sync complexity
client-side so any dumb transport/backend works. This is direct evidence for
the "fallback" option named in NEEDS.md (entity/diff-based sync) but *not*
precedent for the preferred command-with-context approach — that gap is
worth carrying into `legacy/references/comparison.md` explicitly, since none of the
three sources may turn out to support it natively.

## Revisions, conflict handlers, and transactions

Source: https://rxdb.info/transactions-conflicts-revisions.html
(fetched 2026-09-13)

- Every document carries a revision string (e.g. `1-9dcca3b8e1a`): a height
  counter plus a token, incremented on each write. A write must include the
  previous revision it was based on; if that no longer matches the
  currently stored revision, the write is rejected as a conflict
  (`409 CONFLICT`) rather than silently applied — this is the mechanism
  underlying the replication protocol's `assumedMasterState` check above.
- A **conflict handler** is one object with two responsibilities: detect
  (compare two document states for equality) and resolve (merge them into
  one). The default handler does last-write-loses-to-master: on conflict it
  drops the local ("fork") state and keeps the master's. A custom handler
  can instead merge fields from both states.
- **No multi-document transactions**: RxDB is explicit that "a single write
  operation to a document is the only atomic thing you can do" — no
  cross-document ACID transactions. This is presented as a deliberate
  trade-off for offline-first operation (clients keep working/writing
  locally without coordinating locks across documents), not an oversight.

**Relevance to hoyadb**: the revision-height-plus-token scheme is a concrete,
minimal mechanism worth considering for hoyadb's own conflict detection
regardless of whether sync ends up command- or state-based. The "no
multi-document transaction" trade-off is a direct, explicit precedent to
weigh against hoyadb's relational-ish data model ambition — relations often
imply multi-record invariants, so this is a real tension to resolve in
hoyadb's own ADR, not something to inherit uncritically.
