---
id: DEF-0067
title: Recovery exits lost after conflicted cancel, cancelled dependents and discarded undo
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-10
updated: 2026-11-10
parents: [SPEC-0040]
affects: [SPEC-0038, SPEC-0039, API-0006]
regression_for: [TC-0044, TC-0042, TC-0043]
tags: [conflicts, recovery, review, regression]
---

# DEF-0067: Recovery exits lost after conflicted cancel, cancelled dependents and discarded undo

## Symptom

Three reconciliation paths stranded records. (1) `cancel` of a processed Rejected record whose inverse conflicted committed `Cancelled`, after which rollback fallback, configured Discard, manual `discard` and `forget` all refused it. (2) A no-write confirmation failed with an opaque storage error when a dependent had been cancelled with a conflict, because `settle` renormalized only Queued records. (3) The outside-closure refusal considered undo retained by non-live Discarded records, although SPEC-0039 limits it to live records.

## Acceptance

A conflicted cancel keeps a manual discard exit; a no-write confirmation renormalizes a never-sent Cancelled dependent; a guarded write outside the closure is refused only for a live record's undo.

## Root cause and fix

`discard::eligible` accepts a Cancelled, not-rolled-back record that still stores its Rejected outcome; a sent Cancelled record with no stored outcome is still refused. This narrows the earlier SPEC-0040/API-0006/TC-0044 CP06b wording that every Cancelled record with frozen bytes is refused, and those texts were reworded accordingly; the change awaits maintainer confirmation. `settle` renormalizes a never-sent Cancelled record with reference paths, and `guard_outside_closure` requires liveness.

Regression cases, each observed failing first: `c15_conflicted_cancel_of_a_processed_rejection_keeps_the_discard_exit`, `c1_no_write_confirmation_renormalizes_a_conflict_cancelled_dependent`, `gw_outside_closure_refusal_requires_a_live_record`.
