---
id: ADR-0021
title: Command and trusted-context contents stay in local state; hoyadb sends no telemetry
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0006]
tags: [security, privacy, logging, telemetry]
---

# ADR-0021: Command and trusted-context contents stay in local state; hoyadb sends no telemetry

Source: `legacy/PLAN_DECISIONS.md` D13, folded into `legacy/VISION.md`'s "Security" and
`legacy/docs/07-security/security-model.md`.

## Context and problem statement

Command arguments and trusted-context values are exactly the kind of data
that lands in debug logs or crash reports by accident. hoyadb must hold
both — the offline queue cannot function otherwise (`ADR-0016`), and
developer-facing debugging visibility depends on it (`ADR-0028`).

## Considered options

- **Log them only within hoyadb's own local queue and state; send no
  telemetry externally by default.**
- **Redact command arguments** inside the local queue as well.
- **Ship opt-in telemetry** that reports queue/sync state to a collector.

## Decision outcome

In the context of where command and trusted-context contents may appear,
facing the fact that the offline queue must persist them to work at all, we
decided for **holding them only in hoyadb's own local queue and state, with
no external telemetry by default** and against redacting them inside the
local queue and against shipping any telemetry facility, to achieve the
debugging visibility `ADR-0028` requires without hoyadb becoming a data
egress path, accepting that this data does sit at rest on the device.

### Consequences

**Good**
- hoyadb is a library, not a hosted service. D13's answer is explicit:
  "think library and library API, not a Saas."
- Redacting inside the local queue was rejected because it would break the
  queue itself — a redacted command cannot be retried, and per-command
  status could not show what is pending.
- A developer who wants telemetry adds it themselves, on top of hoyadb,
  where they control what is sent and where.

**Bad**
- Command arguments and trusted-context values do sit in local state at
  rest. That local state is covered by `ADR-0019`/`ADR-0020`'s at-rest
  encryption, with exactly the limits `ADR-0020` records.
- **hoyadb cannot stop a developer's own logging, crash reporter or
  analytics SDK from capturing this data** once they read it through the
  debugging APIs hoyadb deliberately provides (`ADR-0028`). Recorded as
  `THR-0007`; hoyadb's mitigation is bounded to its own behaviour.
- The **diagnostic export** named in `ADR-0028` as useful is undesigned,
  and explicitly has **no redaction policy** decided for it — which is
  precisely where this decision would need extending. Disclosed, not
  designed (`legacy/docs/08-non-functional/observability.md`).

### Confirmation

Confirmed by `SPEC-0014`'s logging section and the telemetry test cases in
`TP-0001`, which verify hoyadb opens no outbound connection of its own
(the same property `REQ-0028` fixes for sync).

## Traceability

- Parent: `OBJ-0006`.
- Referenced by: `SPEC-0014`, `SPEC-0015`.
- Mitigates: `THR-0007`.
