---
id: REQ-0041
title: Schema lifecycle changes preserve a validated atomic managed state
type: requirement
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [OBJ-0001, OBJ-0002]
mitigates: [THR-0015]
tags: [schema, migration, integrity, compatibility]
---

# REQ-0041: Managed schema lifecycle integrity

WHEN a managed database initializes, adopts compatible Phase 1 data, applies an
authored migration chain, resets a supported missing-chain store or performs a
public write, THE system SHALL publish only a complete validated state whose
application rows, logical inventory, canonical schema metadata, implicit row
schema versions and ID counter have committed atomically under API-0004.

IF expected state is stale, schema/metadata is malformed or unsupported, a
reserved name collides, or a step or commit fails, THEN THE system SHALL return
an explicit error without a partial result, partial mutation or fallback reset.
Downgrades, same-version schema drift, unknown reserved bookkeeping and any
queue-present state SHALL be rejected unchanged. Compatible adoption SHALL
preserve all application rows and IDs and SHALL occur only at declared version
1 for a populated unversioned store; an empty store MAY initialize at any
positive supported declared version.

Example: a version-1 store migrates 1→2 successfully inside a transaction, then
its 2→3 required-column backfill fails. Opening returns failure; version 1,
its full definition, rows and counter remain unchanged. No reset is attempted.
A version-1 store with no complete authored path to version 4 instead resets
atomically only when its metadata is known, supported and queue-free.

The existing private SQLite format marker is not an application schema version.
All execution fixtures are task-owned; this change performs no migration or
reset of an existing user database. Queue recovery, sync resumption and
crash/power-loss durability are not established by this requirement.
