---
id: SPEC-0040
title: Choose stop or discard while keeping conflict outcomes truthful
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-27
updated: 2026-11-10
parents: [SPEC-0029]
satisfies: [REQ-0017, REQ-0043]
depends_on: [SPEC-0038, SPEC-0039, API-0006, API-0007, API-0011, DEF-0032]
tags: [conflicts, configuration, recovery, proposed]
---

# SPEC-0040: Simple command conflict policy

## Need and approved decision

The operator chose “Discard queued command; keep current row” and requested
simple configuration covering realistic conflicts without overengineering.
The approved design uses two reactions and one manual action. It does not
implement row deletion or forced rollback. On 2026-10-27 the operator approved
the reviewed Stop/Discard design and its stored compatibility consequence,
noting that the library has not been released and may be modified. This permits
the API and stored-state changes specified here; it does not authorize edits to
user stores or claim that the feature is implemented.

The checked baseline in `command/records.rs` keeps a rejected record live until
rollback completes. `reconcile.rs::inverse` returns RollbackConflict when a later
direct edit differs from the recorded after-image. SPEC-0038 then withholds later
same-row commands. Clearing undo without a new state is unsafe: an old binary
could run an empty inverse and report rolled_back=true without restoring data.

## Developer configuration

The host option is `conflict: { default: "stop" | "discard", commands?:
{ "<versioned command name>": "stop" | "discard" } }`, defaulting to Stop.
An exact recorded `record.command` name override wins. An override for an unknown
command name is accepted and inert until such a command exists. There is no table/row policy hierarchy,
arbitrary callback deciding writes, or automatic retry. Per-row compare-and-set
remains SPEC-0039's guarded correction API.

Core uses `ConflictPolicy { default, commands }` with the same two values and a
`set_conflict_policy` operation. The policy belongs to one open owner, is never
persisted, and is separate from persisted SyncOptions. Reopening starts at Stop
until the host reapplies its options. The facade open frame is unchanged. SDKs
apply configuration before returning a usable database; if configuration fails
or close invalidates that attempt, they dispose the new owner before returning.
Policy updates use the existing idle admission rule; an admitted handler gives
Busy and leaves the previous policy intact. Existing command-name validation
and codec size bounds apply to the override map.

`stop` returns the existing conflict and changes nothing. `discard` means
**keep the device's current data**, retire only eligible pending command work,
and report the decision. It can leave the device different from the server.
It never claims that a rejected command succeeded or that rollback happened.

## When the option applies

Consult policy only for a closure-proof mismatch: after ordinary state,
mapping, guard and SPEC-0039 protection checks pass, the inverse proof of this
command's own dependent closure finds a different current image. Do not catch
every RollbackConflict and turn it into discard.

Ordering with SPEC-0038 C4: C4 is part of the normal correctness proof and runs
first. Only a closure-proof mismatch that C4 does not resolve can reach the
policy. The EntityRecord refusal below is unchanged; approval does not bypass
C4 or any earlier protection.

| Situation | Stop | Discard |
| --- | --- | --- |
| Accepted repeated offline edits | Normal SPEC-0038 confirmation | Same |
| Later command waiting for an earlier outcome | Keep waiting; expose awaiting_key | Same |
| Rejected command with unchanged optimistic rows | Restore before-images normally | Same |
| Rejected command followed by an unrecorded direct edit | Conflict; retain pending recovery | Keep every row; mark eligible records discarded |
| Correction conflicts with a later edit in its own closure | Conflict; retain pending correction | Apply valid mappings, skip all correction writes, confirm |
| Stale caller guard or protected unrelated row | Conflict, unchanged | Same |
| Network, Busy, Closed, capacity, size or protocol error | Existing error | Same |

The same three refusals apply to both rollback and correction paths. A later
live record outside the dependent closure that wrote any affected row refuses
the action (LaterWorkOnRow). Cancel that later unsent command first, then resolve
the earlier outcome and reissue the intent (SPEC-0038 X1/X2), or wait for the
outcome of later work already sent before the upgrade. Refuse the
entire action if any affected record has an entity descriptor, an undo row in
an entity-managed table, or metadata_undo (EntityRecord). Refuse in-flight
descendants (InFlight). These are three fixed refusal conditions, not a policy
framework; the public error remains RollbackConflict, with no data change.

## Manual action and atomic results

The configured policy covers reconcile and rollback only. `cancel()` keeps its
existing behavior, including retaining recovery data when its inverse conflicts.
The explicit discard action can retire that never-sent Cancelled record.
Configured Discard on rollback applies only to a processed Rejected root. A
Cancelled-root rollback mismatch stays RollbackConflict under either policy.

`discard(key)` is explicit and independent of configuration. Its root must be
an unsent Queued record (no frozen bytes, attempts zero), or a processed Rejected
record that was not rolled back, or a Cancelled root that was never sent
(frozen None, attempts zero) and still retains unresolved recovery data. Sent,
Acknowledged, Applied, Corrected and Cancelled-after-send roots return Protocol. A processed rejection may retain frozen bytes;
its terminal outcome makes it eligible. The same holds after a conflicted
`cancel()` of that processed rejection: the record is then Cancelled, not rolled
back, and still stores its original Rejected outcome, so it stays eligible as a
root or descendant. "Cancelled-after-send" in this spec means a sent Cancelled
record with no stored outcome, whose in-flight copy may still execute.

Use the existing transitive created-ID dependency closure, not every later
command. Check the whole closure before changing anything. Each still-pending
descendant must be unsent Queued, processed Rejected or never-sent unresolved
Cancelled. A Sent, Acknowledged or
Cancelled-after-send descendant refuses the whole action. Already discarded or
rolled-back records remain unchanged and are not reported as newly discarded.
No row is removed, replaced or partially restored. Created local IDs stay in the
data, are listed as unmapped, and cannot be treated as mapped server identities
by subsequent commands. The existing UnknownLocalReference behavior applies.

On the correction path, Applied/Corrected records never become discarded.
Keep all valid mappings, skip every write in the plan (including guarded outside
writes), and confirm as an explicit empty-plan reconcile would. The outcome
stays Applied or Corrected; this is consent to keep local data, not convergence.
Protections and guard validation run before this choice, never after it.
Every caller SPEC-0039 expectation remains a storage RowGuard in the atomic
commit even when its write is skipped. A race that invalidates that guard is
RollbackConflict at commit and never triggers another policy choice.

Add reporting operations `reconcile_resolving`, `rollback_resolving` and
`discard`, sharing one planner with existing operations. The guarded reporting
path reuses SPEC-0039 expectations; it does not implement a second guard system.
Existing reconcile/rollback signatures stay unit/null and obey configured
policy by delegating to that planner and dropping only the report. Thus, under
Discard, Ok from rollback can mean current data was kept; read status or use
the reporting method to distinguish it. Default Stop preserves prior behavior.

The bounded result is `Completed` or
`KeptCurrent { discarded_keys, unmapped, skipped_writes }`:

| Path | discarded_keys | unmapped | skipped_writes |
| --- | --- | --- | --- |
| Rollback or manual discard | Newly discarded root and eligible dependents | Their created IDs without mappings | 0 |
| Correction confirmation | Empty | Empty, because valid mappings apply | Number of skipped plan writes |

The report describes this transition. Normal history trimming may remove a
newly discarded record in the same commit, so a reported key is not a promise
that a later status lookup will still retain that record.

Encode and size-check the exact result before commit. An oversized result is
LimitExceeded with no change, even for a unit-returning caller. SDKs always use
the reporting path internally and send KeptCurrent to optional
`onResolution(key, resolution)`. Recommend this callback for any Discard policy.
Contain callback failures through the existing background-error mechanism;
they cannot change the committed result. All SDKs keep onResolution installed
after a failure and report each failing invocation once as
{source:"callback",kind:"CallbackFailure",token:null}; there is no listener token.
Suppress old-epoch reports after close/reset and contain sink failures. The
callback is not retired and its failure is never relabelled as a sync failure.
Kotlin's existing API-0011 sink gap is tracked by DEF-0040; a callback-only
source correction is not full reporting parity, and JVM execution remains held.
No new event framework is introduced.

## Stored truth and approved compatibility

Use a new `Discarded` / `"discarded"` record state. It has rolled_back=false,
reconciled=true, no entity descriptor, empty metadata_undo, and outcome None
(unsent) or the original Rejected outcome. Keep created entries, undo and frozen bytes unchanged;
they are history, not an implied recovery API. Discard also clears each newly
discarded record's reference_paths and dependencies in the same atomic plan.
Args, created, undo, frozen bytes, attempts, due_ms, outcome and mappings stay
unchanged; only state, reconciled, rolled_back and that retired dispatch
bookkeeping may change. This rule applies to eligible Queued, processed Rejected,
never-sent unresolved Cancelled and rejected-then-cancelled unresolved records. Empty reference_paths and dependencies
are required by the Discarded legal state, preserving existing reference and DAG
validation without an exception. Non-live records do not extend producer
retention. Never-sent Cancelled cleanup must not clear created or undo on
Discarded history; those remain until the record itself is removed.
Field preservation describes the discard transition, not an exemption from
existing later global identity rewrites: producer mappings still rewrite undo
identities and Reference values on retained Discarded records, without changing
their args or frozen bytes. Cleared dependencies exclude them from later
producer rollback or correction closures. A dependent may be discarded while
its producer remains live; a later producer rollback can remove the producer's
row while the kept dependent row still references it. This is a keep-current-data
consequence under existing Reference typing, which does not guarantee target
existence; no new live-producer refusal is introduced.
It is not live, does not dispatch,
and resolves SPEC-0038's same-row blocker. Emit the existing status update; do
not fabricate an outcome. A sent discard retains the same Rejected outcome,
so discarding publishes no new Outcome event; an unsent discard has no outcome
and publishes only its status change. Whole-closure state and the bounded report commit in
one atomic storage plan. Failed persistence preserves the original ledger.

Every operation treats a discarded record explicitly:

| Operation | Result |
| --- | --- |
| rollback, cancel, reconcile, request_redelivery | Protocol, never an empty inverse |
| receive of its identical stored Rejected outcome, or receipt-only None after it was sent | No-op |
| conflicting outcome, or any receipt/outcome for a never-sent discard | Protocol |
| forget and history trim | Existing non-live rules; a Discarded producer may be removed while Discarded dependents remain, with their retired bookkeeping empty |
| pending_reconciliations | Excluded |

All SDK status decoders/enums and exhaustive matches must change together.
The ledger format number stays unchanged, but this is a **stored semantic
compatibility break**: existing decode_state rejects the unknown state, so older
app versions refuse to open a store while a discarded record remains. History
trimming depends on later activity and the configured history limit; there is
no time guarantee or promise that downgrade becomes possible. Turning the
policy back to Stop prevents new discards but does not remove existing records.
No automatic conversion, downgrade tool or user-store edit is authorized here.

The operator approved this fail-closed older-version behavior on 2026-10-27.
The unreleased-library clarification permits the specified compatibility break;
it does not promise that an older build can open discarded history. Current
rows remain intact, compatible new builds report Discarded truthfully, and
history trimming has no guaranteed removal time. Never substitute an unmarked
record that an old binary could misreport as rolled back.

The design approval resolves the operator decision only. This artifact remains
draft while test-first implementation and verification under TC-0044 are pending.

## Verification, quality and rollback

TC-0044 covers configuration, the situation table, refusal boundaries, guard
precedence, reports, restart/fault atomicity and a real old-binary refusal probe.
New operations first get signature-only seams; compile errors are not RED.
No implementation or passing test claim is made in this draft.

Functional suitability: two explicit reactions with truthful outcomes.
Performance: bounded record closure and report, no retries or timer policy.
Compatibility: default Stop and disclosed stored-state downgrade refusal.
Interaction capability: one default, optional exact command overrides, reports.
Reliability: atomic plans and unchanged state on every failed preflight/commit.
Security: public identifiers only in reports; existing payload/error redaction.
Maintainability: one planner and the existing guard and event mechanisms.
Flexibility: per-owner ephemeral configuration and one explicit manual action.
Safety: no silent deletion, fake rollback, in-flight discard or entity bypass.

Before any discard is stored, reverting the feature restores the prior behavior.
After one is stored, reverting to an older binary may refuse open; preserve the
store and use a compatible binary. No destructive rollback procedure is proposed.
