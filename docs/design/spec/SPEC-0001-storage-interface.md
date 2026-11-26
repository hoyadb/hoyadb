---
id: SPEC-0001
title: Storage interface — the engine-agnostic persistence contract
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-22
parents: [ARC-0001, ADR-0003]
satisfies: [REQ-0034, NFR-0003]
tags: [storage, interface]
---

# SPEC-0001: Storage interface — the engine-agnostic persistence contract

Source: `legacy/docs/04-storage/storage-interface.md`.

## Purpose

Define the one narrow contract every concrete storage engine implements, so
the layers above — schema, relations, reactivity, sync — never change based
on which engine is running underneath (`ADR-0003`, `OBJ-0002`).

## Responsibilities the interface must expose

1. **CRUD against typed tables**, matching a declared schema (`SPEC-0003`),
   with the concrete engine representing each hoyadb column type in its own
   native types.
2. **Subscribable queries** — enough surface for the reactivity layer to
   detect that a query's *result set* has changed, not merely to run the
   query once (`SPEC-0006`).
3. **Transactions, where the engine supports them** — used directly by the
   offline queue to commit a write's state transition together with the
   local data mutation it produced (`SPEC-0009`, `NFR-0003`).
4. **An in-memory / fake implementation**, required so application tests
   have a fast, hermetic backend with no filesystem or platform storage API
   involved (`REQ-0034`).

An engine that cannot support transactions still implements the rest; the
offline queue's atomicity guarantee is correspondingly weaker there, an
accepted limitation rather than a bar every engine must meet.

## Query access is async

Queries against this interface — including a read issued from inside a
command handler — are awaited. hoyadb serializes command-handler execution
rather than requiring synchronous engine access (`REQ-0011`, `ADR-0025`).

## What layers on top, not into

Cross-cutting concerns — at-rest encryption (`SPEC-0014`) and compression —
are layers above this interface, **not** methods it defines. The generic
contract does not grow encryption-specific methods; a concrete engine
adapter handles encryption internally, invisibly to everything above
(`REQ-0030`). This mirrors RxDB's storage wrappers, which compose on top of
any concrete engine without changing it.

## Known design risk

**Behaviour can leak across engines if the interface is not drawn
carefully** — a query feature or transaction guarantee one backend supports
and another does not. `legacy/docs/04-storage/storage-interface.md` states plainly
that no source read in this project's research treats this as a solved
problem. This is a live risk while the method set is drawn, **not something
this spec closes**.

## Open

- **The exact method signatures are not fixed** — query API shape,
  transaction API shape, and how change detection is exposed to the
  reactivity layer are all implementation-time detail. This spec fixes the
  interface's *responsibilities* and its pluggable, engine-agnostic
  property, not its literal function signatures.
- **There is no storage-engine conformance suite**, so "a new engine
  actually satisfies this contract" is currently unverifiable
  (`legacy/docs/09-testing.md`, `REQ-0034`). Disclosed, not designed.
- **Concurrent access to one store from two processes** — two browser tabs,
  two desktop windows — is unaddressed: no file locking, no leader election
  (`RISK-0003`).
- Indexing, which query execution against this interface depends on, is
  **undecided** (`RISK-0004`).

## Traceability

- Parents: `ARC-0001`, `ADR-0003`.
- Satisfies: `REQ-0034`, `NFR-0003`.
- Realized by: `SPEC-0002` (the default engine implementing it).
- Verified by: `TC-0016`, `TC-0008`.

## Current implementation contract — 2026-10-22

API-0002 remains the initial CRUD/atomic-write subset. SPEC-0025 and API-0003
now define supplied-schema typed queries, engine-owned execution and returned
committed before/after changes; TC-0025 is their conformance/cost gate. ADR-0030
resolves physical index ownership. These internal contracts do not complete
public subscriptions, encryption, queue durability or multi-client access.
The Open section above records the original design gaps; the concrete query
and change-return subset is now fixed by API-0003 rather than left unspecified.
