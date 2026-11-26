---
id: ADR-0006
title: Topology & scoping — one authoritative backend, server-computed membership
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, scoping, topology, membership]
---

# ADR-0006: Topology & scoping — one authoritative backend, server-computed membership

Source: `legacy/refined-vision/06-sync-scoping-topology/alternatives.md`, forked
into `legacy/docs/06-sync/scoping-topology.md`. The reference research flagged
this as a genuinely open, first-order gap deserving its own decision,
independent of the command-vs-state question in `ADR-0005`.

## Context and problem statement

Two linked questions: is there an authoritative side at all, and how much
of the data a client is permitted to see does it actually hold? `legacy/NEEDS.md`
describes reconciling with "a server-side backend over an API or other
communication layer," but does not fix the topology.

## Considered options

- **One authoritative backend, with what a client holds declared once
  server-side as membership rules and computed for that client.**
  PowerSync's Sync Rules/buckets are the concrete precedent: a parameter
  query deriving e.g. `user_id` from the JWT, plus data queries using those
  parameters, computing exactly which buckets a client syncs.
- **Sync the whole accessible dataset unconditionally** — RxDB's and
  WatermelonDB's approach; the simplest possible scoping, no membership or
  bucket machinery at all.
- **Scope to whatever queries are currently mounted in the UI, with a TTL
  after deactivation** — Zero's model.
- **Peer-to-peer / multi-master with no designated server** —
  PouchDB/CouchDB's proven approach, and cr-sqlite's row/column-CRDT
  equivalent.

## Decision outcome

In the context of deciding hoyadb's client-server topology and how much
data a client holds, facing the need for a predictable data footprint and
somewhere for conflict resolution to land, we decided for **one
authoritative backend with server-declared, server-computed membership
rules** and against unconditional whole-dataset sync, against UI-state-
derived scoping, and against a peer-to-peer/multi-master topology, to
achieve a client footprint that is a function of identity and permissions
rather than of which screens happen to be open, accepting that hoyadb now
depends on a backend that can express and compute those rules.

### Consequences

**Good**
- The client sees exactly what the backend decides to expose; nothing has
  to be inferred client-side.
- Conflict resolution has a natural place to land — the authoritative side
  (`ADR-0007`, `ADR-0017`).
- Membership is computed from identity, so two devices under one identity
  hold the same scoped data (`ADR-0018`).

**Bad**
- PowerSync names a concrete, load-bearing scaling constraint worth
  carrying forward rather than treating the abstraction as free: a default
  cap on bucket count per client (1,000).
- The exact shape of a membership rule declaration (a query language, a
  config file format, something else) is **not fixed** — implementation-time
  detail.
- Peer-to-peer was **not rejected on technical grounds**. It is the most
  production-proven option at scale across multiple vendors and most
  directly honours `legacy/NEEDS.md`'s "or other communication layer" phrasing.
  It is flagged as the option most worth revisiting if a real signal for it
  (multi-device editing with no backend at all) ever appears — there is no
  such signal today.
- What happens to already-synced data when membership narrows is a separate
  decision, resolved deliberately *against* automatic purging
  (`ADR-0015`, `THR-0004`).

### Confirmation

Confirmed by `SPEC-0012` (scoping and topology) and the membership test
cases in `TP-0001`.

## Pros and cons of the options

### Server-computed membership, one authoritative backend (chosen)
- Good: predictable footprint tied to identity; declared once, reused by
  every matching client; gives conflict resolution an authoritative side.
- Bad: bucket-count scaling constraint; rule-authoring syntax undecided.

### Whole accessible dataset, unconditionally
- Good: simplest possible — no machinery at all.
- Bad: does not scale to large permission-filtered datasets, undercutting
  the very storage-layer effort (lazy loading, off-JS-thread work) projects
  like WatermelonDB otherwise invest in to handle large local datasets.

### UI-mounted-query scoping with TTL (Zero)
- Good: tightest possible footprint.
- Bad: makes the footprint a function of UI state rather than
  identity/permissions — less predictable for capacity planning; a briefly
  unmounted query either re-syncs from scratch or relies on a TTL tuning
  knob a declarative model does not need.

### Peer-to-peer / multi-master
- Good: most production-proven at scale; honours "or other communication
  layer" most directly.
- Bad: the biggest architectural commitment of the four — conflict
  resolution must be solved symmetrically with no authoritative side to
  defer to, with no current need for it.

## Traceability

- Parent: `OBJ-0003`.
- Referenced by: `ARC-0001`, `SPEC-0012`.
