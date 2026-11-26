---
id: TC-0003
title: Migration chain-or-reset, and the reserved namespace
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0006, REQ-0033, NFR-0005]
tags: [test, migrations, compatibility]
---

# TC-0003: Migration chain-or-reset, and the reserved namespace

## Purpose

Verify both halves of `REQ-0006` — an authored chain applies in order and
preserves data; a missing chain resets rather than guessing — and that a
schema declaring a name inside hoyadb's reserved namespace is rejected
(`REQ-0033`, `NFR-0005`).

## Not covered, deliberately

**What happens to unacknowledged queued writes at the moment of a reset has
no scenario here, because it has no decided behaviour to verify**
(`REQ-0006`, `SPEC-0005`). Writing a scenario would mean inventing the
answer.

## Scenarios

See `TC-0003-migrations-and-namespace.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0006`, `REQ-0033`, `NFR-0005`.
