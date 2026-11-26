---
id: TC-0006
title: Command envelope contents and versioning
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0012, REQ-0013, REQ-0014]
tags: [test, sync, commands, wire-format]
---

# TC-0006: Command envelope contents and versioning

## Purpose

Verify what the envelope carries and — the load-bearing assertion for
`OBJ-0003` — what it deliberately does **not**: the row mutations the local
handler produced never travel as the authoritative payload (`REQ-0012`).
Also verifies that hoyadb attaches the key, versions and context itself
(`REQ-0013`, `REQ-0014`).

## Scenarios

See `TC-0006-command-envelope.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0012`, `REQ-0013`, `REQ-0014`.
