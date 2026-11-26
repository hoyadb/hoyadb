---
id: SPEC-0014
title: Security model — what hoyadb owns, and what it requires but does not implement
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001, ADR-0019, ADR-0020, ADR-0021]
satisfies: [REQ-0015, REQ-0030, REQ-0031, REQ-0032]
depends_on: [SPEC-0002, SPEC-0001]
tags: [security, encryption, privacy]
---

# SPEC-0014: Security model — what hoyadb owns, and what it requires but does not implement

Source: `legacy/docs/07-security/security-model.md`.

## Purpose

Draw the line precisely: hoyadb protects the local store it owns, and makes
no network call, implements no authentication, and emits no telemetry.

## The floor

Every security-relevant piece — whether hoyadb implements it or a developer
plugs it in — is held to current state-of-the-art practice, never a custom
or reduced scheme. **"State of the art" is a floor, not a slogan**: wherever
this design names a mechanism hoyadb actually owns, it must be checkable (a
specific cipher and library), not left as an unverifiable claim.

## Out of hoyadb's scope: transport and auth standards

hoyadb makes no network call itself (`SPEC-0013`) — it has **no wire of its
own** to pin a TLS minimum or an auth-token standard on. The bar is still
fixed even though the pick is not hoyadb's:

- Transport **must** be encrypted.
- The authentication mechanism **must** be an established, vetted standard
  (JWT, OAuth2, mTLS) — never bespoke or home-grown.

Which concrete standard is used is chosen **entirely by the integrating
developer**, based on what their backend already uses. hoyadb can neither
verify nor detect a violation (`THR-0005`, at `accepted-risk`).

## Owned by hoyadb: at-rest encryption

Unlike transport, hoyadb's own storage layer performs at-rest encryption, so
the algorithm is hoyadb's to pick — but **per platform/engine, not one
shared algorithm**. hoyadb relies on the concrete engine's own
encrypted-storage capability (an equivalent of SQLCipher for the
SQLite-family default, `SPEC-0002`); a platform on a different engine brings
its own equivalent.

This fits `SPEC-0001`'s design: that design's "cross-cutting concerns layer
on top" line is about the **generic interface contract** not needing
encryption-specific methods — not a requirement that the cipher itself be
engine-agnostic code. Each concrete engine adapter handles it internally,
invisible above the interface.

## Key management

```typescript
const db = await openDatabase({
  filename: "app.hoyadb",
  encryptionKey: await platformKeychain.getOrCreateKey("hoyadb-key"),
});
```

hoyadb takes key material via a configuration entry. **It does not generate,
store, or manage the key or its lifecycle** across reinstall, logout or
multi-device — that is entirely the developer's responsibility, based on
their platform.

**Threat model, stated explicitly**: this protects the raw local file from
being read outside the app's own runtime — a stolen or unlocked device, an
extracted device backup, another process reading the file on a
rooted/jailbroken device. It does **not** protect against a compromised
running app, since the key must be reachable on-device for offline operation
to work at all. An accepted limitation, not a gap left open — the same
trade-off every on-device at-rest scheme with offline requirements makes
(`THR-0002`).

## Backup exclusion

```typescript
configure({ storage: { excludeFromBackups: true } });  // default
configure({ storage: { excludeFromBackups: false } }); // opt back in
```

Default exclusion, because the local store can hold synced data covered by
the same membership rules as the backend (`SPEC-0012`) — and a platform
backup is a channel those rules do not reach.

## Logging and telemetry

hoyadb's core holds command and trusted-context contents **only within its
own local queue and state** — required for the offline queue (`SPEC-0009`)
and developer debugging visibility (`SPEC-0015`). **It never sends telemetry
externally**; hoyadb is a library, not a hosted service. A developer who
wants telemetry adds it themselves.

Redaction *inside* the queue was rejected: a redacted command cannot be
retried, and per-command status could not show what is pending
(`ADR-0021`).

## Identity boundary

Specified in full in `SPEC-0008` §7: hoyadb carries an already-authenticated,
opaque identity faithfully and makes it client-unforgeable within its own
plumbing, and **never** establishes, validates or inspects it. The backend
must independently re-derive identity from the transport's own auth
(`THR-0001`, `THR-0008`).

## Open

- **The concrete cipher and library per storage engine remains deliberately
  deferred to implementation time** — the one item `legacy/VISION.md`'s "Deferred
  to implementation time" section carries. The floor is fixed now; the pick
  is not. Until then, this design names a placeholder rather than a
  checkable mechanism, which is a known shortfall against its own "must be
  checkable" standard.
- **Key rotation on an already-encrypted local store is not covered
  anywhere** — only initial key supply is designed (`ADR-0020`).
- hoyadb cannot detect a developer supplying a weak or hard-coded key.
- **The diagnostic export named as useful in `SPEC-0015` has no redaction
  policy decided** — it would be a structured dump of exactly the content
  this design keeps local (`THR-0007`).

Nothing else in `legacy/docs/07-security/security-model.md` is open.

## Traceability

- Parents: `ARC-0001`, `ADR-0019`, `ADR-0020`, `ADR-0021`.
- Satisfies: `REQ-0015`, `REQ-0030`, `REQ-0031`, `REQ-0032`.
- Depends on: `SPEC-0002`, `SPEC-0001`.
- Verified by: `TC-0012`, `TC-0015`.
