---
id: ADR-0020
title: The developer supplies at-rest key material; hoyadb never generates, stores or manages it
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0006]
tags: [security, encryption, key-management]
---

# ADR-0020: The developer supplies at-rest key material; hoyadb never generates, stores or manages it

## Context and problem statement

`ADR-0019` settles which cipher performs at-rest encryption but not the
key's lifecycle — where it comes from, where it lives, and what happens
across app reinstall, logout, and multiple devices on one account.
"Pick a library" does not answer any of that.

## Considered options

- **The developer supplies key material through a configuration entry**;
  hoyadb neither generates, stores, nor manages the key or its lifecycle.
- **hoyadb generates and stores the key** in the platform keychain/keystore
  on the developer's behalf.
- **Derive the key** from the authenticated identity or a developer-supplied
  passphrase inside hoyadb.

## Decision outcome

In the context of at-rest encryption key lifecycle, facing platform-specific
key storage hoyadb would have to abstract over, we decided for **the
developer supplying key material through a configuration entry** and
against hoyadb generating and storing the key and against deriving it
internally, to achieve one consistent boundary with the rest of the
security surface hoyadb does not own, accepting that hoyadb can make no
statement about where the key actually lives or how it survives a
reinstall.

### Consequences

**Good**
- Consistent with `ADR-0019` and `OBJ-0006`: hoyadb owns the cipher, not
  the secret. Where the key lives — a platform keychain/keystore, or
  anything else — is the developer's call, matching their platform.
- Reinstall, logout and multi-device key lifecycle follow whatever the
  developer's own key-supply mechanism does. hoyadb does not define a
  lifecycle it does not own.

**Bad**
- **The guarantee is real but narrow, and stated as such.** It protects the
  raw local file from being read outside the app's own runtime — a stolen
  or unlocked device, an extracted device backup, another process reading
  the file on a rooted/jailbroken device. It does **not** protect against a
  compromised running app, because the key must be reachable on-device for
  offline operation to work at all. An accepted limitation, not a gap to
  close — the same trade-off every on-device at-rest scheme with offline
  requirements makes (`THR-0002`).
- hoyadb cannot detect a developer supplying a weak or hard-coded key.
- **Key rotation on an already-encrypted local store is not covered
  anywhere** — `NFR-0005` lists it among
  four unaddressed operational questions. Only initial key supply is
  designed. A real, disclosed gap.

### Confirmation

Confirmed by `SPEC-0014`'s key-management section and the encryption test
cases in `TP-0001`, which verify the configuration entry exists and that
hoyadb generates no key of its own.

## Traceability

- Parent: `OBJ-0006`.
- Depends on: `ADR-0019`.
- Referenced by: `SPEC-0014`.
- Threat this partially mitigates: `THR-0002`.
