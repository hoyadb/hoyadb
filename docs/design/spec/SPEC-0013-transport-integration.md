---
id: SPEC-0013
title: Transport integration — the developer-supplied push/pull object
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-24
parents: [ARC-0001, ADR-0026, ADR-0019]
satisfies: [REQ-0028, REQ-0029, REQ-0034]
depends_on: [SPEC-0009, DM-0002, DM-0003]
tags: [sync, transport, integration]
---

# SPEC-0013: Transport integration — the developer-supplied push/pull object

## Current implemented seam — 2026-10-24

API-0006/API-0008/API-0009 refine the illustrative TypeScript interface below.
The Rust core stays passive: the host obtains owned push/pull work and performs
network/timer operations without holding a storage borrow. Receipt and outcome
recovery are separate. Entity responses use table/recordId/writeId correlation;
pull returns complete bounded pages with explicit completion, comparison clocks
and an owned request token. Busy retains bytes/token; StalePull requires a fresh
fetch. The actual Swift/JS fixtures use the lossless API-0009 representation.
This resolves the original receipt-window and bootstrap gaps, without claiming
an automatic core scheduler or a production TypeScript SDK.

Source: `legacy/docs/06-sync/implementation-guide.md` (client-side half).

## Purpose

Specify the seam where hoyadb stops and the developer's own network code
starts. **hoyadb never opens a network connection** (`REQ-0028`); it calls
into an object the developer provides at database-open time.

## The interface

```typescript
interface Transport {
  push(envelopes: object[]): Promise<WireOutcome[]>;
  pull(since: string | null): Promise<{ checkpoint: string; changes: unknown[] }>;
}
```

`push` returns `WireOutcome[]` — the correlation-bearing wire shape — **not**
`CommandOutcome[]`. hoyadb's core strips the correlation field and maps each
down before the application sees an outcome (`SPEC-0008` §4).

The real interface only needs to forward whatever envelope it is given and
return whatever comes back: dispatch, business logic and outcome mapping all
happen server-side, not in the transport.

## When hoyadb calls it

- **`push`** — whenever the offline queue holds one or more writes eligible
  to send (an initial send, or a retry), passing them **in the same local
  execution order** `SPEC-0008` §3 guarantees.
- **`pull`** — on hoyadb's own sync schedule, or when the developer triggers
  it.

**There is no separate decision point where hoyadb chooses to batch.** `push`
is the one method, always, whether there is one queued write or several — a
single write is an array of length one. `ADR-0026`'s "batching is the
default" answer is reflected directly in the interface shape: the core has
nothing to decide and an implementation has nothing to switch between.

## Reference implementation

```typescript
function createHttpTransport(baseUrl: string, getAuthToken: () => Promise<string>): Transport {
  const headers = async () => ({
    "Content-Type": "application/json",
    Authorization: `Bearer ${await getAuthToken()}`,
  });
  return {
    async push(envelopes) {
      const res = await fetch(`${baseUrl}/hoyadb/commands`, {
        method: "POST", headers: await headers(), body: JSON.stringify(envelopes),
      });
      return res.json();
    },
    async pull(since) {
      const res = await fetch(`${baseUrl}/hoyadb/changes?since=${since ?? ""}`, {
        headers: await headers(),
      });
      return res.json();
    },
  };
}
```

The `fetch` call, the URL, the TLS configuration and the token are **all the
developer's code**. hoyadb sees none of them — which is the structural
reason `THR-0005` sits at `accepted-risk`.

## Opting into one request per write

Done **inside** a developer's own `push`, not through a different method or
URL: the same endpoint, called once per envelope with a length-one array.

**Critically, sequentially — never with `Promise.all`.** `envelopes` arrives
in the local execution order, and a dependent write can only resolve
correctly if the backend sees what it depends on first. Firing split
requests concurrently races them and silently breaks that ordering. This
opt-out changes request *count*, not order.

## What hoyadb attaches automatically

A `Transport` never populates these; it forwards envelopes already carrying
them (`DM-0002`): the idempotency key, `schemaVersion`, `context.identity`,
and `context.capturedAt` (whatever `ctx.now()` returned during the local
run).

## Test double

`createFakeTransport` implements this same interface in-process with no HTTP
server (`REQ-0034`), mapping each envelope through per-command handlers and
**attaching that envelope's `idempotencyKey` (or `recordId`) to the outcome
itself** — because correlation by array position is not part of the real
contract either, so the fake does not rely on it. Returning a different
`status` simulates a rejection or correction; having `push` reject or delay
simulates a dropped ack or slow network.

## Open

- **Proving this contract works unmodified over a second transport is
  unstarted.** `legacy/docs/06-sync/transport-http-example.md` retains its ⚠ for
  exactly this: `legacy/VISION.md`'s claim that the contract is "usable over more
  than one transport" is **asserted, not verified**, and one worked HTTP
  example does not close that. This names real remaining work, not a
  decision someone can just answer.
- **The ack/outcome visibility window** for a transport that delivers them
  separately is not designed for (`SPEC-0008` §4).
- The package boundary for the test double is implementation-time detail.

## Traceability

- Parents: `ARC-0001`, `ADR-0026`, `ADR-0019`.
- Satisfies: `REQ-0028`, `REQ-0029`, `REQ-0034`.
- Depends on: `SPEC-0009`, `DM-0002`, `DM-0003`.
- Realized as a concrete contract by: `API-0001`.
- Verified by: `TC-0012`, `TC-0014`, `TC-0016`.
