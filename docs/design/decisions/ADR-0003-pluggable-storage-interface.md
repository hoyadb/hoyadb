---
id: ADR-0003
title: Storage — one narrow pluggable interface, SQLite-family engine as the default
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [storage, engine, sqlite]
---

# ADR-0003: Storage — one narrow pluggable interface, SQLite-family engine as the default

Source: `legacy/refined-vision/03-storage-engine/alternatives.md`, forked into
`legacy/docs/04-storage/storage-interface.md` and `legacy/docs/04-storage/default-engine.md`.

## Context and problem statement

`legacy/NEEDS.md` expects the storage engine to "differ per environment rather
than being one engine everywhere, but leaning toward SQLite or an
equivalent embedded relational engine per platform," without finalizing it.
hoyadb needs persistence that can vary per platform without the schema,
relation, reactivity or sync layers above it changing.

## Considered options

- **One narrow, engine-agnostic storage interface, concrete engine swapped
  per platform.** Three independent, sequential, non-collaborating systems
  converged on this shape: PouchDB (the earliest — one API, IndexedDB/
  WebSQL/LevelDB per environment), RxDB (`RxStorage`, with composable
  storage *wrappers* for encryption and compression), and WatermelonDB (a
  narrower `DatabaseAdapter`, SQLite vs. LokiJS on web).
- **A fixed, non-pluggable engine** — Realm's approach.
- **Local storage as a byproduct of syncing an existing server database** —
  PowerSync and ElectricSQL's model.

## Decision outcome

In the context of choosing how hoyadb persists data across platforms,
facing `legacy/NEEDS.md`'s own expectation that the engine differs per
environment, we decided for **one narrow storage interface with the
concrete engine swapped underneath, defaulting to a SQLite-family engine**
and against a fixed non-pluggable engine and against deriving local storage
from an existing server database, to achieve per-platform engine freedom
without changing anything above the interface, accepting the real risk that
behaviour can leak across engines if the interface is not drawn carefully.

### Consequences

**Good**
- Nothing above the interface — schema, relations, reactivity, sync —
  depends on which engine is running. Three converging, non-collaborating
  systems including the oldest in the set is treated as evidence this shape
  is durable, not a fad.
- Cross-cutting concerns (at-rest encryption, compression) layer on top of
  the interface rather than becoming interface methods; each concrete
  engine adapter handles encryption internally (`ADR-0019`).
- The interface must include an in-memory/fake implementation, which is
  what gives `legacy/docs/09-testing.md` a hermetic test backend (`SPEC-0001`).

**Bad**
- **Behaviour can leak across engines** — a query feature or transaction
  guarantee one backend supports and another does not. `legacy/docs/04-storage/
  storage-interface.md` states plainly that no source read in this
  project's research treats this as solved. It is a live design risk while
  the interface's method set is drawn, not something this ADR closes.
- Transactional atomicity for the offline queue is engine-dependent by
  construction: engines without transactions get best-effort state tracking
  (`ADR-0016`, `NFR-0003`).
- The exact method signatures — query API shape, transaction API shape, how
  change detection is exposed to reactivity — are **not fixed**;
  implementation-time detail.
- There is **no storage-engine conformance suite**, so "a new engine
  actually satisfies the interface" is currently unverifiable
  (`legacy/docs/09-testing.md`). Disclosed, not designed.

### Confirmation

Confirmed by `SPEC-0001` (the interface) and `SPEC-0002` (the default
engine), and by `TP-0001` running app-level cases against the in-memory
engine.

## Pros and cons of the options

### Narrow pluggable interface (chosen)
- Good: per-platform engine freedom; three independent precedents;
  composable encryption/compression wrappers.
- Bad: cross-engine behaviour leakage; no conformance suite yet.

### Fixed, non-pluggable engine (Realm)
- Good: one behaviour, nothing to leak.
- Bad: directly contradicts `legacy/NEEDS.md`'s stated expectation, and ties
  hoyadb to whatever one engine can or cannot do on every platform — e.g.
  no browser-native OPFS option if that engine were SQLite-only.

### Local storage as a byproduct of syncing a server database
- Bad: rejected for the same external-database-coupling reason as
  `ADR-0002`'s rejected mapping option.

## Notes on the default engine

The default is a SQLite-family engine, expected to sit on `rusqlite` to
stay aligned with `legacy/NEEDS.md`'s SQLite lean while fitting `ADR-0001`'s Rust
core. An ecosystem check (not part of the original 8-source research) rules
out `sled` — described as in maintenance mode and unsuitable for new
projects, with `redb` named as its own suggested successor. **Whether
`rusqlite` or a hand-rolled layer over `redb` is the concrete crate is not
finalized**; the check narrows the field, it does not pick between the two
remaining options.

## Traceability

- Parent: `OBJ-0002`.
- Referenced by: `ARC-0001`, `SPEC-0001`, `SPEC-0002`, `SPEC-0014`.
