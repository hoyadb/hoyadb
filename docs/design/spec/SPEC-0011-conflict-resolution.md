---
id: SPEC-0011
title: Conflict resolution — per-field LWW, deletes win, per-table override
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-24
parents: [ARC-0001, ADR-0007, ADR-0017, ADR-0018]
satisfies: [REQ-0024, REQ-0025]
depends_on: [SPEC-0010]
tags: [sync, conflict-resolution]
---

# SPEC-0011: Conflict resolution — per-field LWW, deletes win, per-table override

## Current Phase 5 contract — 2026-10-24

ADR-0033/API-0009 fix the pure per-table MergeHook signature, guarded persistent
field stamps and tombstones, and backend-selected typed corrections. Hooks can
customize addressed unequal-time values with the default metadata and identity
preserved; they cannot choose unresolved ties or resurrect authoritative
tombstones. TC-0034 E07–E10 verifies real two-client sequences and backend
first-arrival fixtures. The alternatives below remain research options, not
additional implemented algorithms. The hook-shape open item is resolved.

Source: `legacy/docs/06-sync/conflict-resolution.md`.

## Purpose

Specify how the entity-diff path (`SPEC-0010`) resolves two writes that
genuinely disagree, and why the command path needs no equivalent.

## The command path needs no merge step

For a command, **the backend's authoritative re-run *is* the resolution**.
There is no independent client-side and server-side *state* to reconcile —
only a client-side guess and a server-side authoritative result. Conflict
resolution as a distinct mechanism applies **only** to the entity-diff
fallback, where two writes to the same record can disagree with no re-run to
arbitrate.

## Default: per-field last-write-wins, deletes win

- The most recently written value for **each individual field** wins.
- **A delete always beats a later update** to the same record.
- Any table may **override** the default with its own resolution logic.

Field granularity is load-bearing: it depends on `SPEC-0010`'s payload
carrying only the fields actually written, and it is what avoids the silent
whole-record loss `legacy/VISION.md` rejects.

## The default's real limitation, stated

**Per-field LWW still silently loses one side of a genuine field-level
double-write.** Two users incrementing the same counter in the same window
both see their own value overwritten, not summed.

This is exactly the counter-CRDT gap that motivates the command path
existing as the primary mechanism in the first place. **A table with that
kind of merge need should use a command (`SPEC-0008`), not lean on a
per-table override of this default to solve it.**

## Available per-table overrides

Three alternatives were weighed and kept as available overrides rather than
adopted as the default (`ADR-0007`):

| Override | Strength | Cost |
| --- | --- | --- |
| Server-wins then client-wins per changed field (WatermelonDB's) | Probably the most reusable concrete algorithm in the research set; a local edit is not lost merely because the server's clock or write order was later | Needs per-row `_status`/`_changed` bookkeeping — a real ongoing storage cost — and still does not solve counters |
| Per-column CRDT dispatch (cr-sqlite's) | Handles ordering, and counters if correctly supported, automatically | "No manual hook at all" is the opposite pole from hoyadb's pluggable design; cr-sqlite's own docs and README disagree on whether its counter CRDT works |
| Loser preserved for the app to reconcile (PouchDB's `_conflicts`) | Strictly safer against silent data loss than any of the above; worth offering for safety-sensitive fields | Defers reconciliation to the app, which must actively look at the preserved revision or it accumulates unseen |

## Tie-break for identical timestamps

Where two writes to the same field carry an identical timestamp, the tie is
broken **server-side, by real arrival order** — the first write the backend
receives wins, the second is ignored. Only the backend knows real arrival
order; `clientTimestamp` is comparison input, never the tie-break authority.

Backend-side transactions are **recommended** to prevent a double-apply race
under true concurrency. **This is a backend-implementation recommendation
hoyadb documents, not a mechanism hoyadb runs** — hoyadb has no server-side
component to enforce it in.

## Same identity, multiple devices

Two devices authenticated as the same identity, writing offline to the same
field, is treated as an **ordinary case** for whichever rule already applies
— the default above, or a table's override. hoyadb adds no device-aware
merge logic and requires no device identifier by default (`ADR-0018`,
`SPEC-0012`).

## Open

- **The exact shape of the per-table override hook** — its signature, what
  state it receives (both candidate writes? the record's full history?) — is
  implementation-time detail, **not fixed**.
- **Broader integration guidance remains incomplete.** The supporting
  [sync implementation guide](../../guides/sync-implementation.md) explains
  client/server sync wiring, but does not cover every non-sync integration
  pattern. Multi-device behavior is specified here and in `SPEC-0012`;
  the broader integration-guide gap remains open.

## Traceability

- Parents: `ARC-0001`, `ADR-0007`, `ADR-0017`, `ADR-0018`.
- Satisfies: `REQ-0024`, `REQ-0025`.
- Depends on: `SPEC-0010`.
- Verified by: `TC-0011`.
