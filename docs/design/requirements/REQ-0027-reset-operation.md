---
id: REQ-0027
title: hoyadb exposes an explicit local reset; it never purges revoked data on its own
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0006]
tags: [sync, scoping, membership, security]
---

# REQ-0027: hoyadb exposes an explicit local reset; it never purges revoked data on its own

## Pattern

Optional feature, with an Unwanted-behaviour clause against automatic
purging.

## Statement

WHERE the application invokes the reset operation, THE system SHALL rebuild
the local store — dropping and recreating it, or opening a different local
store — so that previously-synced data is no longer held.

IF a client's server-computed membership narrows, THEN THE system SHALL NOT
detect that change on its own, SHALL NOT selectively purge the
now-out-of-scope records, and SHALL NOT invoke the reset itself. Detecting
the change and invoking the reset is the integrating developer's
responsibility.

THE system SHALL NOT inspect a queued write's data against current
membership before sending it — a queued write is sent normally, and the
backend's own authorization check rejects what the revoked identity may no
longer write (`REQ-0017`).

## Examples

Example 1 — developer-invoked reset
(`legacy/docs/06-sync/scoping-topology.md`):

```typescript
async function onMembershipRevoked() {
  await db.reset(); // drops and recreates the local store
  // or: await db.reopen({ filename: "workspace-b.hoyadb" });
}
```

Result: locally-held data for the prior membership is gone. hoyadb ran this
because the developer called it, not because it inferred anything.

Example 2 — the developer does not call it: previously-synced data simply
persists on the device. hoyadb does not police its own data lifecycle —
it is a library, not an application that manages this on the developer's
behalf.

Example 3 — queued writes against revoked data: a write queued offline
against an org the client has since been removed from is still transmitted
on reconnect, and is rejected by the backend's authorization check,
surfacing through the ordinary rejection path.

## Open

**This is a deliberate decision not to mitigate a real confidentiality
exposure inside hoyadb** — recorded as `THR-0004` at `accepted-risk`, not
`mitigated`, which is the honest status. What happens to still-queued
unacknowledged writes at the moment of a reset is **unaddressed**, the same
gap `REQ-0006` records for a migration reset.

## Traceability

- Parent: `OBJ-0006`.
- Under: `ADR-0015`.
- Satisfied by: `SPEC-0012`.
- Verified by: `TC-0013`.
