---
id: ADR-0013
title: Every envelope declares the client schema version; command names carry a version suffix
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, versioning, schema]
---

# ADR-0013: Every envelope declares the client schema version; command names carry a version suffix

## Context and problem statement

Narrower than `ADR-0009`: this is about a sync round-trip that straddles a
migration. Without a version on the wire, an old client and a newer backend
can silently disagree about what a command's arguments or a schema column
mean.

## Considered options

- **Declare the client's schema version in every command/entity-diff
  envelope, and version command names explicitly** (`add-order-v1`,
  `add-order-v2`).
- **Schema version only**, with command names left unversioned.
- **Negotiate a version once per session** rather than per envelope.
- **Nothing on the wire** — let the backend infer compatibility.

## Decision outcome

In the context of a sync round-trip that straddles a schema migration,
facing the risk of an old client and a newer backend silently
desynchronizing, we decided for **a schema version on every envelope plus
an explicit version suffix in command names** and against schema-version-only,
session-level negotiation, and nothing at all, to achieve two independent
version axes — the schema's and the command shape's — visible on every
single write, accepting that what a backend *does* on seeing a mismatch is
deliberately not fixed by hoyadb.

### Consequences

**Good**
- Two independent axes, correctly separated: a command's shape can change
  without a schema change, and vice versa.
- Per-envelope rather than per-session means a long-lived client that
  migrates mid-session cannot send a stale version claim.
- ADR-0013's own framing — "the goal is to avoid code desynchronisation" — is
  served by the naming convention being visible in the command registry on
  both sides.

**Bad**
- **What the server does on a mismatch (reject, translate, accept-as-is) is
  explicitly not fixed by hoyadb** — that policy belongs to the backend's
  own implementation, the same way ack policy does (`ADR-0011`).
- `SPEC-0008` names the concrete case this leaves
  unaddressed: a staged app-store rollout means a backend serves clients on
  schema version *n* and *n-1* simultaneously for days or weeks, not a
  clean cutover. Near-universal in practice, and still on the backend
  implementer to handle.
- Versioned command names mean an old command handler must be kept
  registered on the backend for as long as any client might still send it;
  hoyadb does not track or enforce that retention.

### Confirmation

Confirmed by `DM-0002` and `DM-0003` (both envelopes carrying
`schemaVersion`), `SPEC-0008`, `API-0001`, and the schema-version test
cases in `TP-0001`.

## Traceability

- Parent: `OBJ-0003`.
- Referenced by: `SPEC-0008`, `SPEC-0010`, `DM-0002`, `DM-0003`, `API-0001`.
- Depends on: `ADR-0009` (the migration this version tracks).
