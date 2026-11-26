---
id: REQ-0006
title: A schema version change applies an authored migration chain, or resets the local database
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [data-model, migrations]
---

# REQ-0006: A schema version change applies an authored migration chain, or resets the local database

## Pattern

Event-driven, with an Unwanted-behaviour clause for a missing chain.

## Statement

WHEN an application opens a database whose local schema version is lower
than the declared schema's version, THE system SHALL apply each authored
migration step in order from the local version up to the declared version,
as a purely local operation performed on app start before sync resumes.

IF no authored migration chain reaches the declared version from the
client's current local version, THEN THE system SHALL drop and recreate the
local database at the declared schema, and SHALL NOT attempt a partial or
inferred upgrade.

THE system SHALL NOT infer a migration step that a developer did not
author.

## Examples

Example 1 — chain exists (`legacy/docs/03-data-model/migrations.md`): a client on
schema version 1 opens an app declaring version 3, with migrations
`1→2` (add column `orders.cancelled_at`) and `2→3` (add column
`order_items.discount_cents`) authored.

- Result: both steps apply in order, 1→2 then 2→3. Locally-held orders and
  order items are preserved, with the two new columns added.

Example 2 — no chain (`legacy/docs/03-data-model/migrations.md`): a client on
schema version 1 opens an app that jumped straight to a version 4 schema,
with no chain reaching 4 authored.

- Result: the local database is dropped and recreated at version 4. That
  client's locally-held data is discarded. A reset does **not** on its own
  resync data back down from the backend.

## Open

The exhaustive set of migration step primitives (add/drop/rename column,
add/drop table, backfill an added column) is implementation-time detail,
not fixed here.

**A real, disclosed gap**: neither this requirement, `ADR-0009`, nor
`legacy/docs/06-sync/offline-queue.md` says what happens to commands still
sitting unacknowledged in the offline queue at the moment of a reset.
Dropping the local store presumably drops them, silently discarding a
user's pending write. **Not resolved** — no test case covers it because
there is no decided behaviour to verify.

## Traceability

- Parent: `OBJ-0002`.
- Under: `ADR-0009`.
- Satisfied by: `SPEC-0005`.
- Verified by: `TC-0003`.
