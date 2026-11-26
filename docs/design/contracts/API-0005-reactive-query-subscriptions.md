---
id: API-0005
title: Projected storage and owner-driven reactive query delivery
type: api
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-31
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0007, REQ-0008, REQ-0009, REQ-0042]
depends_on: [SPEC-0027, API-0003, API-0004]
tags: [reactivity, phase-three]
---

# API-0005: Reactive query contract

Public types live in hoyadb_core::model; projected storage types live in
hoyadb_core::storage. Data is local application data under DM-0001 classification
and retention; ephemeral caches/events last until drain/cancel/drop. No network,
telemetry, runtime dependency or persistent subscription state.

```rust
pub struct ProjectedRow {
    pub id: String,
    pub values: BTreeMap<String, Value>,
    pub order_key: Option<Value>,
}
pub trait ObservationStorage: SchemaStorage {
    fn project(&self, schema: &Schema, query: &Query, fields: &[String])
        -> impl Future<Output=Result<Vec<ProjectedRow>,StorageError>> + Send;
}
pub struct Subscription(pub u64);
pub enum ObservationValue { Rows(Vec<Row>), Count(usize) }
pub struct Notification {
    pub subscription: Subscription,
    pub value: Result<ObservationValue, StorageError>,
}
pub struct ReactiveDatabase<S> { /* exclusive Database owner and shared caches */ }
```

project validates query and requested own/direct-to-one paths and physical tables
even at limit zero. Predicate/order wrong-kind cells retain API-0003 semantics
(nonmatch / Null ordering). Requested cells on returned keys validate type and
required presence; malformed consulted physical payload is BackendFailure.
order_key is None without ordering; otherwise it is the API-0003 normalized
ordering cell, including Some(Null) for missing/dangling/wrong-kind cells. This
selection role is separate from strict explicitly requested values.
Limit zero reads no values. It follows API-0003 filtering/order/
limit and returns only id plus requested values (missing/dangling optional
cells normalize to Null). Count requests only IDs plus any required ordering
key, never a full Row or unrelated field. Ordinary full-row query validation
remains unchanged; projection validates its consulted cells, not unchecked
unselected payload. All SQL names/values are bound data.

Database::into_reactive(self) creates a ReactiveDatabase where S implements
ObservationStorage. It exposes immutable dereference to Database for query,
related, schema and fetch, plus insert/replace/remove/apply_batch/link/unlink
with API-0004 arguments/results. It does not expose mutable/raw ownership until
into_storage(self), which closes all subscriptions. An internal optional
committed-change journal carries successful public mutations to this owner;
nonreactive Database does not accumulate a journal.

```rust
impl QueryBuilder {
    pub async fn observe<S: ObservationStorage>(&self, db: &mut ReactiveDatabase<S>)
        -> Result<Subscription,StorageError>;
    pub async fn observe_count<S: ObservationStorage>(&self, db: &mut ReactiveDatabase<S>)
        -> Result<Subscription,StorageError>;
    pub async fn observe_with_columns<S: ObservationStorage>(
        &self, db: &mut ReactiveDatabase<S>, columns: Vec<String>)
        -> Result<Subscription,StorageError>;
}
impl<S: ObservationStorage> ReactiveDatabase<S> {
    pub fn unsubscribe(&mut self, subscription: Subscription) -> bool;
    pub fn drain_notifications(&mut self, now_ms: u64) -> Vec<Notification>;
    pub async fn refresh_notifications(&mut self);
    pub fn observation_stats(&self) -> ObservationStats;
    pub fn into_storage(self) -> S;
}
```

Named columns are own declared application columns or id, never managed names,
relation paths or unknown names; invalid registration returns InvalidQuery
before cache mutation. Registration errors retain the database. Membership
compares visible ID sets, never only counts; per-field compares actual stored
optional values; Count suppresses equal counts even if membership changed.
Initial events queue immediately. Default/finer rows are complete owned current
commit snapshots with managed fields stripped. Updates to cached ordering may
change the next emitted ordering without emitting for reorder alone.

ObservationStats exposes groups, observers, projected_queries,
candidate_updates and materialized_rows as diagnostic work counters. It does
not promise constant-time writes or hide initial key loading. Normal direct
writes are restricted to changed IDs; reverse dependencies use indexed FK
queries; navigation structural changes may rebuild only that related key set.
Zero observers means zero maintenance work. Subscription IDs are globally
nonreused within the running core instance across database owners and reopen;
old-owner handles cannot cancel a new registration. Exhaustion rejects rather
than wrapping. Worker/process replacement additionally requires bridge generation
identity; no handle is valid across those boundaries.

Count interval is 16 ms. Initial count drains immediately, then at most one
latest unequal count per interval. A host must schedule a drain at/after the
next deadline; absent a host drain there is no callback promise. This is the
same explicit scheduling responsibility as the proven drain bridge. Remaining
row events from canceled handles are removed; generation/liveness checks in
bindings also suppress events already extracted by a drain. Recursive delivery
is rejected; callbacks may synchronously read/write/cancel/close after core
extraction. A callback write is delivered on the next drain.

Post-commit observation errors are Notification Err values, not mutation errors.
Dirty groups retry through refresh_notifications or subsequent committed writes;
recovery emits the latest exact snapshot/count. No partial refresh state or
stale row success is published. Typed errors retain existing StorageError
variants; handle exhaustion uses BackendFailure without wrapping. Diagnostic
strings are unstable, do not log payloads. Non-Send public async futures and
exclusive owner guarantees remain API-0004's contract.

Native/browser helpers are test-only bridges, not published SDKs. They translate
owned snapshots/count/error events without reimplementing membership logic.
No C pointer/callback/context outlives the call; callbacks cannot unwind. The
browser wrapper clears stale generations and reports callback errors locally.
No authentication claim or cross-client observation is introduced.

Recovery preserves the most recent successful count delivery time. Invalidating
its value permits a same-cardinality recovery event, while retaining its 16 ms
deadline. Equivalent registration while a group is dirty returns BackendFailure
until refresh or a later committed write recovers it; it cannot allocate a
second cache. A failed candidate refresh discards undelivered pending counts.

## Reviewed failure isolation clarification — SPEC-0028

Only candidate-key failure dirties all modes. Full-row materialization failure
keeps valid keys/counts and retains retry state for affected row listeners.
Explicit refresh retries either kind of failure and queues an error for every
failed explicit attempt. Successful row-only retry publishes exact current rows;
count delivery remains independent, projected and deadline-preserving.
Current in-tree adapters complete without suspension. If post-commit observation
publication is suspended and its future is dropped, subscription registrations
are retained. Completed groups and previously queued snapshots remain available;
incomplete and unvisited groups discard undelivered pending counts and recover
the latest exact rows/counts through refresh or a later committed write. Recovery
preserves the last count delivery time and its 16 ms deadline. Cancellation does
not roll back the committed write or promise that its future will return.
This narrow recovery guarantee (DEF-0061) does not establish general mutation
cancellation safety; a genuinely pending adapter still requires additional
cancellation validation before adoption.

After row-only delivery failure, equivalent count registration uses the valid
candidate cache normally. Equivalent row registration attempts a fresh initial
snapshot; failure leaves no token/cache/listener side effect, success registers
normally. An older failed row listener stays retryable independently.
