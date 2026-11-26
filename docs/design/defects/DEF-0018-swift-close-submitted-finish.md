---
id: DEF-0018
title: Swift close can hide a submitted command's committed result
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, TC-0036]
affects: [REQ-0046, API-0011]
regression_for: [TC-0036]
tags: [phase-seven, swift, command, lifecycle]
---

# DEF-0018: Swift close can hide a submitted command's committed result

## Source observation and qualification

The Swift foundation at95bcc96 deliberately exempts CommandControl.finishing
from cancellation. API-0011 preserves the actual durable result once finish has
been submitted. However, Database.submitCommandFinish rejects its successful
native reply as Closed when owner/epoch changed while awaiting that reply.
Database.close invalidates owner/epoch without distinguishing that submitted
result. A real finish that commits before close but whose successful host reply
is held until after close is therefore hidden by the owner-generation guard.

This is a source-only peer-review finding, not yet a runtime reproduction.
CommandTests already has a real HeldFinishRuntime regression for task.cancel,
but it does not cover close or owner replacement across the held finish reply.
Run the additional formal regression after this preparation is reviewed and
committed, before changing implementation.

## Intended correction and rollback

Keep the captured submitted operation's actual native outcome observable to
its original command caller. Preserve stale-scope rejection and prevent the
old completion from scheduling observations or sync work on a replacement
owner. Do not invent success for a rejected native finish, replay a command,
retract a durable write or block close indefinitely on an application-held
reply. Existing queued/admitted cancellation still releases their scopes.
No ABI, dependency, persistent-format or product-policy change. Revert the
source change to roll back, preserving stores and recovery records.

## Regression owed before implementation

TC-0036 G07/G09 must hold an actual committed finish response, close its owner,
then release the response and require the exact CommandResult, including key
and lossless result. A retained real store reopened independently must contain
the committed row and recovery record. A replacement owner must retain its own
rows/subscriptions and receive no stale completion side effects. Keep the
existing cancel-after-submit positive control and pre-submit close rejection.
Record the expected Closed RED and minimal GREEN on matching source/artifacts.

## Resolution evidence

The actual retained-store regression failed with Closed after a committed
finish reply was held across close/reopen. The correction returns that
submitted operation's actual native outcome while scheduling observation/sync
side effects only for its original live owner and epoch. Exact key/value,
independent retained row/queue, replacement state and no stale drain are tested.
All44 Swift tests and the outside-checkout public macOS writer/reader pass;
separate strict formatting returns0. [The matching checkpoint](../../implementation/phase-seven-swift-public-testing/README.md)
retains preparation, failure, successful checks and source/native artifact
hashes. Current iOS acceptance remains open and earlier evidence is unchanged.
