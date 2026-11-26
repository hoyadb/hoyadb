---
id: DEF-0043
title: Default browser factory retains a runtime after its worker retires
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-29
updated: 2026-10-29
parents: [SPEC-0044]
affects: [API-0011, API-0012]
regression_for: [TC-0048]
tags: [javascript, browser, recovery, regression]
---

# DEF-0043: Retired browser runtime remains cached

## Symptom and checked cause

After a default worker reports ready, emit a worker error and call the real
`createWebRuntime()` again. Both the Opus 5.5 probe and the parent agent's
independent Node host probe returned
`{"sameRuntime":true,"workers":1,"terminated":true}`. The successful shared
promise stays cached after the channel retires. The worker cannot serve calls.
DEF-0041 addresses bootstrap rejection only and does not fix this case.

## Operator decision and fix

The operator chose fresh-worker recovery for later default opens on 2026-10-29.
API-0011 now records that decision. The factory clears only the shared attempt
belonging to the retiring worker. Concurrent replacement opens share one fresh attempt;
existing handles remain closed, and no request, owner or callback is migrated
or replayed. Explicit workers cannot clear the default slot.

## Regression

TC-0048 QUALITY10 covers a crash during an active call, failure of old handles,
concurrent fresh opens and late old-worker event isolation. The host regression failed on cached-promise identity before the fix, then
passed after it. It runs in its own test file/process to isolate default module
state. The artifact remains draft pending review; actual browser execution
remains a separate platform verification gap.
