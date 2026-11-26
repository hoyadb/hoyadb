---
id: REQ-0032
title: Command and trusted-context contents stay in local state; no telemetry leaves the device
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0006]
mitigates: [THR-0007]
tags: [security, privacy, logging, telemetry]
---

# REQ-0032: Command and trusted-context contents stay in local state; no telemetry leaves the device

## Pattern

Ubiquitous, with an Unwanted-behaviour clause against external
transmission.

## Statement

THE system SHALL retain command arguments and trusted-context values only
within its own local queue and state, as required to operate the offline
queue (`REQ-0018`) and provide queue introspection (`REQ-0021`).

THE system SHALL NOT transmit telemetry, diagnostics, usage data, or any
other content to any external destination by default — and, per
`REQ-0028`, makes no network call of its own at all.

## Examples

Example 1 — what is retained and where: the `add-order-v1` envelope's
`args` (`customerId`, `items`) and `context.identity` sit in hoyadb's
reserved local queue table until the command is acknowledged and discarded
(`legacy/docs/07-security/security-model.md`). That local state is covered by
`REQ-0030`'s at-rest encryption, with the limits `REQ-0030` records.

Example 2 — no egress: a developer wanting telemetry adds it themselves, on
top of hoyadb, where they control what is sent and where. hoyadb does not
do it on their behalf — it is a library, not a hosted service.

Example 3 — why redaction inside the queue was rejected: a redacted command
cannot be retried, and per-command status could not show what is pending.
Redaction would break the queue itself.

## Open

**hoyadb cannot stop a developer's own logging, crash reporter or analytics
SDK from capturing this data** once it is read through the debugging APIs
hoyadb deliberately provides (`REQ-0021`). hoyadb's mitigation is bounded
to its own behaviour (`THR-0007`).

The **diagnostic export** named as useful in
`legacy/docs/08-non-functional/observability.md` is undesigned and explicitly has
**no redaction policy decided** — which is precisely where this requirement
would need extending (`ADR-0028`).

## Traceability

- Parent: `OBJ-0006`.
- Under: `ADR-0021`.
- Satisfied by: `SPEC-0014`, `SPEC-0015`.
- Verified by: `TC-0012`.
