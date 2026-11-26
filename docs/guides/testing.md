# Testing

> For the implemented JavaScript application API and executed examples, build the [public HTML documentation](../public/README.md) and start with its Getting started page. Illustrative syntax below describes design intent; consult the SDK contracts for concrete APIs.

> Supporting guide. Current design authority: [Testing support](../design/requirements/REQ-0034-testing-support.md); [Test plan](../design/tests/plan/TP-0001-test-plan.md).
> Code and package names are illustrative design examples; no SDK implementation
> is supplied by this guide. Current API contracts define the supported SDK surface.

How an app developer tests code built against hoyadb: in-memory storage,
a fake transport, and deterministic clock/id injection. The examples below
illustrate the testing support specified by the linked formal artifacts.

## Non-goals

Carried forward from [VISION.md](../design/vision/VIS-0001-hoyadb.md)'s "What we deliberately
avoid", applicable to this doc:

- **hoyadb making network calls on the integrating app's behalf.** The
  fake transport below is a test double for the developer's own network
  code, not hoyadb performing calls itself even in tests.

## In-memory storage for hermetic tests

[04-storage/storage-interface.md](../design/spec/SPEC-0001-storage-interface.md)
requires every engine implementation to include an in-memory/fake
implementation usable for tests. An app's tests open hoyadb against this
in-memory engine instead of a real on-disk one — same schema, same query/
[command](glossary.md#command)/[subscription](glossary.md#subscription) behavior, no filesystem or platform storage API
involved:

```typescript
import { openDatabase } from "@hoyadb/client";
import { schema } from "./schema";

const db = await openDatabase({ schema, engine: "memory" });
```

Because the [storage interface](../design/spec/SPEC-0001-storage-interface.md) is the
only thing that changes, everything above it — schema, relations, query
subscriptions, [command handlers](glossary.md#command-handler) running
against the local store — behaves identically to a real deployment; a test
written against the in-memory
engine is testing real hoyadb behavior, not a simplified stand-in for it.

## A fake transport for sync tests

[06-sync/transport-http-example.md](../design/contracts/API-0001-http-sync-transport.md)'s
contract (the command/[entity-diff](glossary.md#entity-diff-fallback-path) envelope shapes and the
`WireOutcome` shape from
[command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#4-reliability))
is transport-agnostic by design — a test-only fake transport implements
the same contract in-process, without an HTTP server, letting tests drive
specific backend responses deterministically:

```typescript
import { openDatabase } from "@hoyadb/client";
import { createFakeTransport } from "@hoyadb/testing";

const transport = createFakeTransport({
  handlers: {
    // A handler returns just the outcome's business fields — no
    // idempotencyKey/recordId. createFakeTransport's own dispatch logic
    // attaches the correlation field from each envelope automatically
    // (see below), the same way a real backend's response does; a
    // handler author never has to remember to do this themselves.
    "add-order-v1": (envelope) => ({
      status: "applied",
      result: { id: "order_1", total_cents: 4200 },
    }),
    // An entity-diff write (no `command` field) falls back to a default
    // `{ status: "applied" }` unless a `handlers.$entityDiff` override is
    // given — this fake's own dispatch convention, not part of the
    // `Transport` interface itself.
  },
});

const db = await openDatabase({ schema, engine: "memory", transport });

const outcome = await addOrderV1.execute({ customerId: "c1", items: [] });
// outcome resolves against the fake handler above, exercising the same
// optimistic-apply/ack/CommandOutcome path a real backend would drive
```

`createFakeTransport` implements the same `Transport.push(envelopes):
Promise<WireOutcome[]>` interface real transports do (see
[implementation-guide.md](sync-implementation.md#client-side-implementing-a-real-transport)) —
internally, it maps each envelope in the array through `handlers` by
`command` name, takes each handler's return value, and attaches that
envelope's `idempotencyKey` (or `recordId` for an entity-diff) before
including it in the result array — correlation by array position alone
isn't part of the real contract either (see
[transport-http-example.md](../design/contracts/API-0001-http-sync-transport.md#post-hoyadbcommands)),
so the fake doesn't rely on it. A rejection or correction is simulated by
returning that `status` from a handler function instead of `applied`; a
dropped ack or a delayed response is simulated by having `push` itself
reject or delay before resolving, same as a real network failure would
look to the caller — giving a test control over exactly the sync
scenarios [command-protocol.md](../design/spec/SPEC-0008-command-protocol.md) and
[offline-queue.md](../design/spec/SPEC-0009-offline-queue.md) specify, without a real
network or backend involved.

## Deterministic clock/id injection

[command-protocol.md](../design/spec/SPEC-0008-command-protocol.md)'s determinism section
names non-deterministic operations (wall-clock reads, random values)
inside a [command handler](glossary.md#command-handler) as the developer's own responsibility to keep
consistent between client and server runs — see `ctx.now()` in that doc's
example. Testing a handler that uses `ctx.now()` or a generated id (an
[idempotency key](glossary.md#idempotency-key), or a local record id)
requires those values to be injectable and deterministic in a test, not
sourced from the real clock or a real random generator:

```typescript
import { openDatabase } from "@hoyadb/client";

const db = await openDatabase({
  schema,
  engine: "memory",
  clock: () => new Date("2026-09-15T12:00:00Z"),
  idGenerator: () => "test-fixed-id-001",
});
```

This is what lets a test assert on a command's exact local result (e.g.
`placed_at` equals a known timestamp) without either flaking on real time
or needing to normalize timestamps after the fact.

## What's still open

The exact package boundary (whether the fake transport and deterministic
clock/id injection ship in the core `@hoyadb/client` package or a separate
`@hoyadb/testing` package, as sketched above) is implementation-time
detail, not fixed by this doc.

**No client/server handler-drift testing story**, despite
[VISION.md](../design/vision/VIS-0001-hoyadb.md) naming [command](glossary.md#command)
determinism the single riskiest assumption in the whole project (see
[command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#6-determinism--a-non-goal)'s
determinism section): nothing here addresses contract/golden testing
between a client handler and its independently-authored server
counterpart, e.g. replaying the optional debug execution-log from
`command-protocol.md` against the server handler to catch drift. This doc
is scoped to "how an app developer tests code built against hoyadb" — a
defensible reason this might stay out of scope — but given the risk
severity VISION.md itself assigns, it's worth naming rather than leaving
silent.

**No storage-engine conformance suite.** [storage-interface.md](../design/spec/SPEC-0001-storage-interface.md)
names "behavior can leak across engines" as a real, unsolved risk when a
new concrete engine implements the interface — this doc's testing story
is entirely about app-level tests against the in-memory engine, not about
verifying a *new engine implementation* actually satisfies the interface
contract. Also disclosed, not designed.
