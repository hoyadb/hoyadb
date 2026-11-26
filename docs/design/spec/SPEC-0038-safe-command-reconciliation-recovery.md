---
id: SPEC-0038
title: Confirm repeated command edits safely, order same-row dispatch and rearm lost outcomes
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-11-10
parents: [SPEC-0029]
satisfies: [REQ-0010, REQ-0017, REQ-0043, REQ-0044]
depends_on: [API-0001, API-0006, API-0007, API-0009, DEF-0032, DEF-0036, DEF-0039, SPEC-0035]
tags: [purpose-audit, reconciliation, dispatch, recovery]
---

# SPEC-0038: Safe command reconciliation and recovery

## Need and verified baseline

DEF-0032: two commands A and B replace the same existing row offline and both
receive Applied. `reconcile(A, empty)` returns RollbackConflict forever and A
stays live. Cause, verified at 79ab4b0: `reconcile` (`command/reconcile.rs:20`)
proves the whole inverse of `closure(A)` against current rows
(`simulate_inverse`, `simulate_metadata_inverse`) before applying any plan,
including an empty one. `closure` follows created-local-ID dependencies only, so
a later write to the same existing row makes `A.after` differ from the current
row although nothing would be written.

DEF-0036 CL04: an Acknowledged record whose final outcome is lost has no path
back to delivery. Automatic resend after receipt stays forbidden.

Undo stores whole-row before/after images (`CommittedChange`). Rebasing a later
record's before-image while keeping its visible after-image retains rejected or
uncorrected cells. That diverges silently and is rejected by this spec (see
"Rejected designs").

## Scope of this cut

In scope:
- C1 confirmed no-op reconciliation.
- C2 row-causal dispatch (RD) with a visible `awaiting_key`.
- C3 explicit redelivery of a lost outcome, plus the backend deduplication rule.
- C1a value-aware confirmation promotion (DEF-0039), implemented before C4.
- C4 proven entity fallback at the closure proof, for the correction or
  rollback of a single existing-row entity Update (the O2 slice). Every other
  entity overlap stays refused.

Unchanged: non-empty correction plans keep the existing guarded closure proof
and its RollbackConflict. Rollback, cancel and forget are unchanged except for C4's
single entity Update rollback path. Entity typed corrections are unchanged except
for C4. Persisted format is unchanged.

Out of scope, recorded:
- O1 out-of-closure correction writes (DEF-0038, below).
- O2 entity overlap beyond C4 stays refused: multi-record closures, mappings or
  created identities, mixed command and entity live chains, unreconciled later
  records, finalized changes to owned fields, creates, deletes, tombstones and
  hooks.
- G1 stays open. A pinned trace is a processed rejected entity P whose P-owned
  field was changed by a finalized write: it has no exit (see C4). No other
  ineligible state gains a new guaranteed exit. No DEF-0032 closure claim.
- O3 opt-in abandonment of a rejected inverse (proposed separate API, below; not
  implemented in this cut and not a recovery success).

## C1. Confirmed no-op reconciliation

Applies when `reconcile(key, plan)` targets an unreconciled record whose
outcome is Applied or Corrected and `plan.writes` is empty (no Put, Remove or
Entity write).

1. Every existing precondition stays: `ledger.validate`, `validate_snapshot`,
   state, schema version, `plan.mappings` keys equal `created` keys, and the
   mapping target checks (empty, `local:` prefix, duplicate, occupied, owned by
   another command) with their existing errors.
2. Skip `simulate_inverse` and `simulate_metadata_inverse`. No cell is written,
   so no later local edit can be overwritten.
3. Everything after the proof runs exactly as today, in the same SyncPlan:
   entity metadata key move, owned row rename, `rewrite_row` of declared
   Reference cells, undo and metadata-undo identity rewrite, `promote_metadata`,
   clearing the producer's undo, queued argument normalization with
   `check_send_size`, `finish` and `trim`. A never-sent Cancelled record that a
   conflicted cancel retained is normalized like queued work (without the send
   size check, since it is never sent), so no retained reference names a
   mapped-away identity.
4. A Corrected outcome with an empty plan means the application chose not to
   apply the backend's correction. This is its existing translation duty
   (API-0006 "Authoritative local apply"). The core never discards a correction
   on its own, and this case is not reported as convergence.

Non-empty plans keep today's full closure proof; no relaxation.

## C1a. Value-aware confirmation promotion (DEF-0039)

Applies wherever confirming an accepted entity write X promotes optimistic
stamps to authoritative (`promote_metadata`). That covers C1 empty-plan
confirmation, typed corrections and C4's eff0. Today promotion compares
FieldStamp equality only. Because equal clocks are legal for independent
pending writes, it can promote a different pending value (DEF-0039), and it can
fail to promote an accepted one. Both orders are defects:
- **A confirms first.** A and a later pending E both stamp f at the same time, with
  different values. Today the current cell (E's value) and E.after(f) become
  authoritative. Required: only E.before(f), which holds A's value, is promoted;
  the current f and E.after(f) stay authoritative=false.
- **E confirms first.** Today `promote_metadata` skips f entirely, because
  E.before(f) and E.after(f) carry equal stamps. The accepted current E therefore
  stays authoritative=false, a false negative. A's images also stay false.
  Required: the current f (E's value) becomes authoritative. A's different-valued
  images stay authoritative=false, as an accompanying control. A later
  confirmation of A changes nothing on f, because no paired cell holds A's value
  with A's timestamp.

Rule:
1. **Candidate fields.** The keys of X's declared `args.changes`, plus
   `removed_fields`. Fields the plan explicitly corrects are excluded: they keep
   their selected authoritative stamps.
2. **Targets.** The current row and metadata of X's identity, and every other
   record's paired row-undo and metadata-undo images for that identity (before
   and after separately). A record supplies evidence for an identity only when
   it has exactly one matching row-undo entry and exactly one matching
   metadata-undo entry. Ambiguous means more than one matching entry in either
   list. If X lacks that unique pair, nothing on that identity is promoted from
   it: not the current row, and not any other record's images. If another record
   lacks that unique pair, only that record's images for that identity are
   skipped. An existing unique row-undo entry whose before or after is None
   explicitly records absence and remains eligible. No validation error or
   persisted shape changes.
3. **Promotion.** For each candidate field f and each target, set
   authoritative=true only when both of these hold:
   - the paired row cell exactly equals X's after-image cell. That means the same
     value, or both absent; an explicit Null is not absent.
   - the target's FieldStamp is present and has the same timestamp as X's
     after-image stamp for f. The authoritative flag is ignored in this
     comparison, so a match is not missed because a flag already differs.
   A missing stamp is never synthesized, not even as timestamp 0. A present stamp
   with timestamp 0 is valid: it is preserved and eligible for promotion.
4. **Delete.** A declared Delete promotes only a matching tombstone (same
   timestamp) where the paired row is absent.
5. **No other effect.** Values, presence, frozen bytes, args and `removed_fields`
   are never changed. No persisted shape changes.

Existing stores are not repaired: authority flags set by the old rule in
already-reconciled records stay as they are. There is no migration or demotion
without evidence. C1a is implemented before C4.

## C2. Row-causal dispatch

Definitions, all derived from existing persisted fields:
- `rows(X)` = the set of `(change.table, change.id)` over `X.undo`.
- A is **unresolved** iff its state is Queued, Sent or Acknowledged; or Applied
  or Corrected with `reconciled == false`; or Rejected with
  `rolled_back == false` (processed or not).
- Cancelled records never block. Disclose: an in-flight copy of a cancelled
  Sent record may still execute after later work.
- A **blocks** B iff `A.order < B.order`, `rows(A) ∩ rows(B)` is non-empty, A is
  unresolved, and not both A and B carry an entity descriptor
  (`record.entity.is_some()`). Entity-entity pairs are exempt because entity
  changes merge per field with stamps; this exemption is not a fix for O2.

Rule: `plan_push` never moves a Queued record B to Sent while any blocker of B
exists. The existing created-ID `dependencies` rule is unchanged and applies
first. A record already Sent is never blocked, so retries of pre-upgrade Sent
pairs continue (legacy residual, disclosed: their server order may differ from
local order).

Effect for pairs created under RD: B reaches the server only after A's final
outcome is received and A is explicitly reconciled (or rolled back, for a
rejection). Same-row server execution order then equals local order.

Liveness: same-row progression now requires the host to reconcile. After a
rejection, the later withheld command's own local write makes `rollback` of the
rejected record conflict, so release requires cancelling the later command first
(X2); there is no automatic path. The
reconcile callback stays optional, but without it a later same-row command waits
indefinitely and status shows why. The core never auto-reconciles, auto-discards
a correction or auto-rolls back.

Status: `QueueEntry` gains `awaiting_key: Option<String>`, the key of the
blocker with the smallest order. It is computed, not persisted. The bindings
status wire adds `awaiting_key` (string or null) beside `awaiting_local`. Every
in-repository SDK status decoder is updated in the same change; strict decoders
must not be left rejecting the new field. `blocked_on` keeps its local-ID
meaning.

Disclosed residuals: rows a command only reads are not ordered; server effects
on rows absent from local undo are not ordered; each same-row command costs one
round trip plus a reconcile before the next is sent.

Cost: O(records x undo entries) set checks per `prepare_push`, bounded by the
configured capacity. No extra store scan.

## C3. Explicit redelivery after a lost outcome

`request_redelivery(key, now_ms)` on `CommandDatabase`, bindings operation
`request_redelivery`:

1. Hold an idle permit. Validate `now_ms` with the same range as `prepare_push`
   (`0..=253_402_300_799_999`), else InvalidCommand. Unknown key is Protocol.
2. Acknowledged with no outcome: set state Sent and `due_ms = now_ms` (the raw
   sample, per SPEC-0035; not the identity floor). Attempts, frozen bytes, key,
   `header.last_ms` and all other fields are unchanged. Persist through the
   existing atomic owner plan.
3. Already Sent: Ok with no write.
4. Any other state: Protocol, with no write.
5. It never sends; the next `prepare_push` sends the identical frozen bytes and
   increments attempts as for any due retry.

Backend obligation (new normative API-0001 text): a repeated `idempotencyKey`
or entity `writeId` MUST NOT re-execute side effects. It MUST return the stored
final outcome if one exists, otherwise a receipt. Existing Sent retries already
depend on this.

## C4. Proven entity fallback at the closure proof (O2 slice)

### Position, precedence and errors
- C4 is a fallback at exactly one step: a closure-proof mismatch. That is where
  `simulate_inverse` or `simulate_metadata_inverse` over `closure(K)` finds a
  current image different from the optimistic one, which is surfaced today as
  `CommandError::RollbackConflict`. C4 never replaces any other RollbackConflict,
  and never any earlier error. Every earlier check runs unchanged, with its
  existing precedence and errors:
  - key and state (Protocol);
  - plan write shape (InvalidCommand);
  - SPEC-0039 guard structure (InvalidCommand);
  - SPEC-0039 data refusals for outside-closure writes, entity-managed tables and
    live undo (RollbackConflict).
- C4 never bypasses an outside-closure refusal.
- If any C4 condition below fails, the result is the original closure-proof
  mismatch's `CommandError::RollbackConflict`, with rows, metadata, ledger, status,
  pending outcomes and notifications unchanged. There is no new error kind.
- SPEC-0040 may consult an explicitly configured policy only after this fallback
  does not resolve the closure-proof mismatch. It never intercepts an earlier
  error. Its EntityRecord refusal preserves C4-ineligible entity conflicts and
  G1; it supplies no entity discard or forced rollback. Default Stop preserves
  the refusal behavior above. Manual discard is SPEC-0040's separate operation.
- C1 is unchanged. An empty plan for an Applied or Corrected record keeps its
  existing success path without any proof and never reaches C4. A typed Update
  with no changes that reaches the fallback may refuse; its normal-path behavior
  is unchanged.

### Shape
- **Correction path:** `reconcile` (or `reconcile_guarded`) of A, where A is an
  existing-row single entity Update record:
  - its entity descriptor op is Update and `created` is empty;
  - it has exactly one row undo change and one metadata undo change, both on row W;
  - `closure(A) == {A}`;
  - the plan has no mappings, no Put or Remove, and only typed Update
    corrections, each on W's table and record id;
  - W exists before and after, so there is no create, delete or other identity.
  A SPEC-0039 guard on W is unused and already returns InvalidCommand.
- **Rollback path:** `rollback(P)` only, never `cancel`, where P is a processed
  rejection (`Rejected`, reconciled, not rolled back) with the same record shape.
- **Owned fields O:** the fields K declared, as changes or removals. On the
  correction path O also includes every field the typed correction touches.
- **L:** the live records after K that touch W, in order. L may be empty.

### Eligibility
Live records refuse unconditionally:
- **E1.** Every live record outside K that touches W is an entity record. A live
  named command on W refuses.
- **E2.** No live record outside K with an order below K's touches W.
- **E3.** Every Li is Queued, Sent or Acknowledged with no outcome. An Applied,
  Corrected or Rejected unreconciled record, or a live Cancelled one, refuses.
- **E4.** No create, delete or tombstone for W in K or L or in W's current
  metadata, and no merge hook on W's table.

Exact images:
- **E5.** When L is non-empty, `Li.after == L(i+1).before` over whole rows and whole
  metadata, authoritative flags included, and current W equals `L_last.after`.
- **E6.** Baseline B is `L1.before` (whole row and metadata), or current W when L is
  empty. For every f in O, B(f) equals `K.after(f)` in value and FieldStamp.

Records that are no longer live (finalized entity writes, reconciled named
commands, trimmed records) are not inspected individually. They refuse only through
E6, when they changed an owned field. A change confined to fields outside O is
preserved exactly (EO16). No baseline is ever inferred from a stamp's authoritative
flag, because a stamp does not bind a value (EO10b).

### Order of operations (normative)
1. All ordinary validation, with its existing precedence (see above).
2. The closure-proof mismatch.
3. E1-E6.
4. Build eff0. The backend's typed correction selections and the C1a
   value-aware promotion of A's uncorrected declared fields are applied
   only into eff0, never to the current row or to L's images.
5. Recompute L last (rule G2).
6. Commit one SyncPlan.

On the C4 path, the ordinary `apply_entity_correction` and `promote_metadata`
for K and W do not run a second time. Nothing re-promotes or re-corrects the
recomputed row, its metadata or L's images. API-0009's "Then apply only the
supplied fields…" refers, on this path, only to building eff0.

### Starting image eff0
eff0 is the whole B row and metadata, with only owned fields changed:
- f not in O: `eff0(f) := B(f)`, value and stamp. K's whole before or after rows
  are never copied.
- Correction path, f corrected: the corrected value and exact backend stamp,
  authoritative, as in API-0009.
- Correction path, f declared by A but not corrected: B(f), promoted by the C1a
  value-aware rule (value or presence and timestamp must match A.after).
- Rollback path, f in O: `P.before(f)`, value and stamp.

### Pending recompute (rule G2: strict pending-stamp wins)
A field value is one of present, explicit Null, or absent, and is copied exactly,
together with its stamp or the lack of one (API-0009 lossless rows). Absence and
Null are never conflated.
For each Li in order, and for every field f of W (row and metadata), separately:
1. `Li.before(f) := eff(i-1)(f)`.
2. If Li declared f (a value or a removal), Li's own client stamp must be strictly
   newer than the timestamp of `eff(i-1)(f)`. For this comparison only, a field
   without a stamp counts as time 0, as in `resolve_entity`. A missing stamp is
   never synthesized, not even as 0. A present timestamp 0 is valid and
   preserved. Then `eff(i)(f)` is Li's declared value with that
   stamp, unauthoritative. An equal or older timestamp refuses the whole action.
   If Li did not declare f, `eff(i)(f) := eff(i-1)(f)`.
3. `Li.after(f) := eff(i)(f)`, for every field, whichever branch applied.

Current W becomes `eff(last)`, or eff0 when L is empty. Whole images are recomputed
for every field, so no rejected or uncorrected cell survives in a later image. That
is why C4 is not the rejected v2 R2.2/R3 design, which rebased only a before-image.
A pending local intent is never hidden by a backend value.

### Effects and exactness
- One atomic SyncPlan. It carries:
  - W's row change through the existing derived guard;
  - the recomputed L undo and metadata undo images;
  - current metadata;
  - K's transition: reconciled on the correction path, rolled_back = true on the
    rollback path.
- Writes touch only W. Frozen envelopes, args, declared changes, keys and attempts
  of every Li are byte-identical.

### Exits and gap G1
- When the correction path refuses, the existing exit is C1: an empty-plan
  `reconcile` for an Applied or Corrected A, which knowingly drops the correction.
  C1 does not apply to Rejected.
- This slice promises no exit for any C4-ineligible rollback. That includes, among
  others, a finalized write to a P-owned field (G1, pinned by G1-OPEN), a chain break,
  and a permanently live recovery record. In those cases P remains live with
  rolled_back false. Existing operations still apply where they already did; for
  example, a live later record can be cancelled or resolved first (X1 and X2).
- There is no DEF-0032 closure claim, and no new exit, discard or abandonment
  semantics. A separate terminal-Rejected confirmation rebase is under review and
  is not part of this cut.
- **Pull protection:** a pull returns PullConflict for any identity in live undo
  (`pull.rs` `protected`), so an accepted pull cannot change W while K or L is live.
  E5 and E6 are retained as defense in depth; EO14 and EO15 pin this.

### Traces
- **EO2.** A sets f=A@10, is Corrected to S@15, and pending E sets f=E@20.
  - B = E.before = A.after holds (E6). eff0(f) = S@15, authoritative. E@20 > 15,
    so E keeps E@20.
  - Result: E.before(f) = S@15, E.after unchanged, current f = E@20.
  - Rolling E back later restores S@15. With S@25 instead, the action refuses (EO3).
- **EO16** (L empty).
  - Correction path: A sets f=A@10 and is Corrected to S@15. A later entity write E,
    changing only g, has been Applied and confirmed, so it is no longer live.
    B = current W; B(f) = A.after(f), so E6 holds. Result: f=S@15, and g keeps E's
    value and stamp exactly.
  - Rollback path: P sets f, is rejected and processed, and finalized E changed only
    g. Result: f returns to P.before(f), g keeps E's value and stamp, and
    rolled_back = true.
  - Both are refused today, because the whole-row proof fails on g.
- **EO14.** An accepted change of an untouched field g is preserved in eff0, in every
  L image and in the current row. A public pull cannot make that change while A or
  L is live (it returns PullConflict); a pure planner case asserts the preservation.
- **EO-ORDER.** A sets f=A@20 and is Corrected to S@15; pending E sets f=E@20.
  - E6 holds: B(f) = E.before(f) = A.after(f).
  - eff0(f) = S@15, authoritative. E@20 > 15, so E wins.
  - Result: E.before(f) = S@15 authoritative, while E.after(f) and the current f stay
    E@20 authoritative=false. No second promotion can mark them, because their
    shared clock 20 equals A's.
  - After a restart, a later rejection and rollback of E restores S@15
    authoritative.
- **EO15.** A correction field g that A did not declare is owned on the correction
  path. If a finalized entity write E changed g after A (public path), or an
  external change reached B (pure planner path, because pull protection blocks the
  public interleaving), then B(g) differs from A.after(g) and the action refuses
  (E6), with the exact state preserved.
- **EO17.** Three states stay distinct through the recompute and a later rollback:
  1. A removed optional field is absent, not Null, and its stamp is retained in
     metadata.
  2. An explicit Null is present as Null.
  3. A missing FieldStamp stays missing, and is never synthesized, not even as 0.
     Time 0 is used only for comparisons. A present stamp with timestamp 0 is
     valid and preserved.

  The recompute copies value state and stamp (or its absence) exactly into
  `Li.before` and `Li.after`. A later rollback of Li restores `Li.before` exactly.
  Frozen `changes` and `removed_fields` of every Li are unchanged. This tests
  existing declared semantics only.
- **ET3.** P is rejected and processed, and pending E later patches the same field
  with a newer stamp.
  - eff0(f) = P.before(f), and E keeps its value.
  - Result: E.before(f) = P.before(f), rolled_back = true, current value = E's.

## SDK translation and typed status

The same component updates every in-repository typed status surface:
JavaScript `QueueEntry.awaiting_key: string|null`, Swift
`QueueEntry.awaitingKey: String?`, and Kotlin `QueueEntry.awaitingKey: String?`.
Swift and Kotlin decode the exact eleven-field queue entry, accepting a string
or null for `awaiting_key` and rejecting malformed or unknown fields. Fetch and
status observation use the same decoder. The JavaScript host preserves the
owned status wire value and exposes the new field in its TypeScript declarations.

SDK helpers delegate one explicit rearm call with an explicit clock sample:
JavaScript `requestRedelivery(key, now)`, Swift
`requestRedelivery(_ key: String, nowMilliseconds: Int64) async throws`, and
Kotlin `suspend fun Database.requestRedelivery(key: String, nowMilliseconds: Long)`.
They encode `{key,now_ms}` losslessly and return the core's result or error;
they do not send an envelope themselves, automatically retry the rearm call or
fabricate an outcome. After a successful rearm they wake the existing managed
sync driver, if started, just as receipt processing does. That driver sends the
original frozen bytes through its ordinary transport when due. Without a started
driver, no transport call occurs; the existing manual prepare-push path remains.
Add `request_redelivery` to Swift and Kotlin's observed-mutation classifications;
in JavaScript the sync-mutation helper wakes on this operation as on receive_wire.
This also schedules existing observation delivery for the changed queue status.

SDK tests use scripted channels for translation; the corrected Rust framework
and WASM are rebuilt into task-owned fixtures for actual integration checks.
Old packaged binaries still emitting ten-field entries are not evidence for the
new decoder. Same-row fixtures that formerly sent two commands in one batch
are explicitly superseded by the RD sequence: send A, receive and reconcile A,
then send B. They retain both outcomes, durable recovery and final-row assertions.
Kotlin/Android execution remains held; source review cannot be labelled a passing
compiler or runtime check. Adding a public signature or compilation seam alone
does not count as a behavioral RED.

## Explicit resolution paths using existing operations

- Correction with later unsent same-row work (X1): `reconcile(A, correction)`
  returns RollbackConflict. Under RD the later command E is still Queued, so the
  app may cancel E (exact guarded inverse), apply A's correction, then issue a
  new command for E's intent. Alternatively it reconciles A with an empty plan
  and repairs locally with an explicit write.
- Rejection with later unsent same-row work (X2): `rollback(P)` returns
  RollbackConflict, P stays live with `rolled_back == false` and keeps blocking.
  Cancel E, then `rollback(P)` succeeds.

Neither path reports success while rejected or uncorrected cells remain.

## Open finding O1: out-of-closure correction writes

`reconcile` applies Put/Remove writes to rows outside `rows(closure)`
unconditionally (`reconcile.rs` write loop; only `validate_public_row` runs).
Such a write can overwrite an unrecorded local edit, which may contradict
REQ-0043. This cut keeps the behavior, pins it by a characterization control,
and links DEF-0038 for a separate guarded contract and regression correction. It is
neither relaxed nor tightened here.

## Proposed separate API (not in this cut): abandon a rejected inverse

Recorded only so the need is not lost; it requires its own review before any
test or code.
- Explicit opt-in call on a processed rejection with `rolled_back == false`, no
  dependents, and no entity descriptor (entity-bound records return Protocol).
- Clears `undo` and `metadata_undo`. Rows stay as they are. `rolled_back` stays
  false. Status must distinguish "inverse abandoned" from "rolled back".
- Documented as a deliberate local divergence the application must repair, not
  convergence and not a recovery success.

## Rejected designs

- Rebasing a later record's before-image (v2 R2.2, R3): retains rejected or
  uncorrected cells in the visible row and can report `rolled_back` falsely.
  C4 differs: it recomputes whole before AND after images under E1-E6 and G2, and
  refuses rather than predicting a backend LWW loss.
- Inferring a finalized later baseline from an authoritative FieldStamp: a
  stamp does not bind a value (EO10b).
- Silent supersede of a correction.
- Reusing `rolled_back` for anything but a completed inverse.

## Integrity and compatibility

- No persisted field, format or decoder change. Old binaries read every ledger
  this produces; they lack RD and C1, so a downgrade restores the stuck
  behavior, not corruption.
- Behavior changes to disclose in the changelog: RD ordering and its reconcile
  requirement; C1 empty-plan confirmation succeeding under later edits; the
  Acknowledged-to-Sent transition through an explicit call; the additive status
  field; C1a value-aware promotion, under which equal-clock different pending
  values are no longer marked authoritative; C4 entity corrections and rollbacks
  now succeeding where they used to
  conflict: TC-0042 ET1 different-field, ET3, EO2, EO4, EO5 and EO16.
- C4 rewrites only W, the current metadata, and the undo and metadata-undo images
  of pending later records, with existing fields and shapes.
- C1a changes which stamps become authoritative at confirmation. Authority flags
  already set by the old stamp-only rule are not repaired automatically: no
  migration or demotion without evidence.
- Downgrade: committed repaired images persist, and old binaries decode them. Old
  inverse and promotion code interprets them consistently, because they are the
  exact images of the chain C4 proved. An old binary refuses future C4-only
  actions, but it cannot undo a committed C4 transition. Only actions it does not
  support return to the old refusal.
- Existing tests that expect two same-row Queued commands in one batch are
  relabelled explicitly as RD characterization changes, never edited silently.
- Storage transition checks (`storage/sync.rs:70-84`) restrict identity counters
  and frozen envelopes only; Acknowledged-to-Sent with unchanged frozen bytes is
  expected to pass. Confirm in RED.

## Acceptance and TDD

TC-0042 supplies the cases. Each starts with a source-bound behavioral RED that
fails on unchanged code for the stated reason, not a missing symbol, fixture or
import. Characterization cases (marked) must pass before and after. Capture
implementation, test and toolchain hashes before and after RED and GREEN. Shared
public-owner cases run on memory and SQLite; unit cases exercise the pure
planners. After GREEN, run the surrounding command, sync, entity and bindings
suites, then the full workspace at integration. Do not touch the SPEC-0035 clock
tests.

## Quality attributes

- Functional Suitability: repeated edits confirm; lost outcomes have an explicit
  recovery; same-row server order matches local order for new pairs.
- Performance Efficiency: bounded ledger traversal per push; no store scan; no
  time target.
- Compatibility: no format change; additive status field; legacy Sent pairs keep
  retrying.
- Interaction Capability: `awaiting_key` explains every RD wait; conflicts stay
  explicit; no false success.
- Reliability: every transition is one atomic SyncPlan and restart-stable.
- Security: no new input source; redelivery relies on the stated backend
  deduplication; bounds and redaction unchanged.
- Maintainability: blocker and C1 rules have direct unit cases.
- Flexibility: existing reconcile callback and transport seam unchanged.
- Safety: no physical control; no user data is touched by this work.

## Rollback

Revert task-owned source, tests and contract text together. No format rollback
or migration is needed. Keep all RED, GREEN and review evidence. This spec
changes no security approval, hold or gate.

## Appendix A: candidate contract text (root applies; not edited here)

API-0006 "Dispatch and receipt", append after the selection sentence:

> A queued record is also withheld while an earlier record that wrote any of the
> same rows is unresolved: queued, sent or acknowledged; applied or corrected and
> not yet reconciled; or rejected and not rolled back. Pairs where both records
> are entity writes are exempt. Already sent records are never withheld.
> QueueEntry.awaiting_key names the earliest withholding record. Same-row
> progression therefore requires the application to reconcile.

API-0006 "Authoritative local apply", replace "Before changing anything, compare
the command's net optimistic after-images to current rows; independent later
edits cause RollbackConflict." with:

> An applied or corrected plan with no writes commits mappings, renames,
> reference rewrites, dependent undo updates and queued normalization without an
> inverse proof, because it changes no cell. A plan with writes first compares
> the command closure's net optimistic after-images to current rows; independent
> later edits cause RollbackConflict. The core never drops a correction itself.

API-0006, new paragraph after receive:

> request_redelivery(key, now_ms) holds an idle permit and moves an acknowledged
> record without outcome back to sent, due at the validated raw sample, with
> unchanged frozen bytes, attempts and key. Sent is a no-op; other states are
> Protocol. It never transmits.

API-0007: an acknowledged record may return to sent only through
request_redelivery; frozen bytes stay identical.

API-0001: a repeated idempotencyKey or writeId must not re-execute side effects
and must return the stored final outcome if one exists, otherwise a receipt.

API-0009 section 2 promotion sentence (proposed in this review, not applied):
replace "only still-matching optimistic stamps of the accepted write are promoted"
with the C1a value-aware rule. State that on the C4 path, correction and
promotion apply only into eff0, before the pending recompute.

API-0009 section 2 (proposed in this review, not applied): after "Before any
correction, prove the optimistic row and comparison-metadata closure against
current state, preserving independent edits on conflict." add the SPEC-0038 C4
fallback sentence: it covers an existing-row single entity Update whose plan holds
only typed Update corrections of that row. An ineligible fallback returns the
original RollbackConflict with nothing changed, and SPEC-0039 checks and refusals
keep their precedence. After "guarded inverse simulates both in reverse order
before committing either." add the same fallback for rollback (never cancel) of a
processed rejected single entity Update.
