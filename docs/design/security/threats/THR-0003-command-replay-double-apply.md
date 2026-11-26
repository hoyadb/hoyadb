---
id: THR-0003
title: A resent command is applied twice by the backend
type: threat
status: mitigation-planned
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001]
tags: [security, stride, tampering, integrity]
---

# THR-0003: A resent command is applied twice by the backend

## Description

hoyadb retries unacknowledged writes automatically (`REQ-0020`). A command
whose response was lost — not whose delivery failed — is resent even though
the backend already applied it. Without deduplication, an "add to basket"
or a payment-bearing command applies twice.

An attacker can also deliberately replay a captured envelope to the same
effect, which is why this is a security threat and not only a reliability
concern.

## STRIDE grid

| Category | Applies? | Notes |
| --- | --- | --- |
| Spoofing | No | The identity is genuine; the command is the replayed artifact. |
| Tampering | Yes | Server-side state is mutated more times than intended. |
| Repudiation | No | Covered by `THR-0008`. |
| Information Disclosure | No | Not relevant. |
| Denial of Service | Partial | Mass replay can amplify backend load. |
| Elevation of Privilege | No | No privilege boundary crossed. |

## Target ASVS level

L2 (OWASP ASVS 5.0).

## Relevant ASVS items (referenced loosely)

- V11.1.x — verify that business logic flows cannot be replayed or
  processed out of sequence.
- V13.2.x — verify that requests carrying state-changing operations are
  protected against replay.

## Attack scenario

1. A client sends `add-order-v1` with idempotency key `01J...Q3`. The
   backend applies it. The response is lost to a network failure.
2. The command remains `sent`-but-unacknowledged, so hoyadb retries it
   (`REQ-0020`), resending the identical envelope.
3. If the backend does not check the idempotency key, a second order is
   created.

## Mitigation

- `REQ-0013` — every command carries a client-generated, time-sortable
  idempotency key, and a retry resends the **same** key rather than minting
  a new one. This is what makes the duplicate *detectable*.
- `SPEC-0013` shows the backend-side control:
  the idempotency check, the handler's writes, and recording the outcome
  all commit in **one transaction** against an `idempotency_keys` table
  with a unique constraint — closing both the concurrent-double-check race
  and the crash-between-write-and-record window.
- Implemented by: `SPEC-0008`, `SPEC-0009`.

## Why this is `mitigation-planned` and not `mitigated`

**hoyadb provides the key but does not and cannot require any backend to
deduplicate against it.** `ADR-0010` records this explicitly, from ADR-0010's own
answer: whether to deduplicate is "the backend decide to implement it or
not." hoyadb has no server-side component to enforce the check in.

So the threat is reduced — a duplicate is now *detectable* by any backend
that chooses to look — but not closed by anything hoyadb itself runs.
Marking this `mitigated` would overstate what hoyadb delivers. It sits at
`mitigation-planned`: the mitigating requirement exists and is verified on
the client side (`TC-0009` proves the key is reused rather than
regenerated), while the server-side half remains the integrator's to
implement.

## Traceability

- Parent: `ARC-0001`.
- Mitigated by: `REQ-0013`.
- Verified by: `TC-0009` (client-side half only — the backend half is
  outside hoyadb).
