---
id: REQ-0026
title: What a client holds is computed server-side from membership, never from UI state
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, scoping, membership]
---

# REQ-0026: What a client holds is computed server-side from membership, never from UI state

## Pattern

Ubiquitous, with an Unwanted-behaviour clause against UI-derived scoping.

## Statement

THE system SHALL hold exactly the data the backend exposes to a client
based on server-declared membership rules computed for that client's
identity, and SHALL pull changes incrementally from an opaque checkpoint
token.

THE system SHALL NOT derive what it syncs from which queries the
application currently has mounted, and SHALL NOT sync a client's entire
permitted dataset unconditionally.

THE system SHALL assume one authoritative backend and SHALL NOT implement a
peer-to-peer or multi-master topology.

## Examples

Example 1 — membership computed from identity
(`legacy/docs/06-sync/implementation-guide.md`): the backend derives buckets from
the authenticated identity — `const buckets = await
computeMembership(identity);` — and returns only changes within them. Two
devices signed in as the same identity receive the same scoped data,
because membership is a function of identity, not of device or screen.

Example 2 — opaque checkpoint
(`legacy/docs/06-sync/transport-http-example.md`): the client sends
`GET /hoyadb/changes?since=eyJvcmRlcnMiOiJjaGtwdF8xMjMifQ` and receives
`{ "checkpoint": "eyJvcmRlcnMiOiJjaGtwdF8xMjQifQ", "changes": [...] }`. The
token's internal structure — per-table checkpoints, a global sequence
number, or anything else — is a backend detail the client never parses.

Example 3 — not UI-scoped: unmounting the orders list screen does not
narrow what the client holds, and remounting it triggers no re-sync from
scratch.

## Open

The exact shape of a membership rule declaration (a query language, a config
file format, something else) is implementation-time detail, **not fixed**
(`ADR-0006`). PowerSync's precedent names a real scaling constraint worth
carrying forward — a default cap on bucket count per client — rather than
treating the abstraction as free.

**Bootstrap / initial sync is unspecified**: everything here is incremental
catch-up from a checkpoint. A fresh install with no local data —
pagination for a large first sync, how reactivity behaves while it is in
flight — is unaddressed anywhere in the tree (`RISK-0005`).

## Traceability

- Parent: `OBJ-0003`.
- Under: `ADR-0006`.
- Satisfied by: `SPEC-0012`, `API-0001`.
- Verified by: `TC-0013`.
