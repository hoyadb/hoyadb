---
id: DEF-0059
title: Memory rollback retains newly created empty index shells
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-31
updated: 2026-10-31
parents: [SPEC-0047]
affects: [SPEC-0037, API-0007]
regression_for: [TC-0041, TC-0051]
tags: [storage, atomicity, rollback]
---

# DEF-0059: Memory rollback retains empty index shells

## Symptom and source evidence

The genuine-proof R9 memory fault test inserts two application rows and panics
at the second `map-written` hook. Raw tables restore exactly, but the index map
retains a new table entry with empty column maps where no entry existed before.
The behavioral failure is recorded in
`/tmp/hoyadb-followup-20261031/memory-delta-initial.log`. The test compares the
private index structure as well as rows; it does not normalize away the change.

`storage/snapshot/undo.rs` records before/after row images but no prior index
shape. Its rollback calls `remove_entries`, whose empty table/column maps can
survive. SPEC-0037 R7 requires restoration of touched rows and indexes after
unwind, including partial primitive writes.

## Acceptance

Restore the exact prior index shape without copying or scanning unrelated rows
or indexes. Cover application, model and packet writes, partial index changes
and the pre-commit fault using genuine owner-created deltas. Confirm reuse and
strict reopen on restored storage; retain all existing UndoGuard and work-bound
checks. No caller-visible recovery API or production fault seam is added.

## Correction and verification

`Undo::Row` now captures whether the touched table index was absent and which
of the touched row's field maps were absent before the primitive. After restoring
row/value entries, rollback removes only those newly introduced maps. Existing
empty maps survive exactly. This captures field names before the write; it does
not scan or clone unrelated index values or rows.

Four genuine-owner-proof fault matrices cover first registration, existing
replace/remove/insert, prior empty field maps and a partially empty index shape.
They compare private raw tables and exact indexes at every reached write/index/
pre-commit fault, strictly reopen the restored store and obtain a fresh proof for
retry. The owner is closed before the injected unwind; owner reuse across a panic
is not claimed.

The retained coherent packet is
`/tmp/hoyadb-followup-20261031/memory-delta-review/packet.json`:

- Initial behavioral RED: two controls passed and the new-table-shell case failed.
  Final tests against the old undo implementation reproduce two index-shape
  failures with two controls passing on a fresh target directory.
- GREEN: 179 core unit tests, 15 memory adapter/index/sync cases, and seven
  snapshot tests on a separate positive target passed. The 178-input source
  inventory matched after execution.
- A disposable, delta-only `ManuallyDrop` mutant bypassing UndoGuard rollback
  caused all four new matrix tests to fail on raw-row equality. No mutant is
  retained in production source.
- Scoped formatting and diff whitespace checks passed. Final integrated planner,
  workspace Clippy and platform validation remain separate; the pinned checkpoint
  still contains in-progress core dead-code warnings.

Independent Claude Fable 5.1 review completed with **no defects** in the frozen
fix. The reviewer verified all 178 source inputs, exact prior index-shape
restoration and multi-write ordering, and confirmed the genuine-proof tests and
mutation evidence. The report is
`/tmp/hoyadb-followup-20261031/memory-delta-review/fable-review.md`. The three
owned source files still matched the reviewed hashes when this record was
updated. The draft status is unchanged.
