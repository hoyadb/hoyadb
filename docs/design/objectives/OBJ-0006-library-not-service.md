---
id: OBJ-0006
title: hoyadb stays a library — it owns local data protection and nothing beyond it
type: objective
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [VIS-0001]
tags: [security, scope, library]
---

# OBJ-0006: hoyadb stays a library — it owns local data protection and nothing beyond it

## Statement

hoyadb protects the local store it actually owns, and makes no network
call, implements no authentication, and emits no telemetry — so that the
integrating developer keeps control of every boundary hoyadb does not
itself sit on.

## Baseline

n/a — new capability. This is a scope boundary rather than an improvement
on prior art: `legacy/VISION.md` fixes it directly, and `legacy/PLAN_DECISIONS.md`'s
D11, D12 and D13 each resolve one part of it.

## Target metric

Four pass/fail properties, measured by the security and boundary cases in
`TP-0001` and by inspection against `SPEC-0014`:

1. Count of network connections hoyadb's own code opens: **0**. It calls
   into a developer-supplied `transport` object instead (`REQ-0028`).
2. Count of authentication mechanisms hoyadb implements: **0**. It defines
   an extension point for an already-authenticated identity and carries
   that value client-unforgeably (`REQ-0015`).
3. Count of external telemetry destinations hoyadb sends to by default:
   **0**. Command and trusted-context contents are held only in its own
   local queue/state (`REQ-0032`).
4. Local storage is encrypted at rest through the concrete engine's own
   encrypted-storage capability, with key material supplied by the
   developer, and is excluded from platform device/app backups by default
   (`REQ-0030`, `REQ-0031`).

## Target date

Not fixed — see `OBJ-0001`'s note on `ADR-0008`.

## Rationale

`legacy/VISION.md`'s "Security" section sets the floor ("state of the art is a
floor, not a slogan") *and* the boundary: hoyadb "has no wire of its own
to check," so transport security and auth standards are the developer's to
choose. D7, D11 and D13's answers all reduce to the same posture, stated
most plainly in D7: "Don't forget we are creating a library, not an
application."

The limits of what property 4 actually buys are stated, not glossed:
encryption at rest protects the raw local file from being read outside the
app's own runtime, and does **not** protect against a compromised running
app, since the key must be reachable on-device for offline operation to
work at all (`THR-0002`).

## Traceability

- Parent: `VIS-0001`.
- Realized by: `REQ-0015`, `REQ-0028`, `REQ-0030`, `REQ-0031`, `REQ-0032`.
- Threats in this area: `THR-0001`, `THR-0002`, `THR-0005`, `THR-0007`.
