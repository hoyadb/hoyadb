---
id: REQ-0025
title: The client does not break an identical-timestamp tie; the backend does, by arrival order
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, conflict-resolution]
---

# REQ-0025: The client does not break an identical-timestamp tie; the backend does, by arrival order

## Pattern

Ubiquitous, with an Unwanted-behaviour clause against client-side
tie-breaking.

## Statement

THE system SHALL include a client timestamp in every entity-diff payload
for the per-field last-write-wins comparison (`REQ-0024`), and SHALL treat
that timestamp as comparison input only, never as the authority that
settles a tie.

IF two writes to the same field carry an identical timestamp, THEN THE
system SHALL NOT attempt to break the tie client-side — the tie is broken
by the backend, by real arrival order, with the first write received
winning and the second ignored.

## Examples

Example 1 — the payload's role: the `order_notes` update carries
`"clientTimestamp": "2026-09-15T14:03:00.000Z"`. hoyadb's client attaches
it and sends it; it performs no local tie-break with it.

Example 2 — the tie, server-side: two devices write
`order_notes.body` with the same timestamp. Whichever the backend receives
first wins; the second is ignored. Backend-side transactions are
*recommended* to stop a naive read-then-write applying both under true
concurrency.

## Open

**This is a backend-implementation recommendation hoyadb documents, not a
mechanism hoyadb runs.** hoyadb has no server-side component, so it cannot
enforce or detect an incorrect implementation. Only the client-side half —
that the payload carries the timestamp and attempts no local tie-break — is
verifiable within hoyadb (`ADR-0017`).

## Traceability

- Parent: `OBJ-0003`.
- Under: `ADR-0017`.
- Satisfied by: `SPEC-0011`, `DM-0003`.
- Verified by: `TC-0011`.
