---
id: REQ-0031
title: The local store is excluded from platform device and app backups by default
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0006]
mitigates: [THR-0002]
tags: [security, storage, privacy]
---

# REQ-0031: The local store is excluded from platform device and app backups by default

## Pattern

Ubiquitous default, with an Optional-feature clause for opting back in.

## Statement

THE system SHALL mark its local store as excluded from platform-level
device and app backup mechanisms by default, where the platform provides
such a mechanism.

WHERE the application explicitly opts back in, THE system SHALL allow the
local store to be included in platform backups.

## Examples

Example 1 — the default and the opt-in:

```typescript
// Default — excluded from platform backups:
configure({ storage: { excludeFromBackups: true } });

// Opt back in, where the platform supports it:
configure({ storage: { excludeFromBackups: false } });
```

Example 2 — why the default is exclusion: the local store can hold synced
data covered by the same membership and authorization rules as the backend
(`REQ-0026`). A platform backup is a channel those rules do not reach, so
data governed server-side could otherwise leak through it.

## Open

Exclusion depends on the platform providing such a mechanism; hoyadb makes
no claim for a platform that does not. This control is complementary to
`REQ-0030` — an extracted backup of an encrypted store is also protected by
encryption, but exclusion removes the exposure rather than relying on the
key staying secret.

## Traceability

- Parent: `OBJ-0006`.
- Under: `ADR-0019`.
- Satisfied by: `SPEC-0014`.
- Verified by: `TC-0015`.
