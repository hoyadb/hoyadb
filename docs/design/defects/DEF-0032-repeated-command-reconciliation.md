---
id: DEF-0032
title: Repeated command edits can strand successful authoritative outcomes
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-27
parents: [SPEC-0029]
affects: [REQ-0010, REQ-0017, REQ-0043, API-0006]
regression_for: [TC-0032, TC-0042]
tags: [purpose-audit, remediation]
---

# DEF-0032: Repeated command edits can strand successful authoritative outcomes

## Historical symptom and evidence (audit base 7058b29)

Two named commands A and B replace the same existing authoritative row while offline.
Both receive Applied outcomes. Reconcile(A, empty) returns RollbackConflict,
reconcile(B, empty) succeeds, and reconciling A again still returns RollbackConflict.
Cancel, rollback and forget each reject A with Protocol; A remains live. Root reproduced
this through the public memory owner at 7058b29 against the current formatting overlay.
The source-bound executable and output are under
target/purpose-audit-20261026/repeated-edit/ and repeated-edit.txt.

## Historical cause and contract boundary

The command-origin reconciliation path proves the undo inverse against current rows
before applying any plan. Its closure follows created-local-ID dependencies, not later
writes to existing rows. A later independent edit therefore makes A.after differ from
the current row even when the server merely confirms A and no local correction is
needed. API-0006 currently requires the independent-overlap conflict, so this is a
contract-level purpose defect.

## Required correction

Define reviewed reconciliation behavior for ordinary repeated edits, preserving later
committed work and delivery of both outcomes. Cover no-op confirmations, actual
corrections, rejections, created-ID mappings and out-of-order outcomes. Keep guarded
rollback and atomic ledger/data publication. A destructive reset or application
reconstruction of unexposed after-images is not the fix. Record the distinction between
a safe no-op confirmation and corrections that require an explicit rebase or resolution.

## Original regression gap and prep cases

RC01 two Applied commands on an existing row in both receipt/reconcile orders; RC02
independent direct edit after a command; RC03 genuine correction/rejection with later
work preserved or an explicit non-destructive resolution path; RC04 mapped
producers/dependents, restart and pull protection; RC05 unchanged rollback-conflict
controls. Existing c11 overlap checks cover rejection, not successful repeated-edit
recovery.

The concrete correction spec and declarative cases must be independently reviewed before
permanent regression tests or implementation. This intake is not a fix or a passing
result. Each implementation starts with a source-bound behavioral RED, then the smallest
justified GREEN and existing surrounding controls.

## Compatibility and rollback

No existing user store is migrated, reset, deleted or opened during remediation.
Preserve byte budgets, redaction, owned lifecycle and restricted-build controls. Revert
task-owned source and documentation changes together if the correction fails review,
preserving all diagnostic and RED/GREEN evidence. Any on-disk format or public semantic
change needs its explicit reviewed compatibility policy.

## Component status (2026-10-27)

The [SPEC-0038 C4 slice](../spec/SPEC-0038-safe-command-reconciliation-recovery.md)
is implemented in commit `8efb1af`, after the C1a value-aware promotion component
`1b4624e`. C4 handles a proven single-row entity Update correction or processed
rejection rollback across eligible pending later entity writes. It is consulted
only at the closure-proof mismatch. Earlier validation and SPEC-0039 protections,
C1 empty confirmations and cancel behavior remain unchanged.

The [TC-0042 cases](../tests/cases/TC-0042-command-reconciliation-recovery.md)
cover the C4 slice with 27 assertion REDs on unchanged production, whole row and
metadata images, frozen-record preservation, refusal and precedence controls,
fresh SQLite connections before later rollback, and mutation checks. The
counter-exactness follow-up has four separate assertion REDs; its synthetic
unpaired fixture is writer-unreachable but validator-accepted, asserted in the
fixture itself. No new validation rule or stored shape was introduced.

Verification is tied to two cuts:

- Full workspace predecessor tree `29b9f54a`: 689 passed, 0 failed, 0 ignored
  under `--no-fail-fast`, including both adapters' capacity-500 checks and the
  encrypted-storage group. Sources were unchanged.
- Committed tree `85944eba`: 276 focused tests passed, with formatting, clippy
  in three feature configurations and the core/memory WASM build clean, and
  sources unchanged. Its only production difference from the full-run cut is
  exact placement of the metadata visit counter; the remaining delta is core
  test code. This is not a claim of a full workspace run on `85944eba`.

The source-bound records are in
`target/purpose-remediation/c4-root-integration/checks.json` and
`target/purpose-remediation/c4-root-integration/final-s2/`, including the delta,
exact index, source identity and preserved-overlay proofs.

G1 remains open. C4 promises no exit for an ineligible rollback, including a
finalized write to an owned field, a chain break or permanently live recovery
work. It adds no discard or abandonment semantics. The SPEC-0040 compatibility
decision and final combined remediation run remain pending. Reverting C4 restores
the earlier refusal rule for future attempts; committed repaired images remain
in the existing format and are not undone by a code revert.

## Completion

Open. The operator requested fixing the audit findings and repeating Claude Opus 5.5
correctness and purpose reviews until three consecutive issue-free rounds. The
clean-review count starts at zero and resets after a new finding. This does not waive
the independent platform SDK security, advisory or execution gates.
