---
id: ADR-0012
title: Corrections and rejections surface through the ordinary subscription path, with an explicit rollback
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, commands, error-handling, rollback]
---

# ADR-0012: Corrections and rejections surface through the ordinary subscription path, with an explicit rollback

Source: `legacy/PLAN_DECISIONS.md` D4, folded into `legacy/VISION.md`'s "Sync — error
handling & rollback" and `legacy/docs/06-sync/command-protocol.md`.

## Context and problem statement

`legacy/VISION.md` said the backend's authoritative outcome "can differ from,
override, or reject" the local guess, but nothing said what application
code actually *observes* when that happens — leaving the whole optimistic
model without a defined failure surface.

## Considered options

- **Surface it through the same subscription/event/callback mechanism used
  for ordinary data changes, plus an explicit rollback path** that reverts
  the local optimistic mutation.
- **A rejected promise/callback tied to the original `execute()` call.**
- **A status field the app has to poll or check.**
- **A separate error-only channel** distinct from the data-change path.

## Decision outcome

In the context of telling an application that its optimistic local result
was corrected or rejected, facing the fact that the correction is itself a
data change, we decided for **the ordinary subscription/event/callback
mechanism plus an explicit rollback path** and against a rejected promise
tied to the original call, a polled status field, and a separate error-only
channel, to achieve one notification path for every data change regardless
of origin, accepting that hoyadb provides the correction and rollback
*mechanism* but not the recovery *policy*.

### Consequences

**Good**
- One path to observe, not two. A correction is a data change; routing it
  down the data-change channel keeps the app's rendering logic uniform.
- A rejected promise tied to `execute()` would not work: the outcome can
  arrive long after the call resolved locally, possibly after an app
  restart, and the queue is durable across restarts (`ADR-0016`).
- Rollback also serves cancellation (`ADR-0028`) and rejected-parent
  cascades (`ADR-0016`) — one mechanism, three uses.

**Bad**
- **Recovery is explicitly the developer's responsibility.** What the app
  does in response — retry, surface an error, reconcile some other way — is
  not hoyadb's to decide. D4 states this directly: "It will be the
  responsability of the developer to implement the fallback strategy."
- The worked `useManageRollback(entity, id, fn)` example is framed as *an*
  example for one binding (a hook-style JS/React callback), **not** the
  only supported shape every binding must copy. No cross-binding shape is
  fixed by this ADR.
- The same mechanism must also carry the backend-assigned id swap
  (`ADR-0016`), which D4 names as needing "a full test and exemple."

### Confirmation

Confirmed by `SPEC-0008`'s error model, `UX-0001`'s correction step, and
the rejection/rollback test cases in `TP-0001`.

## Traceability

- Parent: `OBJ-0003`.
- Referenced by: `SPEC-0008`, `SPEC-0009`, `SPEC-0015`, `UX-0001`.
- Depends on: `ADR-0011` (the outcome enum this reacts to).
