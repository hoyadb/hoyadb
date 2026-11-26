---
id: SPEC-0027
title: Phase 3 shared incremental query subscriptions
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0007, REQ-0008, REQ-0009, REQ-0042]
depends_on: [SPEC-0006, SPEC-0007, API-0005, SPEC-0026]
tags: [reactivity, phase-three]
---

# SPEC-0027: Phase 3 reactive queries

Origin: operator instruction to continue the next roadmap phase until its end.
Implement SPEC-0006/0007 on the existing public Rust model and both storage
engines, with actual browser worker/main-thread and Swift desktop delivery.
Preserve API-0004, old tests, private persistence format and runtime dependencies.
No sync/bootstrap, cross-client propagation, packaged SDK, additional predicate
operator, pagination, mobile breadth or new reactive tier is introduced.

## Mechanism and acceptance

API-0005 adds a ReactiveDatabase owner around Database. The same QueryBuilder
supports fetch and observe, observe_count and observe_with_columns. Existing
nonreactive Database remains usable. Every successful public write, including
link/unlink, supplies committed before/after changes to the owner; raw storage
is inaccessible while it is open. Failed writes emit nothing. Process each
atomic batch as one net state transition; insert/remove or edit/revert of the
same row within a batch cannot emit intermediate state. Separate committed
batches retain separate owned row notifications until drained.

Normalize AND predicate order, repeated predicates and In values for sharing.
Equivalent query/navigation definitions share one projected candidate cache
across observation modes and named columns. Bootstrap all matching IDs/order
keys without a limit; maintain the visible ordered/limited window locally.
Caches retain projected keys, not complete application records. Default events
compare visible ID sets, so reorder alone does not emit. Per-column events
additionally compare named own application columns on retained visible IDs;
missing and explicit Null are distinct stored values. Empty column sets mean
default membership. Count emits only cardinality, including limit, and never
materializes full records for initialization, navigation or updates.

Storage projection is an additive API-0005 capability. Reuse adapter-owned
predicate semantics and indexes, selecting only IDs and requested cells.
After commit, collapse repeated changes to first-before/last-after, collect
changed source IDs and indexed reverse-FK dependencies, then re-evaluate only
affected IDs. Sort/order changes and boundary refill use the cached full key
set. Relation navigation resolves existence/FKs/through memberships through
projected reads, not full rows; source/through structural changes may rebuild
the constrained relation key set. This fallback is explicit, restricted to
navigation dependencies, and not a whole-table refresh for ordinary writes.
Invalidation covers direct to-one predicate/order dependencies, optional or
dangling targets, all three navigations, duplicate pivots and cycles. No eager
loading, recursive traversal or normal lifecycle scan is introduced.

Initial registration queues the current snapshot/count. Host-driven drain
returns owned events; no callback runs under a Rust borrow or storage lock.
Row events preserve commit order. Count-only is trailing throttled at a fixed
16 ms minimum between deliveries using a caller-provided monotonic millisecond
clock, coalescing bursts to the latest count and suppressing return-to-last
count. Initial count is immediate. Backward clock samples clamp to the last
sample. Core owns no timer/thread; host schedules drains while counts remain
pending. Cancellation removes queued events and the final observer releases
its shared cache. Closing/dropping releases everything; reopening does not
restore subscriptions. Handles are monotonic and exhaustion rejects.

Observation refresh can fail after a write has committed. Never turn that into
a false write failure or silently return stale rows. Queue a typed observation
error and mark its cache dirty; explicit refresh or a later write retries from
current storage. Successful recovery publishes the current exact snapshot.
Failure to register leaves no observer/cache/token side effect.

## Boundary, lifecycle and quality

The experimental Swift C ABI uses production core/memory path dependencies,
checked integer handles, copied callback payloads and explicit drain. Registry
locks/borrows end before foreign callbacks. Check liveness before each extracted
callback; callback unsubscribe/close suppresses subsequent callbacks, writes
appear on the next drain and recursive drains are rejected. Callbacks run on
the drain caller thread; foreign exceptions/unwind are forbidden. Retain no
foreign callback/context. Browser worker sends owned cloned events; the main
wrapper checks token/generation liveness before every callback, catches callback
errors and continues unrelated listeners, prevents recursive/concurrent drains,
and clears registrations on disposal/replacement. The host chooses UI scheduling.
This reuses Stage 0's drain mechanism without adopting its simplified Store.

TC-0029 enumerates acceptance: original TC-0004 behavior, relation/order/limit,
shared maintenance, exact transaction snapshots, faults/recovery, cancellation,
count throttling and native/browser boundary lifecycles. Write tests and observe
expected behavior RED before implementation. Shared Rust assertions execute in
memory, native SQLite and the actual browser worker; real Swift callbacks and
main-thread browser callbacks prove delivery. Record work counts and repeatable
boundary timing samples, not a new latency budget. All existing regression,
fmt, lint, WASM, traceability and RTM gates remain.

Functional Suitability: original subscription and opt-in semantics. Performance
Efficiency: projected cache, targeted maintenance, sharing and measured work.
Compatibility: additive unpublished API, unchanged storage format/dependencies.
Interaction Capability: explicit handles/events/errors; UX-0002, no visual UI.
Reliability: committed net batches, errors/recovery, cancellation/close guards.
Security: REQ-0042/THR-0016, checked handles and owned data, no logging/egress.
Maintainability: one core engine and shared tests. Flexibility: same engine on
memory/native/browser without runtime dependency. Safety: no physical-system
control; stale/partial data hazards are explicitly covered by THR-0016.

Rollback: reviewed source revert preserving databases and existing user CI
edits. Subscriptions are ephemeral and need no migration. No push/merge/release
or production data operation is authorized by this implementation task.
