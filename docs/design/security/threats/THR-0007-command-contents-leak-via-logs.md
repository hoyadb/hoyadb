---
id: THR-0007
title: Command arguments and trusted-context values leak through logs or crash reports
type: threat
status: mitigated
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001]
tags: [security, stride, information-disclosure, privacy]
---

# THR-0007: Command arguments and trusted-context values leak through logs or crash reports

## Description

Command arguments carry application domain data, and the trusted context
carries an authenticated identity value. Both are exactly the kind of
content that lands in debug logs, crash reports, or analytics payloads by
accident — and hoyadb must hold both for the offline queue to function at
all (`REQ-0018`).

## STRIDE grid

| Category | Applies? | Notes |
| --- | --- | --- |
| Spoofing | No | Not directly; a leaked identity value could enable `THR-0001` downstream. |
| Tampering | No | Not relevant. |
| Repudiation | No | Not relevant. |
| Information Disclosure | Yes | The primary impact — domain data and identity values reaching an unintended destination. |
| Denial of Service | No | Not relevant. |
| Elevation of Privilege | No | Not directly. |

## Target ASVS level

L2 (OWASP ASVS 5.0).

## Relevant ASVS items (referenced loosely)

- V8.3.x — verify that sensitive data is not written to logs or included in
  error/diagnostic output.
- V8.1.x — verify that sensitive data is not sent to third parties without
  an explicit, necessary purpose.

## Attack scenario

1. hoyadb holds `add-order-v1`'s `args` and `context.identity` in its local
   queue until the command is acknowledged.
2. The application reads queue state through the debugging APIs hoyadb
   deliberately exposes (`REQ-0021`) to render a sync-status UI.
3. That state is passed to the app's own logger, crash reporter, or
   analytics SDK — and leaves the device to a third party that was never
   meant to see it.

## Mitigation

- `REQ-0032` — hoyadb retains command and trusted-context contents **only**
  within its own local queue and state, and transmits no telemetry,
  diagnostics or usage data to any external destination by default.
- `REQ-0028` — hoyadb makes no network call of its own at all, so it has no
  egress path to leak through even in principle.
- `REQ-0030` — that retained local state is encrypted at rest, with the
  limits `THR-0002` records.
- Implemented by: `SPEC-0014`, `SPEC-0015`.

## Residual risk

**hoyadb's mitigation is bounded to hoyadb's own behaviour.** Once an
application reads queue state through `REQ-0021`'s introspection APIs — the
whole point of which is developer visibility — hoyadb cannot stop that
application's logger, crash reporter or analytics SDK from capturing it.
That is the integrating developer's responsibility, and `ADR-0021` says so.

**The diagnostic export named as useful in
`SPEC-0015` is undesigned and explicitly has
no redaction policy decided** — it would be a structured dump of exactly
this content, intended to be attached to a support ticket. This is the
clearest place this threat would need re-analysis once that feature is
designed; it is disclosed, not solved.

Redaction *inside* the local queue was considered and rejected: a redacted
command cannot be retried, and per-command status could not show what is
pending, so redaction would break the queue itself (`ADR-0021`).

## Traceability

- Parent: `ARC-0001`.
- Mitigated by: `REQ-0032`, `REQ-0028`.
- Verified by: `TC-0012`.
