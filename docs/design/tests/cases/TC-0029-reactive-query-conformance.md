---
id: TC-0029
title: Incremental subscriptions conform across engines and host bridges
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [TP-0001]
verifies: [REQ-0007, REQ-0008, REQ-0009, REQ-0042, THR-0016]
depends_on: [SPEC-0027, API-0005, UX-0002]
tags: [reactivity, reactivity]
---

# TC-0029: reactivity acceptance

Use synthetic exact rows and deterministic clock samples. Shared cases R01–R12
run against memory/native SQLite and the real browser worker. R13–R15 require
actual host boundary and work/timing evidence. Original TC-0004 maps to R01–R05.

```gherkin
@REQ-0007 @REQ-0008
Feature: One query supports fetch and cancelable observation
Scenario: R01 initial snapshots are owned and default emissions follow membership
 Given three pending orders and one query used for fetch and observe
 When initial notifications drain then an order enters or leaves the set
 Then exact initial and changed complete rows match fetch semantics
 And unrelated matched field edits and reorder alone do not emit
 And equal-cardinality ID replacement does emit
Scenario: R02 cancellation close and handle lifecycle stop delivery
 Given equivalent live observers with queued notifications
 When one cancels then the last cancels or the database closes
 Then canceled events cannot deliver and the last cache is released
 And a stale handle from this or a different owner cannot cancel another registration
 And zero observers cause zero maintenance work
Scenario: R03 net transactions never publish intermediate or failed state
 Given observed records
 When a batch inserts and removes a row or edits and restores a field
 Then no intermediate notification is queued
 When a batch fails after a valid prefix
 Then no event or cache change occurs
 And separate successful batches retain separate owned snapshots in commit order

@REQ-0009
Feature: Explicit observation modes preserve their cost and timing contract
Scenario: R04 named columns add only their own stored changes
 Given default and per-column observers of the same query
 When a matched tracked column changes
 Then only the finer observer emits unless membership also changes
 And unrelated columns do not emit and missing versus explicit Null is distinct
 And invalid or managed column names reject without registration side effects
Scenario: R05 count is projected and trailing throttled
 Given count observers with a deterministic millisecond drain clock
 When initial count drains then ten writes arrive within sixteen milliseconds
 Then no complete application record is materialized for count maintenance
 And only the latest unequal count emits at the deadline
 And equal counts and a burst returning to last count do not emit
 And backward clock samples cannot bypass the throttle

@REQ-0007 @REQ-0008 @REQ-0042
Feature: Shared incremental maintenance handles the whole supported query model
Scenario: R06 equivalent queries share cache work and preserve independent listeners
 Given equivalent AND and In queries with multiple observation modes
 When observers register and a relevant write commits
 Then one key cache is bootstrapped and one affected-ID maintenance pass runs
 And each observer receives its own typed owned notification
Scenario: R07 ordered limited windows refill from maintained candidates
 Given more matching records than the limit including ties and optional order keys
 When a boundary row enters leaves changes sort key or is removed
 Then selected rows and deterministic own-id ties match fetch
 And membership default ignores reorder-only changes
 And limit zero empty matches and oversized limits remain valid
Scenario: R08 to-one predicate and order dependencies invalidate exact source IDs
 Given optional dangling cyclic and shared target references
 When a consulted target field changes or that target is inserted or removed
 Then all and only dependent source candidates are refreshed
 And unrelated table or field writes do not trigger full query reevaluation
Scenario: R09 every relation navigation supports observation
 Given to-one has-many and many-to-many queries with filters order and limit
 When a source reference source existence target or pivot membership changes
 Then exact visible target sets update and duplicates or dangling pivots do not synthesize rows
 And count navigation uses only projected source membership and target cells
Scenario: R10 observation refresh errors do not misreport committed writes
 Given an adapter that fails a projected read or row materialization after commit
 When a public write succeeds but observation refresh fails
 Then write success remains true and a typed error notification is queued
 When explicit refresh succeeds
 Then the exact current result recovers without stale success or partial cache publication
Scenario: R11 projection parity uses exact portable values without full records
 Given all primitive values optional null direct to-one fields and bound Unicode names
 When projected predicates order limit and requested cells execute
 Then complete projected keys match portable query semantics
 And invalid paths types and physical tables reject even at limit zero
Scenario: R12 raw no-op link unlink and persisted reopen integrate safely
 Given managed membership and ordinary observed writes
 When identical replacement duplicate link or absent unlink is requested
 Then no false event emits
 When actual link or unlink changes a target set
 Then one net membership event emits
 And reopening retains data but restores no old subscriptions

@REQ-0042
Feature: Host delivery and work are measurable
Scenario: R13 real browser callbacks enforce generation cancellation and recovery
 Given the production Rust core and SQLite in its dedicated OPFS worker
 When main-thread callbacks read write unsubscribe close throw or request a recursive drain
 Then owned events deliver on the chosen main thread without borrowed Rust state
 And canceled or old-generation events cannot deliver
 And callback errors do not prevent unrelated listeners or misreport a write
Scenario: R14 real Swift callbacks run outside registry locks
 Given the production Rust core behind the checked native C ABI
 When a real Swift callback reenters read write unsubscribe close or drain
 Then no lock or Rust borrow spans the callback and recursive drain rejects
 And liveness is checked before every callback on the drain caller thread
 And later drains deliver callback writes while canceled events are suppressed
Scenario: R15 work and boundary timing are repeatable observations
 Given increasing unrelated row counts and multiple equivalent observers
 When direct and referenced writes change one result
 Then instrumented projection calls restrict ordinary updates to affected IDs
 And count produces zero full record materializations
 And equivalent observers share maintenance and final unsubscribe releases it
 And native and browser callback latency samples record runtime and scheduling cadence
```

Preserve every prior regression and prove real behavior RED before code. Record
exact commands, case mapping, measured work, browser/native delivery and limits.
No performance SLO, remote CI, crash durability or consumer release is inferred.

## reactivity execution
