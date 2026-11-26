---
id: DEF-0023
title: A waiting browser permit request can survive a direct worker start
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-25
updated: 2026-10-25
parents: [SPEC-0033]
affects: [SPEC-0033, TC-0037]
regression_for: [TC-0037]
tags: [phase-seven, benchmark, admission]
---

# DEF-0023: A waiting browser permit request can survive a direct worker start

## Symptom and evidence

Claude's focused source review of the41-file closure `bdbd890c` identified an
interleaving in `benchmarks/browser_server.py`. A page registers next_permit
before ADMIT. After ADMIT signals it, a supported direct start can acquire the
collector lock first and consume the permit. The waiter then sees no permit
and resumes waiting even though a worker is active. It occupies the only waiter
slot until expiry, so a subsequent page loop can stop with already_claimed.

The review derived this from source; no controlled regression has executed yet.
The one-use permit remains enforced. No unadmitted worker or false accepted
measurement was demonstrated. The committed416fffb amendment N03 explicitly
requires no waiter across a running worker; its current test covers only an
active worker at request entry, leaving this interleaving uncovered.

## Root cause and intended correction

The wait predicate checks permit/cancellation/shutdown/blocked state but omits
the transition to an active worker. Entry validation alone cannot cover a
transition while Condition.wait releases the lock. End the existing wait when
a direct start becomes active, reject consistently with the entry guard, clear
the waiter in its existing finally path and wake it when necessary. Preserve
the permit, cleanup, source-drift and held-state rules; never replay the run.

## Regression before implementation

Use deterministic coordination around the collector lock: register a waiter,
ADMIT, and let a direct start consume before the waiter reacquires the lock.
Assert the waiter retires promptly for the active-worker reason, cannot claim
a later permit, and does not occupy the slot after the worker finishes. Test
the supported warm/cold/resource start routes or their common transition.
No timing benchmark or arbitrary sleep-based race proof is required. Observe
behavioral RED before the minimal correction, then rerun N01–N08 and A01–A08,
including one-use, duplicate, revoke/recover, cancellation and shutdown checks.

Rollback reverts unpublished harness changes only. Existing captures remain
immutable; no SDK, dependency, database format or customer store changes.

## Correction and verification

The deterministic regression observed five behavioral failures, including the
old waiter actually claiming a later permit after the original run finished.
The common consume path now increments a start generation and signals waiters.
Each waiter captures that generation and a unique token; an intervening start
retires it even after completion, and its finally path clears only its own slot.
The original entry-guard rejection remains non-blocking for future admission.

Root independently reran90 Python tests and79 JavaScript tests successfully and
rehashed the new41-file closure `0dd6d44d` without mismatch. The full
[RED/GREEN evidence](../../implementation/phase-seven-benchmarks/def-0023-resolution-20261025/README.md)
preserves the disclosed post-fix test-path error and later HTTP regression.
Claude's [focused closure review](../../implementation/phase-seven-permit-resolution-review.json)
passes source/log inspection; it is not the final Phase7 review. Actual browser
integration is contaminated functional evidence, not an accepted baseline.
