---
id: TC-0015
title: At-rest encryption and backup exclusion
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0030, REQ-0031, THR-0002]
tags: [test, security, encryption, storage]
---

# TC-0015: At-rest encryption and backup exclusion

## Purpose

Verify that the local store is encrypted through the engine's own
capability, that hoyadb generates and stores no key of its own
(`REQ-0030`), and that backup exclusion is the default with an available
opt-in (`REQ-0031`) — together the mitigations behind `THR-0002`.

## Coverage boundary, stated

**The concrete cipher and library per engine is deferred to implementation
time** (`ADR-0019`), so no scenario asserts a named algorithm. Until that
pick is made, these scenarios verify the *mechanism and key boundary*, not
that the cipher meets `SPEC-0014`'s own "must be checkable" standard.

**Key rotation has no scenario** — it is uncovered anywhere in the design
(`ADR-0020`). Protection against a compromised running app is explicitly
**not** claimed and is not tested.

## Scenarios

See `TC-0015-at-rest-protection.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0030`, `REQ-0031`, `THR-0002`.
