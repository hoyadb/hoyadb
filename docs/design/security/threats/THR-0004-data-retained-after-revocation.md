---
id: THR-0004
title: Synced data stays on a device after the client's membership is revoked
type: threat
status: accepted-risk
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001]
tags: [security, stride, information-disclosure, accepted-risk]
---

# THR-0004: Synced data stays on a device after the client's membership is revoked

## Description

When a client's server-computed membership narrows — it is removed from an
org, or loses access to a workspace — the records it already synced remain
in its local store. hoyadb does not detect the change and does not purge
them.

## STRIDE grid

| Category | Applies? | Notes |
| --- | --- | --- |
| Spoofing | No | No identity is claimed. |
| Tampering | No | No data is modified. |
| Repudiation | No | Not relevant. |
| Information Disclosure | Yes | The primary impact — a former member retains data they are no longer entitled to hold. |
| Denial of Service | No | Not relevant. |
| Elevation of Privilege | No | No privilege boundary is crossed; the data was legitimately held when synced. |

## Target ASVS level

L2 (OWASP ASVS 5.0).

## Relevant ASVS items (referenced loosely)

- V8.1.x — verify that sensitive data is retained only as long as required.
- V6.1.x — verify that client-side stored sensitive data is removed when no
  longer required.

## Attack scenario

1. A user is removed from an organization server-side; their membership is
   recomputed to exclude that org's data.
2. Their device is offline, or the app simply never calls the reset
   operation.
3. Every record synced before revocation remains readable in the local
   store, subject only to `THR-0002`'s at-rest protection — which does not
   help against the former member themselves, who has the running app.

## Decision not to mitigate

**This is a deliberate, recorded decision, which is why it sits at
`accepted-risk` rather than `mitigated`.** `ADR-0015` (from D7) decides
that hoyadb exposes an explicit reset operation (`REQ-0027`) and does
**not**:

- detect the membership change (a developer-owned process),
- selectively purge the now-out-of-scope records, or
- invoke the reset itself.

D7's own framing: "If the developer doesn't reset it, we cannot manage.
Don't forget we are creating a library, not an application."

What hoyadb does provide, and what the accepted residual is:

- `REQ-0027` gives the developer a total, unambiguous reset — rebuild the
  local store or point at a fresh one — rather than a partial purge whose
  correctness hoyadb could not verify.
- `REQ-0027` also confirms that queued writes against revoked data are
  still transmitted and are rejected by the backend's own authorization
  check, so revocation is enforced *server-side* even when local data
  persists.
- **The residual, accepted in full: if the developer does not invoke the
  reset, the data simply stays.** hoyadb does not police this.

Because this is `accepted-risk`, `REQ-0027` deliberately does **not** carry
a `mitigates` link to this threat — it provides the tool, it does not close
the exposure.

## Traceability

- Parent: `ARC-0001`.
- Decision: `ADR-0015`.
- Related requirement (tool, not mitigation): `REQ-0027`.
- Verified by: `TC-0013` verifies the reset operation exists and is total —
  not that data is purged without it, since by decision it is not.
