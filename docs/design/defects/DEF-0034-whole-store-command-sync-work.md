---
id: DEF-0034
title: Command admission and sync commits repeatedly process the whole local store
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-31
parents: [SPEC-0029]
affects: [API-0006, API-0007, API-0009, NFR-0001, NFR-0004]
regression_for: [TC-0032, TC-0034, TC-0041]
tags: [purpose-audit, remediation]
---

# DEF-0034: Command admission and sync commits repeatedly process the whole local store

## Symptom and evidence

StagedScope::begin scans every application table and copies all rows into
SnapshotStorage plus an inventory. Persist serializes the whole recovery ledger. SQLite
apply_sync_plan deletes and reinserts all sync rows; entity pull repeatedly snapshots
the data and rewrites growing comparison metadata. These are confirmed structural costs,
not a fresh latency benchmark. Fixed-size bootstrap pages accumulate work proportional
to the growing full store.

## Confirmed cause and contract boundary

The admission snapshot and complete-row-set compare-and-swap contracts require global
work even for a one-row operation. Contiguous metadata packet ordinals amplify later-row
rewrites. The memory adapter also deliberately copies candidates; that baseline must be
distinguished from avoidable synchronization work.

## Required correction

Review a coherent storage/command contract that bounds incidental admission and
persistence work by the accessed or changed data, while preserving atomicity,
read-your-writes, isolation, strict recovery validation and restart integrity. Use
deterministic scan/read/serialized-row/mutation counters as fitness functions. Do not
invent a latency target, use contended benchmark timings as acceptance, or merely remove
validation. Identify ordered/limited reactive-query costs and address concrete avoidable
full-match work within the accepted dataset scope.

## Regression gap and prep cases

SC01 a narrow command on stores of different unrelated sizes; SC02 one entity patch with
growing unrelated metadata; SC03 fixed-page bootstrap work growth; SC04 corrupted/stale
recovery, atomic rollback and restart, including relocated open/commit catalog checks
and the detection point of an untouched packet edit; SC05 ordered limited queries and
reactive affected-key counts. Existing semantic conformance does not establish these
cost bounds. Interface details and cases require review before implementation.

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

## Completion

Open. The operator requested fixing the audit findings and repeating Claude Opus 5.5
correctness and purpose reviews until three consecutive issue-free rounds. The
clean-review count starts at zero and resets after a new finding. This does not waive
the independent Phase 7 security, advisory or execution gates.

## 2026-10-31 implementation checkpoint

The later SPEC-0047 task implements the accepted SPEC-0037 R9–R10 contracts:
first-image ledger transactions, touched-packet adapter commits, cached progress
and session epochs, bounded pull/merge overlays, and format-3 metadata identities
for newly configured entity ledgers. Existing format-2 stores retain their exact
encoding and required ordinal suffix work; unknown adapters retain the full
validation and compare-and-swap fallback. TC-0041 records the deterministic
counter, refusal, rollback and format-retention cases.

Claude Fable 5.1 reviewed the final core, adapter and format-3 packets without
remaining actionable defects. The latest operator instructions use Fable for
cross-review and Claude Opus 5.5 for record-only handoff; no new Opus review round
is claimed here. The combined workspace passes 882 tests; final Node/WASM,
scoped Chrome OPFS, macOS Swift and Android SDK/consumer verification passes
against the recorded sources. The exact two-attribute production-build
supplement and build-only platform limits remain explicit in the report.
The R11 offline converter remains gated on its unresolved wrong-key/WAL resource
cost and classification decision. See the
[follow-up report](../../implementation/approved-engineering-completion.md)
for exact source-bound results and remaining limits. The earlier intake and
review history above are preserved.
