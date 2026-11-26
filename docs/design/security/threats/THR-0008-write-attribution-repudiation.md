---
id: THR-0008
title: A write cannot be reliably attributed, because hoyadb only carries identity and never verifies it
type: threat
status: mitigation-planned
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001]
tags: [security, stride, repudiation]
---

# THR-0008: A write cannot be reliably attributed, because hoyadb only carries identity and never verifies it

## Description

The identity attached to a command or entity-diff write is an opaque value
hoyadb forwards without inspecting or validating (`REQ-0015`). If a backend
records that value as the author of a write without independently
re-deriving it, the resulting audit trail is only as trustworthy as the
client that supplied it — and a user can plausibly deny a write recorded
against them.

## STRIDE grid

| Category | Applies? | Notes |
| --- | --- | --- |
| Spoofing | Partial | The enabling condition overlaps `THR-0001`; this entry is about the *attribution* consequence. |
| Tampering | No | Not the primary concern here. |
| Repudiation | Yes | The primary impact — a write's author cannot be proven from what hoyadb carries. |
| Information Disclosure | No | Not relevant. |
| Denial of Service | No | Not relevant. |
| Elevation of Privilege | No | Not relevant. |

## Target ASVS level

L2 (OWASP ASVS 5.0).

## Relevant ASVS items (referenced loosely)

- V7.1.x — verify that security-relevant events are logged with enough
  context to identify the actor.
- V4.1.x — verify that authorization and attribution decisions are made on
  a trusted layer, not from client-supplied data.

## Attack scenario

1. A backend implementer reads `context.identity` from the envelope and
   stores it as the write's author, because it is conveniently present.
2. A client — compromised, tampered, or simply buggy — supplies a different
   value (`THR-0001`).
3. The audit record names an identity that never made the write, and the
   real author is unattributable. Conversely, a user can repudiate a write
   genuinely recorded against them, since the value was never verified.

## Mitigation

- `REQ-0015` — hoyadb makes the trusted context client-unforgeable within
  its own plumbing (a handler reads `ctx.identity` and cannot assign it),
  and carries it faithfully and unmodified.
- `REQ-0015` carries the decisive control into the integration contract:
  the backend MUST re-derive the real identity from the transport's own
  authentication and use `context.identity` only for logging or
  cross-checking — **never** as the authorization or attribution source.
  `legacy/docs/06-sync/implementation-guide.md`'s step 1 states this directly:
  "Don't trust `context.identity` in the request body as the source of
  truth for *who this request is from*."
- `REQ-0013` — the idempotency key gives each command a stable, client-side
  identifier that a backend can record alongside the re-derived identity,
  so a specific write is referenceable in an audit trail.
- Implemented by: `SPEC-0008`, `SPEC-0014`.

## Why this is `mitigation-planned` and not `mitigated`

The control that actually closes this — the backend's independent identity
re-derivation — runs in **the developer's own code, which is never
hoyadb's**. hoyadb documents the requirement and can verify only its own
half (that the value is carried faithfully and cannot be reassigned by a
handler). It cannot enforce or detect correct backend behaviour, for the
same structural reason as `THR-0005`: hoyadb owns no wire and no
server-side component (`ADR-0019`).

Marking this `mitigated` would credit hoyadb with a control it does not
run. It stays at `mitigation-planned`, with the client-side half verified.

## Traceability

- Parent: `ARC-0001`.
- Mitigated by: `REQ-0015`, `REQ-0013`.
- Verified by: `TC-0012` (client-side half only).
