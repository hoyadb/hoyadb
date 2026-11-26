---
id: DEF-0022
title: A custom Kotlin timer failure can mask a committed mutation result
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, TC-0036, DEF-0021]
affects: [REQ-0046, SPEC-0033, API-0011, THR-0020]
regression_for: [TC-0036]
tags: [sdk, kotlin, observations, callbacks]
---

# DEF-0022: A custom Kotlin timer failure can mask a committed mutation result

## Source observation and boundary

Database.execute awaits a successful native mutation, then invokes mutated. Its
managed observation wake calls schedule and armDeadline synchronously. With a
live count observer, armDeadline sets deadlineArmed=true and calls the public
injected HoyaTimer.schedule without containing an application exception. A
custom timer throwing IOException there can escape insert after the core has
already committed the new row; the armed flag also has no returned timer to
clear it. The caller therefore can see failure in place of the committed Row.

This is a source-verified custom-capability candidate found during a quiet
benchmark window. No controlled Android runtime regression has executed. The
default internal timer was not observed to fail; no default-runtime failure
rate or customer impact is claimed. Existing normal injected-deadline tests
exercise a successful timer and do not cover this exception path.

## Intended correction

Contain an ordinary injected-timer exception inside managed scheduling after
native success. Preserve the mutation's actual committed result and retire the
affected observation through its existing idempotent cleanup; clear the armed
deadline state and release the original core token/guard exactly once. Any
surfaced SDK error remains a static kind without the timer's message/cause.
Other listeners, later owned operations and explicit drain after retirement must
remain usable. No retry engine, second count throttle, global exception handler,
invented native event or claim of recovery from fatal VM errors is introduced.
The normal single-unpostponed16ms timer path remains unchanged.

## Regression owed before correction

After review/commit, run an actual Android memory-owner test with a real initial
count observer and a custom HoyaTimer that throws checked IOException carrying a
synthetic canary. Insert one row. Assert insert returns its exact committed Row,
an independent fetch sees that row, implicit output contains no canary and
retiring/cancelling the affected observer leaves observation_stats empty and raw
drain usable. A normal control observer and a fresh normal-timer owner continue
to deliver; no stuck armed flag suppresses a replacement observation lifecycle.
Preserve the expected behavioral RED before changing implementation, then rerun
the current ordinary timer, callback/cancellation, owner and command regressions.

## Traceability and rollback

TC-0036 G05/G10 require managed notification scheduling and isolation of injected
application callbacks from durable results; G11 requires truthful committed
outcomes. The missing case is a failing timer after a successful core write.
DEF-0021 provides the separately reviewed registration/token cleanup correction;
this defect covers the scheduler exception boundary. Rollback reverts unpublished
source only and does not alter any customer store or persistent format.
