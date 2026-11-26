---
id: DEF-0021
title: A throwing Kotlin observer can retain its drain guard and core token
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, TC-0036]
affects: [REQ-0046, SPEC-0033, API-0011, THR-0020]
regression_for: [TC-0036]
tags: [sdk, kotlin, observations, lifecycle]
---

# DEF-0021: A throwing Kotlin observer can retain its drain guard and core token

## Source observation

In bindings/android/src/main/kotlin/dev/hoyadb/Observations.kt, delivery catches
an application callback failure and sets Listener.live to false. The callback's
Subscription.close cleanup is conditional on live.getAndSet(false) having been
true; ManagedObservations.close likewise counts only listeners still marked live
when decrementing Database.drainGuard. Therefore the callback-failure path can
skip removing its listener, submitting its core unsubscribe and releasing its
registration's drain guard. An explicit subscription close cannot repair that
path, and closing/reopening the same Database can retain a nonzero guard even
after the old managed listener collection was cleared.

This is a source-verified intake during the benchmark quiet window. No controlled
Android reproduction has executed, no production disclosure is claimed and the
earlier successful ordinary callback/cancellation tests do not cover a throwing
observer. Runtime severity remains unmeasured until the regression below runs.

## Intended correction

Retire a managed listener through one idempotent cleanup path, independently of
whether callback delivery has already marked it inactive. Separate callback
liveness from ownership of the registration/token/guard, so retirement releases
each resource exactly once and explicit close after failure remains harmless.
Core unsubscribe must use the original captured owner; failure or a closed owner
must not redirect cleanup to a replacement owner. Preserve selected-executor
delivery, original native mutation results, buffered-event cancellation and
ordinary listener behavior. Do not add a retry/admission engine or claim recovery
from fatal VM errors.

## Regression owed before correction

After review/commit, add and execute an actual Android memory-owner regression:
register a managed observer whose initial callback throws a checked IOException,
await that attempt and call its Subscription.close. Wait for the retired drain
to finish. Assert a raw drain is no longer Busy, observation_stats reports zero
remaining observers, later operations/another observer still work, and
close/reopen on the same Database keeps the replacement raw drain usable. A
control observer on the selected executor retains its ordinary behavior. Test
watchdogs are bounded fixture controls, not SDK latency guarantees. Preserve the
expected behavioral RED before implementing cleanup, then run current
observation/lifecycle/command/transport regressions. No permanent regression or
implementation fix exists at intake time.

## Affected artifacts

TC-0036 G05/G10 already require cancellation, callback isolation and close cleanup;
the missing case is callback failure followed by explicit cancellation and owner
reuse. API-0011 is unchanged in meaning. A source revert rolls back the unpublished
fix; no customer store or persistent format is altered.
