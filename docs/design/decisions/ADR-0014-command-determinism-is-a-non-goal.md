---
id: ADR-0014
title: Command determinism between client and server runs is an explicit non-goal
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, commands, determinism, non-goal]
---

# ADR-0014: Command determinism between client and server runs is an explicit non-goal

Source: `legacy/PLAN_DECISIONS.md` D6, folded into `legacy/VISION.md`'s "Sync — command
determinism" and `legacy/docs/06-sync/command-protocol.md`.

## Context and problem statement

`ADR-0005`'s whole optimistic-local/authoritative-server model depends on
both runs of a command's handler reaching *compatible* results. D6 was
flagged as a genuinely open problem, not merely an undecided default: the
reference research records that this "only works cleanly if mutators are
deterministic given (tx state, args, ctx)," and even Zero — the strongest
precedent found — does not fully spell out how to guarantee it.

## Considered options

- **Declare it a non-goal**: hoyadb owns the sync infrastructure
  (queueing, transport, optimistic apply, triggering the re-run), not the
  handler's logic, which is the developer's own code on both sides.
- **Ban non-deterministic operations inside a handler body** — wall-clock
  reads, random values — unless threaded through explicitly, and enforce
  that ban.
- **Require a verification/testing regime** that hoyadb checks, proving the
  two handler implementations agree.
- **Run one shared handler on both sides** via an embeddable scripting
  runtime that executes identically inside the Rust core and on the
  backend.

## Decision outcome

In the context of guaranteeing that a command's client and server runs
agree, facing an open problem the strongest available precedent does not
solve either, we decided for **recording determinism as an explicit
non-goal** and against enforcing a ban on non-deterministic operations,
against a hoyadb-verified testing regime, and against committing to a
shared embeddable runtime, to achieve an honest and clearly-drawn
responsibility boundary rather than an unverifiable guarantee, accepting
that **this changes who owns the risk, not whether the risk is real**.

### Consequences

**Good**
- The boundary is stated rather than implied: hoyadb never inspects handler
  internals, so it can neither verify nor enforce agreement from outside.
- Because handler logic is the developer's, nothing stops the two sides
  being written in different languages — e.g. a PHP backend handler and a
  TypeScript client handler, kept compatible by the developer's own test
  suite, hand translation, or automated translation. Unusual, but not
  something hoyadb's contract forbids or needs to know about.
- The `ctx.now()` convention shows the recommended pattern (thread
  non-deterministic inputs through `ctx`, capture the client's value in the
  envelope as `context.capturedAt`, have the backend read it from there)
  without hoyadb enforcing it.

**Bad**
- **An integrating developer who cannot keep both sides compatible will
  still see it surface as an unexpected correction.** hoyadb has simply
  been explicit that it does not certify this for them. Tracked as a live
  project risk: `RISK-0002`, one of the two compounding bets in
  `legacy/VISION.md`'s "Riskiest assumptions."
- **There is no client/server handler-drift testing story**, despite this
  being named the single riskiest assumption in the project.
  `legacy/docs/09-testing.md` discloses this directly: nothing addresses
  contract/golden testing between a client handler and its independently
  authored server counterpart. Disclosed, not designed.
- The embeddable-runtime escape hatch is **flagged as worth researching,
  not adopted** — no such runtime is named or committed to. Carried forward
  as an open question, not a plan.

### Confirmation

There is nothing to confirm by test, by construction — that is the
decision. `SPEC-0008` records the boundary; `RISK-0002` tracks the residual
exposure; `legacy/docs/09-testing.md`'s disclosed gap is the honest status of
verification here.

## Traceability

- Parent: `OBJ-0003`.
- Referenced by: `SPEC-0008`, `SPEC-0013`.
- Risk: `RISK-0002`.
