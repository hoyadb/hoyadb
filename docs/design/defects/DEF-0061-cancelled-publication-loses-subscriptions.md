---
id: DEF-0061
title: Cancelled reactive publication silently removes subscriptions
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-31
updated: 2026-10-31
parents: [SPEC-0047]
affects: [API-0005, SPEC-0037]
regression_for: [TC-0041, TC-0051]
tags: [reactivity, cancellation, recovery]
---

# DEF-0061: Cancelled publication loses subscriptions

## Symptom and source evidence

Claude Fable 5.1's review of the SI-Y01 cancellation packet identified that
`model/reactive.rs::publish` takes the owner's groups into a local vector,
awaits projection/materialization, and restores the vector only on completion.
Dropping that future while a read is Pending drops every registration. The
committed rows and command ledger remain intact, but subsequent writes cannot
notify the lost tokens. Two behavioral regressions reproduced zero retained
groups instead of three after a real delta commit and an actually Pending
observation read. The original review is retained at
`/tmp/hoyadb-followup-20261031/fable-ledger-cancellation-review.md`.

API-0005's existing lack of a general mutation-cancellation guarantee does not
establish this behavior as desirable. This correction is narrowly scoped to
preserving registrations and recoverable observation state when publication is
cancelled after storage commit; it does not promise rollback of a committed write.

## Acceptance

Use a genuinely Pending adapter and cancel after storage commit during
publication. Preserve all subscription tokens and group ownership, invalidate
incomplete candidate/delivery state, and recover exact current rows/counts via
refresh or a later committed write. Cover a group before and after the suspended
group, repeated cancellation and unsubscribe. Retain existing error isolation,
count deadlines, bounded candidate work and cancellation-before-persist checks.
No public API, runtime dependency or production fault hook is added.

## Correction and verification

A private publication guard retains ownership of the detached groups and
restores them when the future completes or is dropped. Its progress index
advances only after each group finishes. On cancellation, the current and
unvisited groups become dirty, discard incomplete pending counts, and invalidate
count equality without resetting the last delivery time. Already completed
groups and previously queued snapshots remain intact. No whole-cache clone,
unsafe code, runtime dependency or public API is added.

The regression file
[`reactive_cancellation_cases.rs`](../../../crates/hoyadb-storage-memory/tests/reactive_cancellation_cases.rs)
uses the existing yielding adapter and a genuine owner-produced delta. It
cancels at the second group's projection and full-row materialization, after
the first group completes and before the third group starts. It checks exact
refresh/later-write recovery, repeated refresh cancellation, unsubscribe,
same-cardinality count recovery, the original 16 ms deadline and preservation
of older queued initial events. A separate regression rejected the first
draft's overly broad removal of those older events (one retained event instead
of three); the final guard leaves the queue intact.

Source-bound evidence is retained under
`/tmp/hoyadb-followup-20261031/reactive-cancellation/`: `red.log` records the two
registration-loss failures; `queue-red.log` records the prior-event regression;
`final-source.json` pins the 180-input immutable `final-work` snapshot. Final
observed results: three new cancellation cases, two existing ledger cancellation
cases, one reactive smoke case, sixteen shared reactive conformance cases, two
bounded/ordered observer unit cases and two real SQLite post-commit decoder
fault cases passed (26 total). Scoped Clippy with `-D warnings`, source formatting
and traceability also passed. The shared yielding helper and existing ledger
cancellation tests were not changed.

API-0005 now states only post-commit observation registration recovery; it still
does not promise general mutation cancellation safety or rollback of a committed
write. Claude Fable 5.1 independently reviewed the frozen correction and its
source-bound evidence and found no defect; the review is retained at
`/tmp/hoyadb-followup-20261031/reactive-cancellation/review/fable-review.md`.
The reviewer did not rerun tests. Two informational observations remain:
cancelling a refresh also dirties unvisited groups that were previously clean,
and work counters include attempted candidate work before cancellation.
Neither changes correctness or ordinary completed publication behavior.
Full integrated workspace/platform verification remains a separate gate.
