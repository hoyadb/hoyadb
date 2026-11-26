---
id: DEF-0035
title: A queued or hung command in one owner blocks unrelated database writes
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [SPEC-0029]
affects: [REQ-0011, API-0006, API-0011, NFR-0002]
regression_for: [TC-0032, TC-0036]
tags: [purpose-audit, remediation]
---

# DEF-0035: A queued or hung command in one owner blocks unrelated database writes

## Symptom and evidence

With independent memory owners A and B, merely creating an unbegun request on A makes
B.insert return Busy. An active scope on A does the same. Root reproduced both through
the public API, then verified B writes succeed after A releases its request. The
source-bound availability probe is under target/purpose-audit-20261026/. A handler that
never resolves, or a leaked queued request, has no core timeout.

## Confirmed cause and contract boundary

One process-wide coordinator rejects idle permits while any request is queued or active;
JS additionally serializes every database through one module-level jobs array. Global
serialization is an explicit accepted contract, so changing its granularity or adding a
default expiry requires a reviewed compatibility decision. Cross-process ownership of
one SQLite file is separately disclosed in RISK-0003 and is not enforced by the
facade-local Busy check.

## Required correction

Provide bounded liveness and independent-store availability without allowing partial
writes or stale scopes to commit. State the chosen serialization boundary, host
cancellation/deadline contract and request cleanup. Enforce the one-owner
persistent-store boundary or explicitly resolve its contract before claiming it safe.
Preserve FIFO and isolation where promised; no silent narrowing of the concurrency
requirement.

## Regression gap and prep cases

AV01 queued request on A and local mutation on independent B; AV02
stalled/cancelled/expired active scope and stale callback; AV03 same-store
serialization, rollback, close/drop and FIFO; AV04 JS/native mixed-owner scheduling;
AV05 independent owners/processes opening one store with exact ownership failure rather
than corruption. Existing tests enforce global FIFO but do not establish liveness.

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
