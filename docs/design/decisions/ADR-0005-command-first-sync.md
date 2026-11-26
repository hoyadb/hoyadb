---
id: ADR-0005
title: Sync mechanism — named commands as the primary write path, entity-diff as fallback
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001, OBJ-0003]
tags: [sync, commands, entity-diff]
---

# ADR-0005: Sync mechanism — named commands as the primary write path, entity-diff as fallback

Source: `legacy/refined-vision/05-sync-mechanism/alternatives.md` (Options A/B/C,
Option A chosen), forked into `legacy/docs/06-sync/overview.md`. This is the
single most-researched question in the project's reference set.

## Context and problem statement

`legacy/NEEDS.md`'s "Sync mechanism" section states a preference for
command-based synchronization over raw state/entity diffing, so the backend
can apply its own logic around a change's *intent* rather than merging
resulting rows — while explicitly allowing a conventional entity-based sync
as an acceptable fallback if commands prove impractical. Both reads and
writes must work with no connection (`OBJ-0001`).

## Considered options

- **Option A — command-based sync as primary, entity-diff as fallback.**
  Zero's mutators are the concrete precedent: named functions
  (`defineMutator`) called optimistically client-side and re-executed
  server-side "in a transaction against your database," with a `ctx` object
  described as "a server-controlled context the client cannot set."
- **Option B — pure entity-diff / CRUD sync, standing alone.** The
  majority approach: 6 of 8 researched systems (RxDB, WatermelonDB,
  PowerSync, PouchDB); WatermelonDB's protocol alone reports roughly 13
  independent backend implementations. By far the best-precedented single
  option in the set.
- **Option C — read-only sync; writes are the app's own problem.**
  ElectricSQL's stated scope.

## Decision outcome

In the context of choosing what actually travels when a client writes,
facing the structural failure of value diffing on merges beyond last-write-
wins, we decided for **named commands as the primary write path with a
plain entity-diff fallback** (Option A) and against pure entity-diff sync
standing alone (Option B) and against read-only sync (Option C), to achieve
correct merge semantics for counter- and ordering-style writes and to
preserve why a change happened, accepting that running two sync models side
by side is more surface area than either alone and that the domain-level
command shape has no direct precedent in the research set.

### Consequences

**Good**
- `increment` is trivially correct as a re-executable function, unlike a
  value diff. This is the exact wall Option B hits — found independently in
  Realm's historical operational-transform work, which needed a special
  increment operation, and in cr-sqlite's own docs-vs-README disagreement
  over counter support.
- The entity-diff path is kept as a real fallback so not every write
  requires designing a command. The two coexist per table/write, not
  globally exclusively.
- "Lower-ceremony" for the fallback means skipping domain logic, **not**
  skipping authorization: an entity-diff write is checked against the same
  trusted context and membership rules a command is (`REQ-0023`).

**Bad**
- Two sync models to design, document, test and keep consistent.
- No source in the reference research does *domain-level* commands
  specifically. Zero's mutators are recorded in the reference comparison as
  "closer to typed RPC with local prediction than true business intent," so
  real, unprecedented design work remains.
- The whole model depends on a command's two handler runs reaching
  compatible results — an open problem hoyadb explicitly declines to solve
  (`ADR-0014`, `RISK-0002`).
- **Bootstrap / initial sync is unspecified** — everything designed here is
  incremental catch-up from a checkpoint. A fresh install with no local
  data (pagination for a large first sync, reactivity behaviour while it is
  in flight) is a real, disclosed gap: `RISK-0005`.

### Confirmation

Confirmed by `SPEC-0008` (command protocol), `SPEC-0010` (entity-diff
fallback), `DM-0002`, `DM-0003`, and the sync test cases in `TP-0001`.

## Pros and cons of the options

### Option A — command-first with entity-diff fallback (chosen)
- Good: correct counter/ordering merges; preserves intent; keeps a
  low-ceremony path for plain tables.
- Bad: two models; no domain-level-command precedent; determinism unowned.

### Option B — pure entity-diff
- Good: by far the best-precedented option; 6 of 8 systems, ~13
  independent backend implementations of WatermelonDB's protocol alone.
- Bad: hits a structural wall on counter-style merges. Kept as hoyadb's
  fallback rather than adopted as the sole mechanism.

### Option C — read-only sync
- Bad: rejected outright — directly conflicts with `legacy/NEEDS.md`'s
  offline-first requirement that reads *and* writes work with no
  connection. Three of ElectricSQL's four named write patterns do not work
  offline for writes at all, and the one that does is really Option A or B
  built on top.

## Traceability

- Parents: `OBJ-0001`, `OBJ-0003`.
- Referenced by: `ARC-0001`, `SPEC-0008`, `SPEC-0010`, `SPEC-0011`.
- Risks: `RISK-0002`, `RISK-0005`.
