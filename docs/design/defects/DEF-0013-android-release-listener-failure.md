---
id: DEF-0013
title: Throwing Android release listener can strand a completed native reply
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, TC-0036]
affects: [REQ-0046, API-0011]
regression_for: [TC-0036]
tags: [sdk, kotlin, ownership, callbacks]
---

# DEF-0013: Throwing Android release listener can strand a completed native reply

## Symptom

Source review identified an unverified runtime failure in
HoyaNativeTransport.released: an inline Executor and Kotlin Runnable throwing
java.io.IOException escape the RuntimeException catch. released is invoked from
call's finally before posting its already-collected native reply. An escaping
listener can therefore prevent delivery and later-listener notification, despite
the original terminal native operation having returned. Intake is source-backed;
no actual Android IOException regression has yet run. No application data/key is
needed: synthetic invalid-handle close gives a completed static core reply.

## Root cause

The subscriber boundary catches RuntimeException only; Kotlin permits a checked
Exception from Runnable.run, and an inline executor propagates it synchronously.

## Intended correction

Retire a throwing/rejecting subscriber while preserving the original owned
native reply, continuing other live subscribers and later channel operations.
Do not turn subscriber failures into a native core result, add an admission queue,
change release-trigger operations or install a global uncaught-exception handler.
The minimal boundary should contain application callback exceptions independently
from executor scheduling exceptions; operational VM fatal errors need no new
recovery guarantee. No persistence/wire/dependency change; rollback is source revert.

## Regression and verification plan

Add TC-0036 K08/G06/G10: subscribe an inline release listener that throws synthetic
IOException and a later normal listener; perform a terminal actual native attempt;
assert the exact original static reply reaches its callback once, the later live
listener runs once, the bad listener is retired and an ordinary later operation
still completes. Test-only capture of the owned worker's uncaught failure may
prevent Android's process-wide default crash policy from hiding the red assertion;
if used, label and isolate it, never production/global behavior.
Run actual Android compiling RED after this preparation is reviewed/committed,
then the minimal fix and all transport/owner/suspended command regressions.
