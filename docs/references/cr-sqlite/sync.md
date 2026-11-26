---
source: https://vlcn.io/docs/appendix/causal-consistency (page stub, "To be published" as of fetch) + https://github.com/vlcn-io/cr-sqlite README (fetched as fallback)
source_type: doc-site
fetched: 2026-09-13
---

# cr-sqlite — Causal consistency and whole-database merge (observed)

Observed, not accepted — this describes cr-sqlite's sync/merge mechanism,
not hoyadb's.

**Source note**: the official docs page for this topic
(`/docs/appendix/causal-consistency`) is a stub reading "To be published"
as of this fetch (2026-09-13) — it points readers to the GitHub README and
external blog posts instead. The content below comes from the
`vlcn-io/cr-sqlite` GitHub README as a fallback source, per this session's
"extract minimal real detail, don't invent" instruction — flagged as a
secondary source, not the project's own primary docs.

## Version tracking

The `crsql_changes` virtual table exposes, per changed row: `col_version`
(per-column version), `db_version` (database-wide change sequence number),
and `site_id` (identifies which replica/writer made the change) — the
mechanism used to determine "what changed since I last synced with this
peer," analogous in role to WatermelonDB's `_status`/`_changed` columns or
PowerSync's upload queue, but exposed as a queryable SQL view rather than
hidden bookkeeping columns.

## Merge process

Per the README: "Rows are identified by primary key," and "Tables are
unioned (and a delete log is consulted) such that both tables will have the
same rows" — i.e. merging two databases is a row-level union keyed by
primary key, with deletions tracked in a separate log (consistent with
[overview.md](overview.md)'s note that a table's chosen CRDT type governs
which rows survive a merge). Conflicts are then resolved **per column**,
using whatever CRDT type that column was assigned (see
[architecture.md](architecture.md#column-level-crdts)) — there is
explicitly "no options for manual merging": the README frames this as
fully automatic, with no app-level conflict-resolution hook at all. This is
a real, opinionated contrast to every other source in this reference set
(RxDB, WatermelonDB, PowerSync all expose a pluggable/custom
conflict-resolution hook; Zero sidesteps merging via mutator re-execution).

## Discrepancy found — flagged, not resolved

The **docs site's column-CRDTs page** (fetched same session, see
[architecture.md](architecture.md#column-level-crdts)) states: "Counter
CRDTs are currently not yet supported." The **GitHub README**, fetched as
a fallback for this unit, lists "Counters: sums values across replicas" as
a supported per-column CRDT type. These two primary sources disagree as of
2026-09-13. Possible explanations (none confirmed): the README describes a
newer/different version than the published docs site, or one of the two
sources is stale. **Not resolved in this pass** — recorded as an open
discrepancy rather than silently picking one claim as fact. If hoyadb's own
design work later depends on whether cr-sqlite actually supports counter
CRDTs, this should be re-verified against the current source code directly
rather than either doc source.

## Whole-CRR sync protocol

Source: cr-sqlite/networking/whole-crr-sync (fetched 2026-09-13)

- "Whole CRR sync" is defined as the goal that "all of the CRRs on
  different databases [are] identical after they merge" — a full,
  symmetric convergence goal (not a client-pulls-a-subset model like
  Shapes/buckets/active-queries in the other sources studied).
- **The wire format is literally the changeset relation itself**: "the
  output of a select from `crsql_changes` can be directly fed back into an
  insert into `crsql_changes` on another database" — i.e. no separate
  serialization step; the same virtual table is both the local change log
  and the sync payload shape. Changesets are noted to "compress well"
  (repeated structure across rows).
- **Four core primitives** make up the whole protocol: `crsql_as_crr`
  (mark a table as a CRDT/CRR), `crsql_changes` (the changeset
  virtual-table, used for both reading and applying changes),
  `crsql_site_id` (a stable unique id per database/replica), and
  `crsql_tracked_peers` (a table storing, per known peer, the last
  `db_version` cursor synced from them).
- **Incremental sync is a version-cursor pull**: to catch up with peer
  `db1`, `db2` asks for all `db1` changes with `db_version` greater than
  whatever cursor `db2` has stored for `db1` in `crsql_tracked_peers` —
  structurally the same "monotonic version/timestamp cursor" idea as
  WatermelonDB's `lastPulledAt` or PowerSync's checkpoints, but here it's
  a native, queryable SQLite table rather than app-managed state.
- **No prescribed transport**: explicitly left to the app (REST, WebSocket,
  WebRTC all mentioned as viable, none mandated) — the same
  "specify the data contract, not the transport" posture as every other
  source in this reference set except PowerSync (dedicated service).

## Relevance to hoyadb

The counter-CRDT discrepancy is itself a useful data point regardless of
which source is right: it shows counters are enough of an edge case in
value-based CRDT systems that even this project's own documentation is
inconsistent about their status — a third piece of evidence (after Realm's
historical OT special-case and this project's explicit non-support in the
docs site) that counters are a recurring, non-trivial problem for
pure-state-based conflict resolution, strengthening the case for
command-based sync being able to handle a real gap other approaches
struggle with. The "no manual merge hook, fully automatic" design is also
a useful opposite pole from PowerSync/RxDB's fully-custom-backend-hook
approach — worth deciding where on that spectrum hoyadb wants to sit.
Exposing sync bookkeeping (`crsql_changes`, `crsql_tracked_peers`,
`crsql_site_id`) as **queryable SQL views/tables rather than hidden
internal state** is a distinctive implementation choice — makes the sync
log inspectable/debuggable with plain SQL, worth considering for hoyadb's
own implementation regardless of which sync architecture is chosen.
