---
id: THR-0005
title: Sync traffic carried over an unencrypted or weakly authenticated transport
type: threat
status: accepted-risk
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001]
tags: [security, stride, information-disclosure, spoofing, accepted-risk]
---

# THR-0005: Sync traffic carried over an unencrypted or weakly authenticated transport

## Description

Command envelopes and entity-diff payloads carry application data, the
trusted-context identity, and idempotency keys. If the developer's own
transport sends them over an unencrypted channel, or authenticates with a
home-grown scheme, that traffic is exposed to interception and forgery in
transit.

## STRIDE grid

| Category | Applies? | Notes |
| --- | --- | --- |
| Spoofing | Yes | A weak or bespoke auth scheme lets an attacker impersonate a client. |
| Tampering | Yes | Envelopes can be modified in flight on an unencrypted channel. |
| Repudiation | No | Covered by `THR-0008`. |
| Information Disclosure | Yes | Application data and identity values are readable in transit. |
| Denial of Service | No | Not specific to this threat. |
| Elevation of Privilege | Yes | A forged envelope may claim membership the attacker does not hold. |

## Target ASVS level

L2 (OWASP ASVS 5.0).

## Relevant ASVS items (referenced loosely)

- V9.1.x — verify that TLS is used for all client connectivity and that
  insecure fallbacks are not permitted.
- V3.x / V2.x — verify that session and authentication mechanisms use
  established, vetted standards rather than custom schemes.

## Attack scenario

1. A developer implements the `Transport` interface (`REQ-0028`) over plain
   HTTP, or authenticates with a custom token scheme of their own design.
2. An attacker on the network path reads command arguments and
   `context.identity`, or replays and modifies envelopes.
3. Because `context.identity` is the value hoyadb carries, an attacker who
   can forge envelopes reaches the same position as `THR-0001`, without
   needing to compromise the client.

## Decision not to mitigate

**This sits at `accepted-risk` because hoyadb structurally cannot mitigate
it.** `ADR-0019` (from ADR-0019) records the reasoning: hoyadb makes no network
call itself (`REQ-0028`) — it hands the developer a payload and calls into
their transport object. It therefore **has no wire of its own** on which to
pin a TLS minimum or an auth-token standard, and claiming one would be an
unenforceable promise.

What hoyadb does state, and what remains accepted:

- The **bar is fixed even though the pick is not**: transport must be
  encrypted, and the authentication mechanism must be an established,
  vetted standard (JWT, OAuth2, mTLS — never bespoke or home-grown).
  `SPEC-0014` states this as a requirement on the
  integration.
- The concrete standard is chosen **entirely by the integrating developer**,
  based on what their backend already uses.
- **The accepted residual: hoyadb can neither verify nor detect a developer
  shipping sync traffic over an unprotected channel.** There is no control
  it can insert.

## Traceability

- Parent: `ARC-0001`.
- Decision: `ADR-0019`.
- Related requirement (the boundary, not a mitigation): `REQ-0028`.
- Verified by: `TC-0012` verifies hoyadb itself opens no connection — the
  property that makes this threat structurally out of hoyadb's reach. The
  transport's own protection is outside hoyadb and untestable from here.
