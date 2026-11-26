---
id: RFD-0002
title: Set data model relation and schema lifecycle policies
type: rfd
status: committed
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [ADR-0002, ADR-0009, OBJ-0002]
depends_on: [SPEC-0003, SPEC-0005, DM-0001, API-0003]
tags: [schema, migration, relations, data-model]
---

# RFD-0002: data model schema lifecycle

Origin: the operator requested starting and finishing full roadmap data model.
SPEC-0003 leaves join-table visibility open; SPEC-0005 and the implementation
plan require resolving migration failure, downgrade and queued-work/reset
boundaries before implementing them. These policies affect core and adapters,
so ARCH-3 requires discussion before an adopting ADR. Existing accepted ADR
bodies remain unchanged.

## Recommended concrete policies

1. Use explicit, application-queryable many-to-many join tables, as in the
   existing schema examples. Named relations provide lazy navigation and
   library-managed link/unlink operations on both sides. Implicit join-table
   generation and eager/batched loading are future extensions.
2. Keep application schema version and full canonical definition separate from
   SQLite's existing private format marker. Use the established `__hoya_`
   namespace for managed logical tables and implicit bookkeeping fields;
   application declarations and input fields in it reject.
3. Apply the complete authored ascending migration chain atomically, validating
   each declared endpoint. A failed step rolls back and reports failure; it
   never silently falls through to reset. No inferred transformations.
4. Preserve ADR-0009's missing-chain reset for supported, known, queue-free
   stores. Reset is a single transaction clearing/recreating owned logical
   application tables and updating metadata; it never deletes the SQLite file
   or its private physical tables. Reject downgrade, same-version definition
   drift, malformed/future metadata and unknown reserved bookkeeping without
   mutation. This includes any future queue until its lifecycle policy is
   separately specified; this phase cannot silently discard queued work.
5. Adopt an unversioned storage store only into declared version 1 and only if
   all its existing tables/rows match the declaration; preserve all rows and
   initialize metadata atomically. An empty store can initialize at any
   declared positive version. Other ambiguous legacy stores reject unchanged.

The public data-model/query API is Rust in the existing dependency-free core;
the guide's TypeScript method names are reference conventions, not an existing
SDK contract. Current platform test harnesses exercise that same core. Published
TS/Swift/Kotlin packages, live subscriptions, sync and release hardening retain
their roadmap phases. No runtime dependency is requested.

## Alternatives and tradeoffs

Implicit join tables avoid application declarations but introduce an unaccepted
name/visibility/migration policy. Explicit declarations reuse accepted examples
and keep membership inspectable, at the cost of declaring the through table.

Resetting downgrade/unknown bookkeeping could erase data created by newer code;
rejection preserves it. This is a compatibility guard, not a softer alternative
for the ordinary supported forward missing-chain reset already chosen in
ADR-0009. Queue-present reset remains outside the supported metadata format
until the queue phase defines recovery and ownership together.

Preserving private v1 DDL avoids destructive library-format migration. Metadata
and data must commit together. Reverting to storage code cannot promise safe
public schema lifecycle for a data model store; retain data and use compatible
code rather than treating deletion as rollback. No existing user store is
migrated by this proposal or the test runs; all execution fixtures are synthetic
and task-owned.

## Confirmation and discussion

Follow with SPEC-0026/API-0004/TC-0026, reviewed and committed before tests and
implementation. Shared memory/native/browser cases must prove TC-0001/0002/0003,
strict namespace/metadata validation, full-chain rollback, real restart and the
absence of resets on unsupported states.

- 2026-10-22: proposed policies ready for operator review; implementation of
  these policy-dependent paths awaits the operator's choice. Independent
  contract and test preparation continues.

- 2026-10-22: operator explicitly approved the recommended policies. The RFD
  completed discussion/publication and is committed; ADR-0031 adopts them.
