---
id: DEF-0009
title: Batch reply preflight accumulates unrelated same-table before images
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, TC-0036]
affects: [REQ-0046, API-0011]
regression_for: [TC-0036]
tags: [phase-seven, review, reply-budget]
---

# DEF-0009: Batch reply preflight accumulates unrelated same-table before images

## Symptom

Claude's read-only checkpoint on 02621eb demonstrated the production sizing
shape rejects 128 keyed replacements with three String fields, although the
actual reply is approximately 45 KB. Its standalone probe used the real core
canonical limiter. A subsequent public-facade ten-row case reproduced the same
LimitExceeded error with a valid approximately 600 KB report. No write committed.

## Root cause

`budget::batch` includes every preceding same-table after image as a possible
before image, even when both IDs are known and different. Sized node/byte counts
grow quadratically while the actual keyed reply grows linearly.

## Intended correction and acceptance

Track the latest known staged image per table and ID, including removals. For
core-allocated insert IDs that are not known during preflight, conservatively
retain only potentially matching unknown images. Do not treat unrelated known
IDs as possible before images. Preserve before-image byte/node/depth bounds;
no post-commit reply failure or counter/queue/event change on rejected batches.

TC-0036 G11 must cover 128 distinct keyed replacements with exact before/after
rows, repeated same-ID transitions, an unknown allocated insertion followed by
a keyed operation, and the existing real oversized-before-image rejection.
An additional ten-row case uses 30 KB String cells: its complete approximately
600 KB before/after reply fits, but unrelated prior images must not reject it.
The shared budget path serves ordinary, scoped and entity batches.

## Fix and regression status

Implemented in `crates/hoyadb-bindings/src/budget.rs`: latest known images are
keyed by table/ID, including removal; unknown allocated images remain separate.
The ten-row public regression failed before this fix, then passed. All 128-row
ordinary/scoped/entity and repeated-ID controls pass. The initial old 128-row
run was stopped while CPU-bound; it is not claimed as an assertion failure.
Full evidence and setup qualifications are in
[the resolution report](../../implementation/phase-seven-reply-budget-resolution.json).
Rollback is a code revert; no stored format or user-store migration changes.
