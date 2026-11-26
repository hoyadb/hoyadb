---
id: REQ-0030
title: The local store is encrypted at rest using the engine's own capability and a developer-supplied key
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0006]
mitigates: [THR-0002]
tags: [security, encryption, storage]
---

# REQ-0030: The local store is encrypted at rest using the engine's own capability and a developer-supplied key

## Pattern

Optional feature (platform-conditional), with an Unwanted-behaviour clause
against key management.

## Statement

WHERE the platform's concrete storage engine provides an encrypted-storage
capability, THE system SHALL encrypt the local store at rest using that
engine's own capability, handled inside the concrete engine adapter and
invisible to every layer above the storage interface (`SPEC-0001`).

THE system SHALL accept the encryption key material through a configuration
entry supplied by the application.

THE system SHALL NOT generate, store, derive, or manage the encryption key,
and SHALL NOT define its lifecycle across app reinstall, logout, or
multiple devices.

THE system SHALL use a vetted, authenticated encryption scheme from an
established library, and SHALL NOT use a bespoke or hand-rolled scheme.

## Examples

Example 1 — supplying the key
(`legacy/docs/07-security/security-model.md`):

```typescript
const db = await openDatabase({
  filename: "app.hoyadb",
  encryptionKey: await platformKeychain.getOrCreateKey("hoyadb-key"),
});
```

Result: hoyadb encrypts using the engine's capability. Where the key lives —
here a platform keychain — is entirely the developer's choice and code.

Example 2 — what this protects, stated precisely: the raw local file cannot
be read outside the app's own runtime — a stolen or unlocked device, an
extracted device backup, another process reading the file on a
rooted/jailbroken device. It does **not** protect against a compromised
running app, because the key must be reachable on-device for offline
operation to work at all. An accepted limitation, not a gap to close
(`THR-0002`).

## Open

**The concrete cipher and library per engine is deliberately deferred to
implementation time** — a named placeholder (an equivalent of SQLCipher for
the SQLite-family default engine), not a pick. The floor is fixed now; the
pick is not. This is the one item `legacy/VISION.md`'s "Deferred to implementation
time" section carries (`ADR-0019`).

**Key rotation on an already-encrypted local store is not covered anywhere**
in the doc tree — only initial key supply is designed (`ADR-0020`).

Because the engine differs per platform, "hoyadb encrypts at rest" is not
one uniform guarantee across platforms.

## Traceability

- Parent: `OBJ-0006`.
- Under: `ADR-0019`, `ADR-0020`.
- Satisfied by: `SPEC-0014`, `SPEC-0002`.
- Verified by: `TC-0015`.
