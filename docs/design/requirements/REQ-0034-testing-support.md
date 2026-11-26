---
id: REQ-0034
title: An in-memory engine, a fake transport, and injectable clock/id make app tests hermetic
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001]
tags: [testing, storage, sync]
---

# REQ-0034: An in-memory engine, a fake transport, and injectable clock/id make app tests hermetic

## Pattern

Optional feature (three instances).

## Statement

WHERE an application opens hoyadb against the in-memory storage engine, THE
system SHALL provide the same schema, query, subscription and command
behaviour it provides against an on-disk engine, without touching the
filesystem or a platform storage API.

WHERE an application supplies a test transport implementing the same
transport contract (`REQ-0028`), THE system SHALL drive it exactly as it
drives a real one, so command outcomes, acknowledgement and retry can be
exercised without a network or backend.

WHERE an application supplies a clock function or an id generator, THE
system SHALL use them in place of the real clock and real id generation.

## Examples

Example 1 — hermetic open (`legacy/docs/09-testing.md`):

```typescript
const db = await openDatabase({ schema, engine: "memory" });
```

Result: because only the storage interface changes, everything above it —
schema, relations, subscriptions, command handlers — behaves identically to
a real deployment. A test written this way exercises real hoyadb behaviour,
not a simplified stand-in.

Example 2 — driving a specific backend outcome:

```typescript
const transport = createFakeTransport({
  handlers: {
    "add-order-v1": (envelope) => ({
      status: "applied",
      result: { id: "order_1", total_cents: 4200 },
    }),
  },
});
const db = await openDatabase({ schema, engine: "memory", transport });
```

Result: the fake attaches each envelope's `idempotencyKey` to the outcome
itself, exactly as a real backend's response does, so tests never rely on
array-position correlation (`REQ-0029`). Returning `rejected` from a handler
simulates a rejection; having `push` reject or delay simulates a dropped
ack or a slow network.

Example 3 — deterministic values:

```typescript
const db = await openDatabase({
  schema, engine: "memory",
  clock: () => new Date("2026-09-15T12:00:00Z"),
  idGenerator: () => "test-fixed-id-001",
});
```

Result: a test can assert a command's exact local result — `placed_at`
equal to a known timestamp — without flaking on real time.

## Open

The package boundary (whether the fake transport and clock/id injection
ship in the core client package or a separate testing package) is
implementation-time detail, not fixed.

Two disclosed gaps in `legacy/docs/09-testing.md`, **neither designed**:

- **No client/server handler-drift testing story**, despite determinism
  being named the project's riskiest assumption (`ADR-0014`, `RISK-0002`).
- **No storage-engine conformance suite**, so "a new engine actually
  satisfies the interface" is unverifiable (`ADR-0003`).

## Traceability

- Parent: `OBJ-0001`.
- Under: `ADR-0003`, `ADR-0005`.
- Satisfied by: `SPEC-0001`, `SPEC-0013`.
- Verified by: `TC-0016`.
