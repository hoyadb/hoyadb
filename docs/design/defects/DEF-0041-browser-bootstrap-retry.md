---
id: DEF-0041
title: Failed browser runtime bootstrap permanently poisons later attempts
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-29
updated: 2026-10-29
parents: [SPEC-0044]
affects: [API-0011, API-0012]
regression_for: [TC-0048]
tags: [javascript, browser, bootstrap, regression]
---

# DEF-0041: Browser runtime cannot retry failed bootstrap

## Symptom

During the 2026-10-29 test-quality audit, two sequential default
`createWebRuntime()` calls with a worker that fails before ready returned the
same rejected promise and constructed only one worker. Verified by executing
the real factory under Node with a deterministic worker error stub:
`{"samePromise":true,"workersCreated":1}`. A transient worker/bootstrap failure
therefore prevents all subsequent default opens in that module instance.

## Root cause

`packages/client/src/web.js` caches the pending promise in `shared`
without retiring it after rejection. The corresponding Node factory in
`node-entry.js` already clears its own failed attempt. Existing channel tests
cover retirement of one failed realm, not the default factory's next attempt.
This is an implementation recovery gap; no new bootstrap protocol is proposed.

## Intended fix

Clear only the rejected attempt if it still owns the shared slot. Concurrent
callers must continue sharing one in-flight/default successful runtime. An
explicit caller-supplied worker must neither enter nor clear the default cache.
Keep public names, owner admission and wire frames unchanged.

## Regression

TC-0048 QUALITY05 covers concurrent calls, failed bootstrap followed by a
successful replacement, and explicit-worker isolation. The permanent regression
failed on promise identity before implementation and passed after the fix.
The record remains draft pending review; host-stub execution does not establish
actual browser/platform execution.
