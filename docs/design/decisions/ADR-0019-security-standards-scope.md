---
id: ADR-0019
title: Transport and auth standards are out of hoyadb's scope; at-rest encryption is engine-native
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0006]
tags: [security, encryption, transport, authentication]
---

# ADR-0019: Transport and auth standards are out of hoyadb's scope; at-rest encryption is engine-native

Source: `legacy/PLAN_DECISIONS.md` D11, folded into `legacy/VISION.md`'s "Security" and
`legacy/docs/07-security/security-model.md`.

## Context and problem statement

`legacy/VISION.md` fixes the *bar* — state-of-the-art, never bespoke, checkable
rather than merely claimed — but not the specific standards. D11 asked for
three: a minimum TLS version, a constraint on acceptable auth-token
standards, and an at-rest encryption algorithm/library.

## Considered options

- **Split the answer by who owns the wire**: transport TLS and auth-token
  standard are out of scope because hoyadb makes no network call; at-rest
  encryption stays hoyadb's, delegated to each concrete engine's own
  encrypted-storage capability.
- **Pin concrete standards for all three** — a minimum TLS version, a fixed
  auth standard, and one cipher shared across every platform.
- **One engine-agnostic at-rest cipher** implemented above the storage
  interface rather than inside each engine adapter.

## Decision outcome

In the context of naming hoyadb's concrete security standards, facing the
fact that hoyadb opens no network connection of its own, we decided for
**splitting the answer by ownership — transport and auth out of scope,
at-rest encryption engine-native per platform** and against pinning
concrete standards for all three and against one shared engine-agnostic
cipher, to achieve commitments hoyadb can actually enforce rather than
unverifiable claims about a wire it never touches, accepting that the
specific cipher and library per engine is deferred to implementation time.

### Consequences

**Good**
- hoyadb only promises what it can check. It has no transport of its own to
  pin a TLS minimum or auth standard on, so claiming one would be
  unenforceable.
- The bar is still fixed even where the pick is not hoyadb's: transport
  must be encrypted, and the authentication mechanism must be an
  established, vetted standard (JWT, OAuth2, mTLS — never home-grown).
  Which one is entirely the developer's choice, based on what their backend
  already uses.
- Engine-native encryption fits `ADR-0003`'s design: the generic interface
  contract does not grow encryption-specific methods; each concrete engine
  adapter handles it internally, invisibly above the interface.
- This is also where the identity boundary lands: hoyadb carries an
  already-authenticated identity faithfully and makes it client-unforgeable
  once set, but deciding whether to trust it on arrival is the backend's —
  and the backend is never hoyadb's code (`REQ-0015`).

**Bad**
- **The concrete cipher/library per engine is deliberately deferred to
  implementation time** — a named placeholder (an equivalent of SQLCipher
  for the SQLite-family default), not a pick. This is the one item
  `legacy/VISION.md`'s "Deferred to implementation time" section carries. The
  floor is fixed now; the pick is not.
- Transport security being out of scope means hoyadb cannot detect or
  prevent a developer shipping sync traffic over an unencrypted channel —
  recorded as `THR-0005` at `accepted-risk`.
- Different platforms using different engines means different ciphers, so
  "hoyadb encrypts at rest" is not one uniform guarantee across platforms.

### Confirmation

Confirmed by `SPEC-0014` (the security model) and `SPEC-0002` (the default
engine providing the capability), and by the at-rest encryption test cases
in `TP-0001`. The deferred cipher pick has nothing to confirm yet, by
decision.

## Traceability

- Parent: `OBJ-0006`.
- Referenced by: `SPEC-0014`, `SPEC-0002`, `SPEC-0001`.
- Accepted risk recorded as: `THR-0005`.
- Key lifecycle decided separately in `ADR-0020`.
