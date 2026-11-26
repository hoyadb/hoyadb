---
id: ADR-0018
title: Two devices under one identity are an ordinary conflict case, not a special one
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, identity, conflict-resolution]
---

# ADR-0018: Two devices under one identity are an ordinary conflict case, not a special one

## Context and problem statement

hoyadb's trusted context identifies *who*, not *which device or session*.
Two devices authenticated as the same identity, both offline, both writing
to the same field, is exactly where per-field LWW (`ADR-0007`) is weakest.
The question was whether the trusted context needs a device identifier and
whether this case deserves distinct handling.

## Considered options

- **Treat it as an ordinary case** for whatever conflict-resolution rule
  already applies to the write, with no device identifier required by
  default.
- **Require a device/session identifier** in the trusted context and add
  device-aware merge logic.
- **Surface it to the end user** rather than resolving it automatically.

## Decision outcome

In the context of concurrent offline writes from two devices under one
identity, facing the weakest point of the LWW default, we decided for
**treating it as an ordinary conflict-resolution case with no required
device identifier** and against mandating a device/session id with
device-aware merge logic and against surfacing it to the user, to achieve a
smaller identity contract with no second mechanism to design, accepting
that hoyadb therefore offers no built-in way to say "edited from your other
device."

### Consequences

**Good**
- The identity contract stays minimal: an opaque, already-authenticated
  value (`ADR-0019`). Membership is computed from identity, so both devices
  hold the same scoped data — consistent with `ADR-0006`.
- Where an integration genuinely needs to distinguish devices, that is a
  second field on the *same* extension point (e.g. `{ identity, deviceId }`)
  — an extension of the existing design, not a separate mechanism.

**Bad**
- **The weakest LWW case gets no extra protection.** Two devices editing
  one field offline resolve by the ordinary rule, with the loss `ADR-0007`
  already accepts.
- ADR-0018's own answer is tentative — "Don't think it's our part of the job,
  it's developer choice? Maybe we can add both cases in the integration
  guide." Recorded here as decided because `VIS-0001` and the docs adopted
  it, but the original answer carried a question mark.
- **The integration guide this defers to does not fully exist.** `SPEC-0051`'s
  target structure has no standalone integration guide;
  `SPEC-0013` covers only the sync-implementation
  slice. The single-device and multi-device patterns currently live in the
  conflict-resolution and scoping docs' own ADR-0018 sections. A real
  documentation gap, disclosed in both docs.

### Confirmation

Confirmed by `SPEC-0011` and `SPEC-0012`, each carrying its half of this
answer, and by `SPEC-0008`'s identity contract showing the extension point
a `deviceId` would be added to.

## Traceability

- Parent: `OBJ-0003`.
- Referenced by: `SPEC-0011`, `SPEC-0012`.
- Depends on: `ADR-0007`, `ADR-0019`.
