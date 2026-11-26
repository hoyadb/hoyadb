---
id: REQ-0029
title: Writes always push as an array, each resolved independently and correlated by key
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, transport, batching]
---

# REQ-0029: Writes always push as an array, each resolved independently and correlated by key

## Pattern

Ubiquitous, with an Unwanted-behaviour clause against cross-write
atomicity.

## Statement

THE system SHALL push eligible writes as an array of envelopes through a
single transport operation, including when only one write is eligible — a
lone write is an array of length one, not a different wire shape.

THE system SHALL correlate each returned outcome to its originating write
by idempotency key, or by record id for an entity-diff, and SHALL NOT
correlate by array position.

IF one write in an array fails, THEN THE system SHALL NOT roll back or
block the others — an array is a transport-level grouping, not a
transaction, and hoyadb promises no cross-write atomicity.

IF an outcome for a given write is absent from the response, THEN THE
system SHALL treat that write as still `sent` and eligible for retry,
exactly as it treats a lost response.

## Examples

Example 1 — two independent commands in one request
(`legacy/docs/06-sync/transport-http-example.md`): `add-order-v1` and
`add-order-note-v1` (on a *different*, already-synced order) are sent
together as a two-element array, and the response is a two-element array of
outcomes, each carrying its own `idempotencyKey`.

Example 2 — a batch of one (`legacy/docs/10-walkthrough.md` step 5): with a note
command still blocked on its parent's unresolved local id (`REQ-0019`),
only the order is eligible. It goes out as a one-element array — the
ordinary case, not a special one.

Example 3 — why position is unsafe: a retry may resend only part of a
previous array. Matching `01J...Q3` by key works regardless of where it
lands in the resent array; matching by index would silently attribute the
wrong outcome.

## Open

**A known, unclosed correlation gap**: `recordId` alone cannot distinguish
two queued entity-diff writes to the *same* record landing in one array —
two offline edits to different fields of one note produce two outcomes with
an identical `recordId` and nothing to tell them apart. Closing this needs
a per-write correlation id for entity-diffs, which is **not designed**
(`ADR-0026`).

A developer wanting one request per write builds that inside their own
transport — same endpoint, length-one arrays — but **must send them
sequentially**, since firing them concurrently races dependent writes
against each other and silently breaks ordering.

## Traceability

- Parent: `OBJ-0003`.
- Under: `ADR-0026`.
- Satisfied by: `SPEC-0013`, `API-0001`.
- Verified by: `TC-0014`.
