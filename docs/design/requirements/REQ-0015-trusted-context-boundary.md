---
id: REQ-0015
title: hoyadb carries an already-authenticated identity client-unforgeably and implements no authentication
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0006]
mitigates: [THR-0001, THR-0008]
tags: [security, identity, sync]
---

# REQ-0015: hoyadb carries an already-authenticated identity client-unforgeably and implements no authentication

## Pattern

Ubiquitous, with an Unwanted-behaviour clause against a handler reassigning
identity.

## Statement

THE system SHALL obtain a trusted context from a developer-supplied
extension point before sending any command or entity-diff write, SHALL
treat its value as opaque, and SHALL forward it unmodified in the
envelope's context.

IF a command handler attempts to assign or overwrite the trusted context,
THEN THE system SHALL reject that assignment — a handler reads the identity
and never sets it.

THE system SHALL NOT implement, obtain, validate or inspect the identity
itself, and SHALL NOT mandate any particular authentication scheme.

THE system SHALL treat the trusted context as identifying *who*, not *which
device or session* (`REQ-0025`).

## Examples

Example 1 — the extension point
(`legacy/docs/06-sync/command-protocol.md`):

```typescript
configure({
  getTrustedContext: async () => ({
    identity: await myAppsOwnAuth.getVerifiedIdentityToken(),
  }),
});
```

Result: whatever this returns becomes `ctx.identity` inside a handler and
`context.identity` in the envelope. hoyadb never inspects or validates it.

Example 2 — client-unforgeability is narrow, and stated as such: a handler
body containing `ctx.identity = "someone-else"` is rejected. This says
nothing about what the backend should do with the received value.
`legacy/docs/06-sync/implementation-guide.md` is explicit that a backend must
**re-derive** the real identity from the transport's own auth — in the HTTP
reference transport, the `Authorization` header — and use `context.identity`
only for logging or cross-checking, never as the authorization source.
That backend is never hoyadb's code.

## Open

Because the backend is the developer's own code, hoyadb cannot enforce that
re-derivation happens (`THR-0008`). Where an integration needs to
distinguish devices, that is a second field on this same extension point
(e.g. `{ identity, deviceId }`) — an extension of this design, not a
feature hoyadb implements (`ADR-0018`).

## Traceability

- Parent: `OBJ-0006`.
- Under: `ADR-0019`, `ADR-0018`.
- Satisfied by: `SPEC-0008`, `SPEC-0014`, `DM-0002`.
- Verified by: `TC-0012`.
