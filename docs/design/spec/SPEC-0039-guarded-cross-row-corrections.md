---
id: SPEC-0039
title: Guard authoritative corrections outside a command optimistic write set
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-27
parents: [SPEC-0029]
satisfies: [REQ-0043]
depends_on: [DEF-0038, SPEC-0038, API-0006, API-0011]
tags: [purpose-audit, corrections, integrity, compatibility]
---

# SPEC-0039: Guarded cross-row corrections

## Problem and scope

The source-bound DEF-0038 diagnostic demonstrates an existing Put on unrelated
row s overwriting a later local edit while reconciliation returns Ok. The
originating command's closure proof covers row r, not s. REQ-0043 prohibits
overwriting unrelated later changes. Simply disabling all server effects on
other rows would remove a legitimate integration capability.

This is proposed behavior for independent review before implementation.
SPEC-0038's no-write confirmation, dispatch, identity mapping and redelivery
remain unchanged. No new stored field, ledger format, dependency or automatic
conflict policy is introduced.

## Proposed contract

1. Add public `CorrectionGuard { table: String, id: String,
   expected: Option<BTreeMap<String, Value>> }`. The expected fields are the
   complete public before-image, without a managed version field; None means
   absent. This is an explicit application expectation, never inferred from a
   backend's desired after-image or supplied as a completeness Boolean.
2. Add `CommandDatabase::reconcile_guarded(key, plan, guards)` alongside the
   existing reconcile method. The facade operation is `reconcile_guarded` with
   exact arguments `{key,plan,guards:[{table,id,expected:Fields|null}...]}`.
   The existing operation supplies an empty guard list internally. SDK helpers
   expose this additive capability using the existing lossless fields codecs.
3. Validate guard tables, ids, field types and reserved-field exclusion. Reject
   duplicate or unused guards as InvalidCommand. A guard addresses a row written
   by the plan, in its pre-reconciliation identity. Every guard compares with the
   original committed public snapshot, before any planned mapping, rename or
   reference rewrite. A mismatch returns RollbackConflict.
   A guard for a closure-owned row, including that row's explicit mapping
   target, is unused and returns InvalidCommand: its existing closure proof
   already applies, so no second expectation is accepted for it.
   Expected fields compare exactly with the public stored map: an omitted
   optional field is distinct from an explicit Null, both of which row validation
   admits without normalization. `id` and every `__hoya_` field, including
   `__hoya_schema_version`, are forbidden in expected fields. Validate all guard
   shapes, duplicate identities and unused identities before returning a data
   conflict; malformed guards return InvalidCommand first.
4. Derive the command closure's written identities from existing undo. Relate
   those identities to explicit mapping targets when classifying plan writes;
   a renamed command-owned row must not become an unrelated row accidentally.
5. Every Put or Remove outside that write set requires a matching guard,
   including a write to an absent row. Without it, return RollbackConflict.
   An expected-absence guard explicitly authorizes an insert or no-op removal
   only while the row remains absent. Multiple ordered writes to one identity
   share one initial guard. This also prevents an unguarded insertion from
   resurrecting a row removed by a later direct local edit.
   A guarded Remove of an absent row counts as using its expectation even when
   no application row changes. Every supplied expectation is also a storage
   RowGuard in the atomic commit, including no-op writes and expected absence.
   API-0007 therefore permits read-only guards (mutated rows are a subset of
   guarded rows), rejects duplicate or unmanaged-table guards even for absent
   expectations, and retains the exact Reset inventory rule.
   Checking only the planner snapshot does not satisfy the comparison guarantee.
   The core planner still guards exactly the rows it mutates; the only additional
   guards are the caller's explicit SPEC-0039 expectations. Any other surplus
   guard is a core Protocol error.
   - If any live record outside the reconciling closure has that identity in
     its undo, return RollbackConflict regardless of the guard. A caller cannot
     overwrite another unresolved command's optimistic value by supplying it as
     an expectation.
   - A non-entity Put or Remove outside the closure on an entity-managed table
     returns RollbackConflict regardless of the guard. This cut does not permit
     bypassing the table's comparison metadata.
   - A typed Entity correction outside the closure also returns
     RollbackConflict. The guarded API in this cut authorizes ordinary
     cross-row writes only on tables outside entity sync. Typed corrections
     within the closure retain their full existing proof and separate overlap
     review; pull remains the entity server-effect path.
6. Guarded writes do not relax the existing optimistic closure proof for plans
   containing writes. All checks finish before a mapping, data or ledger change
   is persisted. Guards, mappings, schema validation, stale-state checks and
   data mutations participate in one existing atomic storage plan.
7. On conflict, retain the outcome pending, with all data, mappings, recovery
   records and notifications unchanged. The application may obtain current rows
   and explicitly choose a new guarded plan. No correction is silently skipped,
   no automatic retry substitutes a fresh expected image, and no empty plan is
   fabricated to claim convergence.

## SDK surface

The additive helpers use API-0011's `reconcile_guarded` operation.

- Guard type:
  - JavaScript: `interface CorrectionGuard {table:string;id:string;expected:Fields|null}`.
  - Swift: `public struct CorrectionGuard: Equatable, Sendable` with
    `public let table: String`, `public let id: String`,
    `public let expected: [String: Cell]?` and a memberwise public init.
  - Kotlin: `class CorrectionGuard(val table: String, val id: String,
    expected: Map<String, Cell>?)`. It keeps an immutable copy of `expected`
    and has a redacted `toString`, as the existing plan models do.
- Helper, alongside the unchanged `reconcile`:
  - JavaScript: `reconcileGuarded(key:string,plan:Plan,guards:CorrectionGuard[]):Promise<null>`.
  - Swift: `public func reconcileGuarded(_ key: String, plan: ReconcilePlan,
    guards: [CorrectionGuard]) async throws`.
  - Kotlin: `suspend fun Database.reconcileGuarded(key: String,
    plan: ReconcilePlan, guards: List<CorrectionGuard>)`.
- Encoding: one `reconcile_guarded` call with `{key,plan,guards}`.
  - The plan is encoded exactly as for `reconcile`.
  - Each guard is `{table,id,expected}`, in the caller's order. `expected` is
    `null` for absence, or the fields object in the existing lossless cell codec
    (Swift `Cell.wire`, Kotlin `fieldsWire`, JavaScript the owned-copy encoder).
  - An omitted optional field stays omitted and an explicit Null cell stays
    present. The SDKs do not normalize, sort, deduplicate or validate guards
    beyond their existing codecs; the core performs every check above.
  - JavaScript forwards guard and plan objects as given. Values the owned-copy
    encoder rejects, such as `undefined`, fail as they do for other arguments.
    A guard object without `expected` reaches the core and is rejected there.
- Snapshot: arguments are captured when the call is made. JavaScript encodes
  synchronously before its first await; Swift arguments are values; Kotlin
  guards and plans are immutable copies.
- Effects: identical to `reconcile`. On success the SDK schedules observation
  delivery and wakes a started managed sync driver, exactly as for `reconcile`:
  JavaScript through its ordinary mutation path, and Swift and Kotlin by adding
  `reconcile_guarded` to their observed-mutation sets. On failure nothing is
  scheduled.
- Errors: API-0006's reconciliation validation precedence and exact storage
  guard-conflict mapping apply. Non-guard storage failures are never converted
  into RollbackConflict. The core's error is returned unchanged, including RollbackConflict,
  InvalidCommand, Protocol, Busy and Closed. There is no automatic retry, no
  refreshed expectation and no fallback to unguarded `reconcile`.
- Managed sync driver: the existing reconcile callback type is unchanged and
  returns only a Plan; the driver still calls unguarded `reconcile`.
  - A callback plan that writes outside the command closure now returns
    RollbackConflict and changes nothing.
  - In all three SDKs a RollbackConflict from that core reconciliation is
    reported through the existing sync failure path and Swift and Kotlin
    `onError`, and the pass continues to later pending entries. It never retries
    that key within the same pass, including across outgoing pages; a later
    independently scheduled pass may invoke the callback again. Skipping a
    conflict creates no wake or retry timer by itself. The failed entry remains
    pending and its row-causal dispatch block remains intact.
  - Catch only RollbackConflict from the core reconcile operation. Callback
    failures and other core errors keep the existing failure behavior. Do not
    fabricate a plan or refresh an expectation. JavaScript and Swift also emit
    the existing SPEC-0036 S fact; Kotlin background reporting remains
    unimplemented while held and is not claimed by this slice.
  - An application needing guards either configures no managed callback, or
    returns only closure-owned plans from it. It then calls `reconcileGuarded`
    after `pendingReconciliations()` and its own read of the current rows.
  - A guarded managed callback is not part of this slice.
- Kotlin implementation and tests remain static and unexecuted while
  JVM/Android execution is held.

## Compatibility decision proposed for review

Existing reconcile signatures and ReconcilePlan construction stay intact. The
new method and wire operation are additive. Existing unguarded cross-row
writes, including server-created rows, become explicit conflicts; applications
intentionally applying such server effects must supply guards, including an
absence expectation for an insert. This tightens behavior rather
than changing stored bytes. A freshly read expected image explicitly authorizes
that value's replacement, while a local edit after it was read is preserved by
the failed comparison. An edit made before the application's read may be
replaced with its explicit consent; the guard does not preserve that edit or
prove the backend's business correctness. Other live command intents and
entity comparison metadata remain protected even with that consent.

The alternative requiring no new public API was to reject every out-of-closure
write. An application could confirm with an empty plan and make an explicit
local write afterwards, but those two operations are not atomic. Under CHG-4,
the operator chose **"Add guarded correction API (Recommended)"** in response to
the question describing atomic explicit value or absence expectations, stale
expectation rejection, and protection of other live commands and entity metadata.
This approves building and testing the additive contract above, after independent
spec and case review. It authorizes no operation on a user's store. The exact
question and answer are preserved in
`target/purpose-remediation-prep/correction-guard-approval.json`.
No unguarded absent-row exception is proposed as a fix.

TC-0043 supersedes TC-0042 CTRL-K's old-behavior characterization once this
correction is implemented. Until then, DEF-0038 stays open. Review and the
concrete compatibility disposition precede test code.

## Verification and rollback

TC-0043 defines regression and preserved controls on memory and SQLite, plus
facade and SDK codec checks. First reproduce an unguarded lost update on the
unchanged implementation, then assert conflict with exact byte preservation.
Record source, test, toolchain and executed binary hashes for RED/GREEN.
Do not count a missing Rust symbol as RED. Run the existing reconciliation,
mapping, entity and callback controls after the change.

Test isolation is part of this verification. The Unproven racing wrappers share
ADR-0037's process-global coordinator. Concurrent independent cases in one test
binary previously competed for it and failed setup with Busy under the default
workspace runner. A test-only, poison-tolerant shared mutex now surrounds each
whole guard-race case, including teardown. The deliberate two-handle storage race
inside a case and every assertion remain unchanged. The ordinary default
parallel runner must pass without thread-count flags, ignored cases or weaker
checks; TC-0043 GW18 pins this. Other multi-case Unproven fixtures are audited
for the same isolation requirement.

Reverting source and contracts changes no ledger format but restores the known
unsafe overwrite behavior. Preserve every diagnostic and RED/GREEN packet. No
user store is opened, migrated, reset or deleted by remediation.

## Quality attributes

- Functional Suitability: retain explicit server effects without unnoticed loss.
- Performance Efficiency: compare only provided guards in addition to the
  existing planner; scaling changes remain SPEC-0037's responsibility.
- Compatibility: additive API; disclosed tighter legacy behavior; no format change.
- Interaction Capability: explicit conflict and caller-controlled expected value.
- Reliability: all-or-nothing mapping, data, ledger and event publication.
- Security: existing bounded lossless codecs and redaction; no raw errors or secrets.
- Maintainability: one shared guarded planner rather than SDK merge algorithms.
- Flexibility: the application chooses conflict resolution through the existing seam.
- Safety: no physical-control application; preserve all stored state on conflict.
