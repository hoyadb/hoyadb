---
id: THR-0001
title: Trusted context forged or overwritten on the client
type: threat
status: mitigated
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001]
tags: [security, stride, spoofing, elevation-of-privilege]
---

# THR-0001: Trusted context forged or overwritten on the client

## Description

An attacker with control of the client process — a tampered app build, a
malicious dependency in the app bundle, or a compromised runtime — sets or
rewrites the trusted context riding alongside a command or entity-diff
write, so writes reach the backend claiming an identity that is not theirs.

## STRIDE grid

| Category | Applies? | Notes |
| --- | --- | --- |
| Spoofing | Yes | The whole point of the attack: claim another identity. |
| Tampering | Yes | The envelope's `context.identity` field is modified. |
| Repudiation | No | Covered separately by `THR-0008`. |
| Information Disclosure | No | No data is read by this attack itself. |
| Denial of Service | No | Not relevant. |
| Elevation of Privilege | Yes | A spoofed identity may carry membership the attacker does not hold (`REQ-0026`). |

## Target ASVS level

L2 (OWASP ASVS 5.0).

## Relevant ASVS items (referenced loosely)

- V1.2.x — verify that trust boundaries are identified and that data
  crossing them is validated on the trusted side.
- V4.1.x — verify that access control decisions are enforced on a trusted
  service layer, never on data supplied by the client.

## Attack scenario

1. Attacker modifies client-side code so a command handler, or code around
   it, assigns `ctx.identity` to another user's value.
2. The forged envelope is pushed to the backend through the developer's own
   transport.
3. If the backend trusted `context.identity` as its authorization source,
   the write would be applied as that other identity.

## Mitigation

- `REQ-0015` — hoyadb obtains the trusted context from the developer's
  extension point and forwards it unmodified; a command handler only ever
  *reads* `ctx.identity` and an attempt to assign it is rejected. This is
  hoyadb's own client-side plumbing guarantee.
- `REQ-0015` also carries the load-bearing half of this mitigation into the
  integration contract: the backend MUST re-derive the real identity from
  the transport's own authentication and use `context.identity` only for
  logging or cross-checking, never as the authorization source.
- Implemented by: `SPEC-0008`, `SPEC-0014`.

## Residual risk

**Client-unforgeability is a narrow guarantee and is stated as such.** It
means a handler cannot reassign the value inside hoyadb's plumbing; it says
nothing about an attacker who controls the process entirely and can forge
the envelope before or after hoyadb touches it. The real control is the
backend's independent re-derivation — and that backend is **never hoyadb's
code**, so hoyadb cannot enforce it, only document it. A developer who
trusts `context.identity` as the authorization source defeats this
mitigation entirely, and hoyadb has no way to detect that.

Marked `mitigated` on the strength of the client-side guarantee plus the
documented backend requirement; the residual exposure above is accepted at
L2 and is inseparable from `ADR-0019`'s decision that hoyadb owns no wire.

## Traceability

- Parent: `ARC-0001`.
- Mitigated by: `REQ-0015`.
- Verified by: `TC-0012`.
