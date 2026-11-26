---
id: DEF-0010
title: Owned prefix terminal flag can prevent byte-exact delivery
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, TC-0036]
affects: [REQ-0046, API-0011]
regression_for: [TC-0036]
tags: [sdk, review, reply-budget]
---

# DEF-0010: Owned prefix terminal flag can prevent byte-exact delivery

## Symptom and investigation status

At intake, Claude identified an unexecuted boundary hypothesis on 02621eb: candidate prefix
sizing uses `has_more:true`, but a terminal `false` costs one additional byte.
An exact candidate could therefore fit while its emitted frame fails, retaining
the same queue on every retry. A later actual public two-event drain reproduced
LimitExceeded at the exact boundary before the implementation changed.

## Suspected cause and intended correction

`commands::prefix` checks the final frame after candidate sizing but cannot fall
back to a smaller prefix. Size candidates using their actual terminal status,
or reduce a failing final prefix before returning. Preserve exact owned bytes
and order. An intrinsically oversized single item must remain pending and
return LimitExceeded; do not truncate, drop it or replay preparation.

## Acceptance and regression status

Extend TC-0036 G11 before tests: craft a two-item queue whose true-flag frame is
exactly 1 MiB and false-flag frame is one byte too large. First delivery must
make progress with a valid prefix; its retry delivers the remainder once and
the empty terminal frame fits. A single undeliverable item stays unchanged.
Observe the real production helper fail before implementation. A public drain
reproduction should verify the same boundary when input budgets permit it.

Size each candidate prefix with its actual terminal flag. Exact events must be
delivered once in successive prefixes. An oversized snapshot remains pending
through repeated failures until subscription cancellation. No stored-format change
or user-store migration is part of this correction.
