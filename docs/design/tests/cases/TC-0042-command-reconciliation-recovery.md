---
id: TC-0042
title: Repeated command edits confirm safely, same-row dispatch is ordered and lost outcomes are rearmed
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-27
parents: [TP-0001]
verifies: [SPEC-0038, REQ-0010, REQ-0017, REQ-0043, REQ-0044, DEF-0032, DEF-0036, DEF-0039]
regression_for: [DEF-0032, DEF-0036, DEF-0039]
tags: [purpose-audit, reconciliation, dispatch, recovery, restart]
---

# TC-0042: Command reconciliation and recovery

State-transition cases with synthetic, offline, task-owned fixtures and injected
clocks. Public-owner cases run on memory and SQLite; RD selection and C1 also
have pure planner unit cases. "Characterization" cases pin current behavior and
must pass before and after the change; they are never relabelled as new
failures. Every other case is a behavioral RED on unchanged code for the stated
reason. Record source, test and toolchain hashes for the RED and GREEN cuts.
"Final rows equal the server sequence" means the rows a backend fake produces by
executing the delivered envelopes in delivery order.

```gherkin
Feature: Safe reconciliation, row-causal dispatch and explicit redelivery
  Scenario: SDK1 typed queue status exposes the row-causal blocker
    Given an eleven-field queue entry with awaiting_key A or null
    When each SDK fetches queue status or receives a status observation
    Then its public entry exposes the exact blocker or absence
    And strict decoders reject malformed awaiting_key and unexpected or missing fields

  Scenario: SDK2 explicit redelivery helpers translate losslessly without sending
    Given an open SDK with a scripted core channel
    When the caller explicitly rearms key K at an exact signed clock sample
    Then exactly one request_redelivery operation carries K and canonical decimal now_ms
    And core Protocol or Busy is returned unchanged without transport or automatic retry
    And no new envelope or outcome is fabricated
    Given managed sync was started and the core rearms an acknowledged record
    Then the helper wakes that driver and exactly one later send uses the original frozen bytes through its transport
    And without a started driver no transport call occurs
    # Compile/public-signature seams are not behavioral RED; assert operation and result.

  Scenario: SDK3 same-row consumer flows follow the changed RD sequence
    Given an existing consumer fixture that expected two same-row envelopes in one batch
    Then its replacement explicitly records the SPEC-0038 dispatch behavior change
    And sends A then receives and reconciles A before sending B
    And preserves both outcomes durable recovery and final-row checks
    # JavaScript/Swift task-owned native/WASM integration; Kotlin/Android execution held.

  Scenario: R1-CTRL the DEF-0032 pair confirms in either order
    Given commands A then B replace the same existing row and both were sent before this change
    When both outcomes are Applied and reconcile runs with empty plans in order A,B and in order B,A
    Then every reconcile is Ok and A and B become non-live
    And the row keeps B's value
    And this fails on unchanged code because reconcile(A, empty) returns RollbackConflict

  Scenario: M1 mapping with later dependent and direct edits confirms without loss
    Given A creates local:x, dependent C edits local:x, then a direct write edits local:x
    When A is Applied and reconciled with mapping local:x to srv1 and no writes
    Then reconcile is Ok and the row is renamed to srv1 carrying the direct edit
    And declared Reference cells, C's undo images and C's queued arguments use srv1
    And no field value other than identities changes
    And reopening the SQLite owner shows the same rows and ledger
    And this fails on unchanged code because the direct edit makes the closure proof conflict

  Scenario: M2 a correction write under a later direct edit stays guarded
    Given M1's state before reconcile
    When A is Corrected and reconciled with mapping local:x to srv1 and a Put on srv1
    Then RollbackConflict is returned and neither the mapping nor any cell nor the ledger changes
    And a following reconcile of A with the same mapping and no writes is Ok
    And an explicit later direct write can apply the corrected value
    And the first two steps are characterization of the unchanged guarded path

  Scenario: M3 a later delete of a created row before mapping
    Given A creates local:x and later command D removes local:x
    When A is Applied and reconciled with mapping local:x to srv1 and no writes
    Then reconcile is Ok, no srv1 row exists and D's undo refers to srv1
    And rolling back D after its rejection restores the row as srv1

  Scenario: M4 invalid mappings keep their errors
    Given A creates local:x and local:y
    When a no-write plan maps to an occupied id, a duplicate target, a local-prefixed id, an id owned by another command, or omits local:y
    Then each returns InvalidCommand and nothing changes
    And this is characterization

  Scenario: RD1 a later same-row command waits for the earlier one
    Given Queued A and Queued B write the same row and A precedes B
    When prepare_push runs
    Then only A is sent and B's status shows awaiting_key A
    And after A's Applied outcome B is still withheld until A is reconciled
    And after reconcile the next prepare_push sends B
    And when A is instead Rejected and processed, B stays withheld while A is not rolled back
    And because B's local write makes rollback(A) conflict, release follows the X2 path
    And an unrelated Queued C with no shared row is sent in the first batch
    And this fails on unchanged code because A and B are sent in one batch

  Scenario: RD2 pre-upgrade sent pairs keep retrying
    Given Sent A and Sent B on the same row created without RD
    When their deadlines pass
    Then both are resent with identical frozen bytes and neither shows awaiting_key
    And an outcome for B before A is accepted as today
    And the possible server reorder is asserted as the documented residual, not as convergence

  Scenario: RD3 without a reconcile callback same-row work waits visibly
    Given a host started sync without a reconcile callback
    And A then B write the same row and A receives Applied
    When the host keeps pushing
    Then B stays Queued with awaiting_key A and no correction or rollback is applied automatically
    And after the application reconciles A explicitly B is sent

  Scenario: X1 correction with later unsent work resolves explicitly
    Given A and later E write the same row, A is sent and E is withheld
    When A is Corrected and reconciled with the correction Put
    Then RollbackConflict is returned with nothing changed
    When the application cancels E, reconciles A with the correction, and issues E's intent as a new command
    Then each step is Ok, the new command is sent only after A is reconciled
    And final rows equal the server sequence

  Scenario: X2 rejection with later unsent work never reports a false rollback
    Given P and later E write the same row, P is sent and E is withheld
    When P is Rejected, processed, and rollback(P) is requested
    Then RollbackConflict is returned, P stays live with rolled_back false and still withholds E
    When the application cancels E and requests rollback(P)
    Then rollback is Ok, rolled_back is true and the row equals its value before P
    And no step reports success while P's rejected cells remain

  Scenario: ET1 entity typed correction over a pending later patch (C4)
    Given entity write A on row r Corrected with a typed correction of field f at time 300
    And a later pending entity patch E on r
    When E patches a different field at time 201
    Then reconcile is Ok, f carries the corrected value and stamp, E's field and stamp are unchanged
    And E's before image equals the corrected baseline and E's frozen bytes are identical
    When instead E patches f itself at time 201, older than the correction
    Then reconcile returns RollbackConflict with rows, metadata and ledger unchanged (rule G2)
    And the different-field variant is a behavioral RED on unchanged code, which returns RollbackConflict

  Scenario: ET2 identity-only entity receipt with a later edit
    Given entity write A and a later local edit of the same row
    When A is Applied with no writes and identity mappings only
    Then reconcile is Ok and the later edit remains
    And this is characterization

  Scenario: ET3 rollback of a processed entity rejection over pending later entity work (C4)
    Given entity write P and a later pending entity patch E on the same field with a newer stamp
    And E is not withheld because entity pairs are exempt from RD
    When P is Rejected and processed and rollback(P) runs
    Then rollback is Ok, rolled_back is true and the current value is E's
    And E's before image carries P's before value and stamp and E's frozen bytes are identical
    And rolling E back afterwards restores P's before value exactly
    And this is a behavioral RED on unchanged code, which returns RollbackConflict

  Scenario: EO2 a correction re-bases a pending later writer that still wins
    Given A sets f at 10 and is Corrected to S at 15, and pending E sets f at 20
    When A is reconciled with the typed correction
    Then reconcile is Ok, current f is E at 20, E.before(f) is S at 15 authoritative, E.after is unchanged
    And a later rejection and rollback of E restores S at 15
    And with a correction at 25 instead, the action refuses because E is not newer (EO3)

  Scenario: EO-ORDER C4 applies correction and promotion only into eff0, before the recompute
    Given A sets f=A at 20 and is Corrected to S at 15, and pending E sets f=E at 20
    When A is reconciled through C4
    Then E.before(f) is S at 15 authoritative
    And current f and E.after(f) are E at 20 and stay authoritative false
    And no second correction or promotion runs over the recomputed row or images
    When the store restarts and E is later Rejected, processed and rolled back
    Then f is S at 15 authoritative

  Scenario: EO4 two pending writers on the corrected field
    Given A is Corrected on f at 15 and pending E1 and E2 set f at 20 and 30
    Then both win, E1.before(f) is S at 15, E2.before(f) is E1 at 20 and current f is E2 at 30
    And with the correction at 25 the action refuses

  Scenario: EO5 rejected P on f and g with pending E on g
    When rollback(P) runs
    Then f returns to P.before(f), g keeps E's value, E.before(g) equals P.before(g) and rolled_back is true

  Scenario: EO14 an untouched field is preserved and pull cannot interleave
    Given A on f and pending L1 on h, with field g untouched by both
    When a pull changes g while A or L1 is live
    Then the pull returns PullConflict on memory and SQLite
    And a pure planner case with a synthetic external g change in B shows g preserved exactly in eff0, every L image and the current row

  Scenario: EO15 a changed correction-only field refuses, publicly and in the planner
    Given entity A on row W that did not declare field g
    And a later entity write E that changed g was Applied and confirmed and is no longer live
    When A's typed correction includes g
    Then reconcile returns RollbackConflict because B(g) differs from A.after(g) (E6)
    And rows, metadata, ledger, status and notifications are unchanged on memory and SQLite
    And separately a pure planner case with a synthetic external g change in B refuses the same way, because pull protection blocks a public pull interleaving

  Scenario: EO17 removal, explicit Null and missing stamps stay distinct through rebase and rollback
    Given optional field x and a field y that has no FieldStamp and is untouched by every record
    And in the first variant K removes x (A Corrected on another field, or P Rejected and processed) and pending L sets x to explicit Null with a newer stamp
    And in the swapped variant K sets x to explicit Null and pending L removes x with a newer stamp
    When C4 reconciles A or rolls back P
    Then L.before carries x exactly as eff0 has it, absent or Null, with its stamp, and L.after carries L's declared state and stamp
    And the current row and metadata equal L.after: a removal is absent and not Null, an explicit Null is present
    And y still has no FieldStamp: a missing stamp is never synthesized, not even as 0
    And L's frozen changes and removed_fields are byte-identical
    When L is later Rejected, processed and rolled back
    Then x returns exactly to L.before: absent stays absent, Null stays Null, and each stamp is retained or missing as recorded
    And both variants run on memory and SQLite, including a restart after C4 and after the rollback

  Scenario: EO16 a finalized change confined to an untouched field is preserved
    Given entity A sets f at 10 and is Corrected to S at 15
    And a later entity write E that changed only g was Applied and confirmed and is no longer live
    When A is reconciled with the typed correction
    Then reconcile is Ok, f is S at 15 and g keeps E's value and stamp exactly
    And in the rollback variant a rejected and processed P on f with the same finalized E on g rolls back with f restored to P.before and g kept, rolled_back true
    And both variants are behavioral REDs on unchanged code, whose whole-row proof returns RollbackConflict

  Scenario: EO-REFUSE-LIVE live or ineligible shapes refuse unconditionally
    Given in turn:
      a live named command on W
      a live cancelled record
      an Applied, Corrected or Rejected unreconciled later record
      an earlier live record outside K
      a chain break
      a pending later stamp equal to or older than the effective one
      a create, delete or tombstone in K or L or in W's current metadata
      a merge-hooked table
      a closure of more than one record or created identities
      a typed Create or Delete of W
    When the correction or rollback runs
    Then it returns the closure-proof mismatch's RollbackConflict with rows, metadata, ledger, status and notifications byte-identical
    And these are characterization cases on unchanged code
    And rollback separately covers a live named command, earlier live work, an Applied unreconciled later record, a delete in L, a hook and a later stamp that does not beat P.before
    And validated pure planner fixtures cover Create K with a synthetic current mismatch, Create K with a dependent in its closure, a rejected Delete P and current tombstone metadata
    And every refused planner result preserves the input ledger and rows

  Scenario: EO-CANCEL cancellation retains its existing conflict transition
    Given a processed rejection P and overlapping later work L whose guarded inverse conflicts
    When cancel(P) runs
    Then it returns RollbackConflict and never uses C4
    And rows and metadata are unchanged and L's images are not rebased
    And P's closure is durably Cancelled and reconciled, not rolled back, and remains live with its recovery data
    And the corresponding status update is expected rather than forbidden
    And this is a characterization of API-0006's existing cancel-conflict behavior

  Scenario: EO-REFUSE-OWNED non-live changes refuse only when they changed an owned field
    Given in turn a finalized entity write, a reconciled named command (including an inherited authoritative stamp, EO10b) and a trimmed record
    And each changed a field that K owns
    When the correction or rollback runs
    Then it returns RollbackConflict with everything unchanged (E6)
    And the same records changing only fields outside O are covered by EO16 instead

  Scenario: EO-PRECEDENCE the fallback never changes earlier errors or C1
    Given an otherwise eligible C4 shape
    When the plan also carries a malformed guard, an unused guard or a guard on W
    Then it returns InvalidCommand before any fallback
    When an Update record's plan also carries an identity mapping
    Then it returns InvalidCommand before any fallback because its created set is empty
    When the plan also carries an outside-closure write, for an entity-origin A a typed correction of another row
    Then the SPEC-0039 RollbackConflict is returned and the fallback is not attempted
    When an Applied or Corrected A is reconciled with an empty plan
    Then the existing C1 success path runs and C4 is never reached
    And these are characterization cases on unchanged code

  Scenario: EO-EXIT the correction-path exit is the empty plan only
    Given a refused correction for an Applied or Corrected entity A
    When the application reconciles A with an empty plan
    Then it is Ok and knowingly drops the correction (C1), and the same exit is not available for Rejected

  Scenario: G1-OPEN pinned open trace: a processed rejected P whose owned field a finalized write changed
    Given P Rejected and processed and a later entity write E on a P-owned field Applied and confirmed
    When rollback(P) and cancel(P) run
    Then both return RollbackConflict and forget returns Protocol
    And after cancel(P), P is Cancelled and reconciled, still live and not rolled back, with rows and metadata unchanged
    And this characterization pins one open G1 trace; G1 is not claimed as fixed or as limited to this trace

  Scenario: ET4 RD exemption applies only to entity-entity pairs
    Given same-row pairs entity-entity, command-entity and entity-command, each earlier record unresolved
    When prepare_push runs
    Then the entity-entity later record is sent in the same batch
    And both mixed later records are withheld with awaiting_key naming the earlier record

  Scenario: C1a-A equal-clock confirmation of A does not promote a different pending value
    Given entity A writes f=A at 20 and a later pending entity E writes f=E at 20
    When A is Applied and confirmed with an empty plan (C1)
    Then E.before(f) holding A's value is authoritative true
    And current f and E.after(f) holding E's value stay authoritative false
    And values, presence and frozen bytes are unchanged, on memory and SQLite and after a restart
    And this is a behavioral RED on unchanged code, which marks current f and E.after(f) authoritative

  Scenario: C1a-B equal-clock confirmation of E first promotes E and not A's images
    Given the same writes, and E is Applied and confirmed before A
    Then current f with E's value becomes authoritative true
    And A's undo images holding A's different value stay authoritative false, as an accompanying control
    When A is later Applied and confirmed
    Then nothing on f changes, because no paired cell holds A's value with A's timestamp
    And this is a behavioral RED on unchanged code: it skips f because E.before(f) and E.after(f) carry equal stamps, so the accepted current E stays authoritative false

  Scenario: C1a-SAME a declared same value with an equal clock promotes
    Given an accepted write and a paired image with the same value, or the same absence, and the same timestamp
    When confirmation promotes
    Then the image is authoritative true even if its flag already differed
    And this runs as a pure planner case if the public API elides a no-op local write

  Scenario: C1a-NULL removal and explicit Null are distinct under equal clocks
    Given an accepted removal of optional x at 20 and a pending explicit Null for x at 20, and the swapped variant
    Then the image that differs in presence is not promoted, and no missing stamp is synthesized
    And both variants run on memory and SQLite, including a restart

  Scenario: C1a-CORRECTED explicitly corrected fields keep their selected stamps
    Given a typed correction of f and an accepted write that declared f and g
    Then f keeps the correction's selected authoritative stamp and only g is a promotion candidate

  Scenario: C1a-DELETE a declared delete promotes only a matching tombstone
    Given an accepted Delete with tombstone time t
    Then only a tombstone with timestamp t and an absent paired row becomes authoritative

  Scenario: C1a-PAIR missing or ambiguous row undo is unknown, not explicit absence
    Given in a pure planner fixture an accepted Delete with a matching tombstone
    And its matching row-undo entry is missing or duplicated, or another record's matching row-undo entry is missing or duplicated
    And duplicate metadata-undo entries for that identity are checked as ambiguous on either record too
    When confirmation promotion runs
    Then no authority is granted from the missing or ambiguous entry, and the affected metadata stays unchanged
    And when only the other record's pairing is unproven, the current metadata still promotes its matching tombstone
    And an existing matching row-undo entry with after None still proves absence and promotes the matching Delete tombstone
    And another record's unique entry with before None stays eligible for its matching tombstone
    And these defensive assertions are RED on the first C1a candidate, which conflates missing entries with explicit absence and accepts the first duplicate
    And each fixture passes ledger.validate before the promotion step
    And no new ledger-validation rule or error is introduced

  Scenario: C1a-MAP promotion uses rekeyed entity identities
    Given an entity Create A and a pending later entity Update E on its row, with equal clocks and different values
    When A is confirmed with a mapping from its local id to a different remote id
    Then current metadata and E's paired images use the remote identity, and only the images holding A's accepted value and timestamp promote
    And the pending E is withheld before mapping, has no frozen bytes, and its queued arguments and undo identities are rekeyed together
    And the ledger survives a fresh SQLite connection
    And this is a behavioral RED on unchanged code, which wrongly promotes E's different equal-clock after-image

  Scenario: C1a-CTRL distinct clocks keep today's promotion
    Given A at 20 and pending E at 21 with different values
    Then promotion matches today's behavior exactly, as characterization
    And in a separate zero-stamp fixture, an accepted write with a present timestamp 0 promotes a matching paired cell whose stamp is present with timestamp 0
    And the accepted before-stamp differs from {0,false}, so the existing stamp-equality skip does not hide the control
    And a paired cell with a missing stamp is not promoted, and no stamp is synthesized for it
    And both zero-stamp controls are characterization cases, not behavioral REDs
    And authority flags already set in stores reconciled by the old rule are not repaired or demoted

  Scenario: CL04 explicit redelivery of a lost outcome
    Given a mixed queue with Acknowledged A without outcome, Sent B, Applied C and Queued D sharing no row with A, B or C
    When request_redelivery(A, r) runs with a valid raw sample r below header.last_ms
    Then A is Sent with due r and unchanged frozen bytes, attempts, key and last_ms
    And the next prepare_push at r sends exactly A's original bytes and D, not C
    And request_redelivery on B is Ok with no write
    And request_redelivery on C, on an unknown key, or with an out-of-range sample is rejected with nothing changed
    And a duplicate receipt for A is a no-op
    And a record withheld behind A is sent only after A's redelivered outcome is reconciled
    And the redelivery survives a SQLite restart before the push
    And the initial facade regression calls the operation by its wire name and
    expects success plus the Sent transition, so unchanged code fails with InvalidRequest
    And a missing Rust symbol or compilation failure is not counted as RED

  Scenario: CTRL-K out-of-closure correction writes are pinned
    Given A writes row r and a direct write edits unrelated row s
    When A is Corrected with a Put on s
    Then the current result is asserted exactly as characterization
    And the case is linked to the open finding O1 rather than changed

  Scenario: CTRL-C existing controls stay green
    Given the C11 and C15 overlap cases, correction_inverse, and the c12 push-order case
    When they run after the change
    Then they pass unchanged, or any same-row batch expectation is relabelled explicitly as RD characterization
```

CL04 assumes SPEC-0035 raw-sample scheduling is already GREEN. R1-CTRL, M1, RD1 and
CL04 are the initial behavioral REDs; M3 and X2 may also
be RED. Characterization cases are ET2, M4, CTRL-K and CTRL-C, plus the
guarded steps of M2. For C4, the behavioral REDs on existing operations are
ET1's different-field variant, ET3, EO2, EO4's winning variant, EO5, both
EO16 variants, the EO17 rebase variants and EO-ORDER. For C1a, C1a-A, C1a-B and
C1a-NULL are behavioral REDs on unchanged code. C1a-SAME, C1a-CORRECTED,
C1a-DELETE and C1a-CTRL pin the rule and today's matching behavior, and run
before C4. EO3, EO-REFUSE-LIVE,
EO-REFUSE-OWNED, EO-PRECEDENCE, EO-EXIT, G1-OPEN and EO15's public refusal are
characterization. EO14 combines a public PullConflict characterization with a
pure planner case, as does EO15's planner variant. Every C4 success is
asserted exactly, with images, frozen bytes and a SQLite restart. C4 correction
and rollback refusals are byte-identical; EO-CANCEL retains its separately
specified status transition. No network, real user store, or held compiler/runtime
operation is needed.

## Recorded component verification (2026-10-27)

C1a was committed as `1b4624e`; C4, including the exact visit-counter follow-up,
was committed as `8efb1af` (tree `85944eba`). The C4 test-first record contains
27 behavioral assertion REDs on unchanged production; the counter fix has four
separate assertion REDs. Characterizations retain their original labels.

The root full workspace run on predecessor tree `29b9f54a` passed 689 tests with
0 failures and 0 ignored, under the default runner and `--no-fail-fast`, including
both capacity-500 checks and encrypted-storage conformance. The exact committed
tree passed 276 focused tests under the default runner, formatting, clippy with
counters on and off (plus the explicit counters library build), and the
core/memory WASM build. Sources were unchanged in both runs. The full run covers
the predecessor: the committed tree differs only in the counter placement and
core test code, recorded in `final-s2/delta-proof.json` under
`target/purpose-remediation/c4-root-integration/`.

C4 success checks now execute the exact whole-image assertions, preservation
checks, intermediate fresh SQLite reopen and later rollback. Refusal controls
include the intentional EO-CANCEL status change; cancel is not a byte-identical
ledger refusal. The unpaired counter fixture explicitly passes ledger validation
and remains unreachable through current writers.

This records component evidence, not completion of every platform case or final
remediation approval. G1, the SPEC-0040 compatibility decision, the final combined
run and the three consecutive clean final reviews remain open. Kotlin/Android
execution remains held. The formal test-case status stays draft.
