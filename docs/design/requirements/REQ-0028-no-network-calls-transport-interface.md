---
id: REQ-0028
title: hoyadb opens no network connection; it calls a developer-supplied transport
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0006]
mitigates: [THR-0007]
tags: [sync, transport, library-boundary]
---

# REQ-0028: hoyadb opens no network connection; it calls a developer-supplied transport

## Pattern

Ubiquitous, with an Event-driven clause for invocation.

## Statement

THE system SHALL NOT open a network connection, perform a network request,
or send telemetry of its own, under any circumstance.

THE system SHALL accept a transport object supplied by the application at
database-open time, exposing exactly two operations: pushing an array of
write envelopes and returning their per-write outcomes, and pulling changes
since a checkpoint.

WHEN the offline queue holds one or more writes eligible to send, THE
system SHALL invoke the transport's push operation with those envelopes in
their local execution order (`REQ-0011`).

THE system SHALL leave the choice of protocol, transport security
configuration, and authentication mechanism entirely to the application.

## Examples

Example 1 — the interface:

```typescript
interface Transport {
  push(envelopes: object[]): Promise<WireOutcome[]>;
  pull(since: string | null): Promise<{ checkpoint: string; changes: unknown[] }>;
}
```

Result: hoyadb hands over envelopes and receives outcomes. Dispatch,
business logic and outcome mapping all happen server-side, in the
developer's own code — the transport only forwards.

Example 2 — supplied at open time:

```typescript
const db = await openDatabase({
  schema,
  transport: createHttpTransport("https://api.example.com", () =>
    myAppsOwnAuth.getVerifiedIdentityToken()),
});
```

Result: the `fetch` call inside `createHttpTransport` is the developer's
code, not hoyadb's. hoyadb never sees a URL, a TLS setting or a token.

Example 3 — a test fake: `createFakeTransport`
implements the same interface in-process with no HTTP server, which is what
makes sync behaviour testable hermetically (`REQ-0034`).

## Traceability

- Parent: `OBJ-0006`.
- Under: `ADR-0019`, `ADR-0026`.
- Satisfied by: `SPEC-0013`, `API-0001`.
- Verified by: `TC-0012`.
