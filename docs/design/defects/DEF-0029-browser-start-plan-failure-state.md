---
id: DEF-0029
title: Browser start preflight failure consumes its permit and strands the controller
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-25
updated: 2026-10-25
parents: [SPEC-0033]
affects: [SPEC-0033, TC-0037]
regression_for: [TC-0037]
tags: [sdk, benchmark, admission]
---

# DEF-0029: Browser start preflight failure strands the controller

## Observed behavior and cause

At commit6b16bf6, a root-run isolated synthetic probe admitted warm coordinates
memory/cohort0/run1/worker probe:1, then created memory.c00.r1.json in the owned
temporary output directory before calling start. The existing-evidence guard
rejected, but the resulting collector had state active, permit null, active null,
starts1, runs0, workers0 and no blocked failure. Recovery rejected with
"nothing to recover"; a fresh admission rejected as a pre-issued permit.
The probe used no browser, SDK, worker or measurement and removed its temporary
directory on exit. No accepted data or existing capture was changed.

start, start_cold and start_resources call consume before their plan validation.
The shared consume path irreversibly clears the permit, advances the start
generation and changes state before the existing-evidence guard can reject.
Admission checked the plan earlier, but output evidence can appear between
admission and start. The existing TC-0037 admission and DEF-0023 tests do not
exercise that preflight failure. No unadmitted worker or false accepted
measurement was demonstrated; this is a controller recovery failure.

## Required behavior

Under the existing collector lock, complete start preflight before committing
the permit consumption and active transition. A plan rejection leaves the
admitted permit unchanged and unconsumed, with no run, worker, active identity,
start event or generation advance. The parent can explicitly REVOKE that
permit, then RECOVER and admit a different valid identity. There is no implicit
retry or replay, and the conflicting evidence remains byte-identical.

Preserve exact permit matching and consumption-time source/artifact rechecks.
Drift must still clear and block the permit as already specified; a mismatched
start must not consume it. The order is non-mutating permit match, existing
drift recheck (clear and block), non-mutating plan validation, then consumption
and start-generation commit. Drift retains precedence when an output conflict
also exists. Record plan rejection as a non-blocking rejected event, without
changing the permit or recording a start. Do not change worker clocks, workload, counts,
cleanup semantics, public SDK code or previously captured evidence. This fix
addresses plan validation failures, not arbitrary filesystem/log-write failures.

## Named cases before implementation

- P01: Given an admitted warm, cold or resource run, when its expected output
  file appears before start, then each actual start route rejects for existing
  evidence without changing the admitted permit, state, generation, run set,
  worker set or active identity. No start event is logged and the file's bytes
  are preserved. Observe behavioral RED before the minimal correction.
- P02: Given that rejection, when the parent explicitly revokes and recovers,
  then a new valid identity can be admitted and started exactly once, without
  deleting or overwriting the conflicting file or replaying the rejected run.
- P03: Given a mismatched permit or consumption-time frozen-source drift,
  when each actual start route is called, then the existing rejection and
  blocking semantics remain intact and no run/worker is registered. Include
  a combined drift-and-existing-evidence conflict case to prove drift wins.
  Retain existing A01–A08, N01–N08 and DEF-0023 regression assertions.

Use stdlib synthetic controller tests and temporary directories only. No
benchmark or browser integration is needed to prove this state transition.
Record source and RED/GREEN identities; a changed source cut supersedes only
future runs, not historical manifests. Rollback reverts unpublished harness
changes while preserving captured originals and failure evidence.

## Correction and root verification

The shared consumption path now accepts the existing route plan and runs it
after permit matching and the drift recheck, before changing admission state
or the start generation. A plan failure logs a non-blocking rejection and leaves
the permit available for explicit revocation and recovery. All three start
routes use this path under the existing lock.
