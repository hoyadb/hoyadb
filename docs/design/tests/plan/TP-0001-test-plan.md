---
id: TP-0001
title: hoyadb — test plan
type: test-plan
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-20
parents: [ARC-0001]
verifies: [REQ-0001, REQ-0002, REQ-0003, REQ-0004, REQ-0005, REQ-0006, REQ-0007, REQ-0008, REQ-0009, REQ-0010, REQ-0011, REQ-0012, REQ-0013, REQ-0014, REQ-0015, REQ-0016, REQ-0017, REQ-0018, REQ-0019, REQ-0020, REQ-0021, REQ-0022, REQ-0023, REQ-0024, REQ-0025, REQ-0026, REQ-0027, REQ-0028, REQ-0029, REQ-0030, REQ-0031, REQ-0032, REQ-0033, REQ-0034, NFR-0002, NFR-0003, NFR-0005, THR-0001, THR-0002, THR-0003, THR-0006, THR-0007, THR-0008]
tags: [test, plan]
---

# TP-0001: hoyadb — test plan

## Scope

Functional verification of every `accepted` REQ and NFR in this tree, and of
the mitigating controls behind every `mitigated` or `mitigation-planned`
threat.

**Explicitly out of scope, and why:**

- **`NFR-0001` (responsiveness) and `NFR-0004` (footprint)** carry no
  Scale/Meter/Target by decision (`ADR-0022`, `ADR-0024`). There is nothing
  measurable to verify, so they have no cases and appear at `draft` rather
  than as untested `accepted` artifacts. Performance/benchmark testing is
  sequenced after a baseline exists.
- **`THR-0004` and `THR-0005`** are `accepted-risk` — deliberate decisions
  not to mitigate (`ADR-0015`, `ADR-0019`). Cases verify the *tools and
  boundaries* hoyadb does provide (the reset operation exists and is total;
  hoyadb opens no connection), never that the accepted exposure is closed,
  because by decision it is not.
- **The server-side half of several controls** — idempotency deduplication
  (`THR-0003`), identity re-derivation (`THR-0008`), the identical-timestamp
  tie-break (`REQ-0025`) — runs in the developer's own backend, which is
  never hoyadb's code. Cases cover the client-side half only. This is
  stated per case, not silently narrowed.

## Test levels and techniques

All cases are API-level acceptance tests in Gherkin
(`tests/cases/*.feature`), written against hoyadb's public surface rather
than its internals, so they survive an implementation change.

## Environment and test data

Per `REQ-0034`:

- **In-memory storage engine** — hermetic, no filesystem or platform storage
  API. Because only the storage interface changes, everything above it
  behaves identically to a real deployment.
- **Fake transport** — implements the same `Transport` contract in-process
  (`SPEC-0013`), letting a case drive `applied` / `corrected` / `rejected`
  deterministically, and simulate a dropped ack by having `push` reject.
- **Injected clock and id generator** — so a case can assert an exact
  `placed_at` or a fixed local id without flaking on real time.
- **Test data** — the walkthrough's `customers` / `orders` / `order_items` /
  `products` / `order_notes` schema, used throughout so cases read against
  the same worked example as the rest of the tree.

## Cases

| Case | Verifies | Summary |
| --- | --- | --- |
| `TC-0001` | `REQ-0001`, `REQ-0002` | Schema declaration, implicit id, undeclared-table rejection, all three relation kinds |
| `TC-0002` | `REQ-0003`, `REQ-0004`, `REQ-0005` | Operators, implicit AND, dot-path traversal, ordering and limit |
| `TC-0003` | `REQ-0006`, `REQ-0033`, `NFR-0005` | Migration chain applied; no chain resets; reserved-namespace collision rejected |
| `TC-0004` | `REQ-0007`, `REQ-0008`, `REQ-0009` | One-shot vs. live; set-membership default; count-only and per-field opt-ins |
| `TC-0005` | `REQ-0010`, `REQ-0011` | Optimistic apply; handler serialization, atomic commit, read-your-own-writes |
| `TC-0006` | `REQ-0012`, `REQ-0013`, `REQ-0014` | Envelope contents; no row mutations sent; key and versions present |
| `TC-0007` | `REQ-0016`, `REQ-0017` | Outcome enum; ack ≠ acceptance; correction and rejection via subscription; rollback cascade |
| `TC-0008` | `REQ-0018`, `REQ-0019`, `NFR-0003` | Queue states; kill-and-restart recovery; dependent write held; local-id swap |
| `TC-0009` | `REQ-0020`, `REQ-0013`, `THR-0003` | Retry reuses the same key; backoff with jitter; only unacknowledged resent |
| `TC-0010` | `REQ-0021`, `REQ-0022`, `THR-0006` | Aggregate and per-command status; `blockedOn`; cancel with cascade; backlog threshold |
| `TC-0011` | `REQ-0023`, `REQ-0024`, `REQ-0025` | Entity-diff payload; partial patch; per-field LWW; delete wins; no client tie-break |
| `TC-0012` | `REQ-0015`, `REQ-0028`, `REQ-0032`, `THR-0001`, `THR-0007`, `THR-0008` | Identity carried unmodified, not reassignable; no network call; no telemetry |
| `TC-0013` | `REQ-0026`, `REQ-0027` | Membership-scoped pull; opaque checkpoint; reset is total; no automatic purge |
| `TC-0014` | `REQ-0029` | Always an array; independent resolution; correlation by key not position |
| `TC-0015` | `REQ-0030`, `REQ-0031`, `THR-0002` | At-rest encryption via engine capability; no key generated; backup exclusion default |
| `TC-0016` | `REQ-0034` | In-memory engine, fake transport, injected clock/id |
| `TC-0017` | `NFR-0002` | Every documented local operation succeeds with every network call failing |

## Entry and exit criteria

- **Entry**: applicable specifications are accepted and an implementation exists
  to exercise. Test plans describe intended verification, not proof that every
  platform has passed.
- **Exit**: all applicable cases pass and traceability identifies no accepted
  requirement or NFR without a verifying case. Record untested targets explicitly.

## Known coverage gaps

Named rather than left to be discovered from an RTM read:

- **No client/server handler-drift testing story**, despite determinism
  being the project's self-identified riskiest assumption (`RISK-0002`,
  `ADR-0014`). Nothing here tests that a client handler and its
  independently authored server counterpart agree.
- **No storage-engine conformance suite** — these cases run against the
  in-memory engine and test hoyadb's behaviour, not whether a *new engine
  implementation* satisfies `SPEC-0001` (`RISK-0004` adjacent;
  `TP-0001`).
- **No performance or load testing**, per the scope note above.
- **No bootstrap/initial-sync cases**, because that behaviour is
  unspecified (`RISK-0005`).
- **No multi-tab/multi-process cases**, because that behaviour is
  unspecified (`RISK-0003`).
- Behaviours that are genuinely undecided — reset versus queued writes,
  dependent chains beyond one hop, entity-diff correlation collisions — have
  **no cases, because there is no decided behaviour to verify**. These are
  gaps in the design, not in the plan.

## Traceability

- Parent: `ARC-0001`.
- Verifies: every `accepted` REQ/NFR and every mitigated or
  mitigation-planned THR, as tabulated above.

## Executed data model boundary

TC-0026 D01–ADR-0024 cover the Rust model through partitions, boundaries and lifecycle
transitions. Shared assertions, schema-storage cases, physical SQL faults and
independent restart checks guard atomicity and persistence. TC-0027/0028 are
permanent regressions for DEF-0002/0003. Performance, concurrency, sync and release
security require their own applicable verification.
