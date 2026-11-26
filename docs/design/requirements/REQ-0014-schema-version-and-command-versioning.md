---
id: REQ-0014
title: Every write envelope declares the client's schema version; command names carry a version suffix
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, versioning, schema]
---

# REQ-0014: Every write envelope declares the client's schema version; command names carry a version suffix

## Pattern

Ubiquitous.

## Statement

THE system SHALL include the client's current schema version in every
command envelope and every entity-diff payload it constructs.

THE system SHALL treat a command's name as carrying its own explicit
version suffix, versioned independently of the schema version, so a change
to a command's argument shape is a new command name rather than a silent
redefinition of an existing one.

THE system SHALL NOT define what a backend does on seeing a schema-version
mismatch — rejecting, translating, or accepting as-is is the backend's own
policy.

## Examples

Example 1 — both versions visible on one envelope
(`legacy/docs/06-sync/command-protocol.md`): the envelope carries
`"command": "add-order-v1"` and `"schemaVersion": 3`. The `-v1` suffix
versions the command's argument shape; `3` versions the schema those
arguments' tables belong to. A schema change to version 4 that does not
alter the command's arguments leaves the command `add-order-v1`.

Example 2 — a command's shape changes: adding a required `couponCode`
argument produces `add-order-v2` as a new, separately-registered command,
rather than changing `add-order-v1`'s meaning. A client still on the older
build keeps sending `add-order-v1`, which the backend must still have
registered.

Example 3 — an entity-diff carries it too
(`legacy/docs/06-sync/entity-diff-fallback.md`): the `order_notes` update payload
includes `"schemaVersion": 3` alongside its `table`, `op`, `recordId` and
`changes`.

## Open

`legacy/docs/06-sync/command-protocol.md` names the concrete case this leaves
unaddressed: a staged app-store rollout means the backend serves clients on
schema version *n* and *n-1* simultaneously for days or weeks, not a clean
cutover. Near-universal in practice, and still entirely the backend
implementer's to handle. hoyadb also does not track or enforce how long an
old command handler must stay registered on the backend.

## Traceability

- Parent: `OBJ-0003`.
- Under: `ADR-0013`.
- Satisfied by: `DM-0002`, `DM-0003`, `SPEC-0008`, `API-0001`.
- Verified by: `TC-0006`.
