---
id: TC-0012
title: The library boundary — identity carried, no network call, no telemetry
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0015, REQ-0028, REQ-0032, THR-0001, THR-0007, THR-0008]
tags: [test, security, boundary]
---

# TC-0012: The library boundary — identity carried, no network call, no telemetry

## Purpose

Verify the three properties `OBJ-0006` measures: the trusted context is
carried unmodified and cannot be reassigned by a handler (`REQ-0015`),
hoyadb opens no network connection of its own (`REQ-0028`), and it sends no
telemetry while retaining command contents only locally (`REQ-0032`).

## Coverage boundary, stated

These verify **hoyadb's own half** of `THR-0001`, `THR-0007` and
`THR-0008`. The decisive control for the first and third — the backend
independently re-deriving identity rather than trusting
`context.identity` — runs in the developer's own code and **cannot be
tested from here**, which is why both threats sit below `mitigated`.

Likewise, hoyadb cannot prevent a developer's own logger or analytics SDK
from capturing queue state it deliberately exposes (`THR-0007`).

## Scenarios

See `TC-0012-boundary-identity-and-egress.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0015`, `REQ-0028`, `REQ-0032`, and the client-side half of
  `THR-0001`, `THR-0007`, `THR-0008`.
