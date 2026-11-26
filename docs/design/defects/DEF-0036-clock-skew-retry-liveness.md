---
id: DEF-0036
title: A corrected forward clock jump can indefinitely postpone persisted retries
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [SPEC-0029]
affects: [REQ-0018, REQ-0043, API-0006, API-0007]
regression_for: [TC-0032, TC-0039]
tags: [purpose-audit, remediation]
---

# DEF-0036: A corrected forward clock jump can indefinitely postpone persisted retries

## Symptom and evidence

A send planned at a large valid forward clock sample persists a deadline at that sample
plus backoff. Subsequent corrected samples are clamped to the persisted last_ms, leaving
the retry deadline unchanged and returning no envelope until the supplied time reaches
it. Root reproduced the stall and eventual retry through the public API in
target/purpose-audit-20261026/availability.txt.

## Confirmed cause and contract boundary

The same monotonic persisted wall-clock floor is used for command identity and retry
scheduling. It protects key order but makes retry progress depend on a mistaken
wall-clock value eventually becoming real. Supported-year range validation does not
detect this skew.

## Required correction

Decouple retry progress or provide an explicit safe clock-recovery contract, retaining
durable at-least-once delivery, unchanged frozen envelopes, unique monotonic identity
and bounded backoff/jitter. Preserve restart behavior and avoid a busy retry loop. Also
review acknowledged records whose final outcome is lost: their host/backend recovery
obligation must have a concrete path rather than a misleading retry guarantee.

## Regression gap and prep cases

CL01 forward jump then corrected samples with due retry progress; CL02 backward clock
and restart; CL03 jitter/cap/overflow and frozen-byte controls; CL04 multiple
pending/acknowledged work with explicit final-outcome recovery. Existing deadline tests
cover restart and ordinary backward samples, not a corrected forward skew.

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
