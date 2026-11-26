---
id: THR-0002
title: Local store file read outside the application's own runtime
type: threat
status: mitigated
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001]
tags: [security, stride, information-disclosure]
---

# THR-0002: Local store file read outside the application's own runtime

## Description

hoyadb's local store holds synced data covered by the same membership and
authorization rules as the backend (`REQ-0026`). An attacker who obtains
the raw file — rather than going through the running app — reads that data
with none of those rules applied.

## STRIDE grid

| Category | Applies? | Notes |
| --- | --- | --- |
| Spoofing | No | No identity is claimed. |
| Tampering | Partial | An attacker with file access could also modify it; the same control applies. |
| Repudiation | No | Not relevant. |
| Information Disclosure | Yes | The primary impact — synced, permission-scoped data read directly. |
| Denial of Service | No | Not relevant. |
| Elevation of Privilege | No | No privilege boundary is crossed within the system. |

## Target ASVS level

L2 (OWASP ASVS 5.0).

## Relevant ASVS items (referenced loosely)

- V6.1.x — verify that sensitive data stored on the client is protected at
  rest.
- V6.2.x — verify that approved, well-vetted cryptographic algorithms and
  libraries are used, never bespoke implementations.

## Attack scenario

1. A device is stolen while unlocked, or an app backup is extracted and
   inspected, or another process reads hoyadb's file directly on a rooted
   or jailbroken device.
2. Without at-rest protection, the raw store yields every synced record
   plus the offline queue's contents — including command arguments and
   trusted-context values (`REQ-0032`).

## Mitigation

- `REQ-0030` — the local store is encrypted at rest through the concrete
  engine's own encrypted-storage capability, using a vetted, authenticated
  scheme from an established library, never a hand-rolled one.
- `REQ-0031` — the local store is excluded from platform device and app
  backups by default, removing the backup-extraction path rather than
  relying only on the key staying secret.
- Implemented by: `SPEC-0014`, `SPEC-0002`.

## Residual risk

**This does not protect against a compromised running app**, because the
key must be reachable on-device for offline operation to work at all. This
is an accepted limitation explicitly recorded in `ADR-0020` and
`SPEC-0014` — the same trade-off every on-device
at-rest scheme with offline requirements makes, not a gap to close.

Two further residual items, both disclosed rather than mitigated:

- **The concrete cipher and library per engine is deferred to
  implementation time** (`ADR-0019`). The floor is fixed; the pick is not,
  so this threat's mitigation is not yet checkable against a named
  algorithm — which `VIS-0001`'s own "state of the art is a floor, not a
  slogan" standard ultimately requires.
- **Key rotation on an already-encrypted store is not covered anywhere**
  (`ADR-0020`). A key believed compromised has no designed remedy.
- hoyadb cannot detect a developer supplying a weak or hard-coded key.

## Traceability

- Parent: `ARC-0001`.
- Mitigated by: `REQ-0030`, `REQ-0031`.
- Verified by: `TC-0015`.
