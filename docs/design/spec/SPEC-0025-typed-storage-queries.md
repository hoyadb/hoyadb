---
id: SPEC-0025
title: Complete Phase 1 typed storage queries and committed changes
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [ARC-0001, ADR-0030]
satisfies: [REQ-0001, REQ-0003, REQ-0004, REQ-0005, REQ-0034, REQ-0040]
depends_on: [SPEC-0001, SPEC-0002, SPEC-0003, SPEC-0004, API-0002, RFD-0001]
tags: [storage, phase-one, query, conformance]
---

# SPEC-0025: Typed storage queries and committed changes

## Need and acceptance boundary

The operator approved RFD-0001's engine-owned index recommendation and asked to
finish Phase 1. Existing API-0002 establishes only CRUD/atomic batches for three
primitive types. Storage must execute the remaining typed query contract and
return committed row changes consistently in memory, native SQLite and the
supported single-worker browser SQLite adapter. API-0003 fixes the exact types,
validation, ordering, null/relation semantics and errors before implementation.

Phase 1 acceptance requires all TC-0025 cases plus preserved API-0002 tests;
query execution stays inside adapters. Supplying a schema is a storage input,
not completion of Phase 2 public schema declaration, relation navigation or
migrations. Committed changes are inputs to Phase 3, not live-query delivery.
Encryption, backup exclusion and crash-tested queue durability stay in their
roadmap phases. SPEC-0002's encryption obligation is not silently waived or
claimed complete. No package publication, remote CI success or broad platform
support follows from passing local storage cases.

## Scope and chosen approach

- Extend the common Value enum without changing existing variants' semantics;
  include finite float, millisecond timestamp, reference and Null, preserving
  equality assertions and owned complete rows.
- Add explicit-schema query and typed atomic batch capabilities from API-0003.
  Keep raw API-0002 CRUD/batches available and unchanged. The caller supplies
  the same schema to typed operations; no hidden mutable schema registry,
  metadata migration, auto-created application tables or new dependency.
- Validate schema/query in core; execute predicates, AND, direct to-one lookup,
  ordering and limit in the engine. SQLite filters/orders/limits in SQL rather
  than fetching all rows into shared Rust. Memory executes against its own
  maps/indexes. Shared semantic helper comparisons do not transfer execution
  ownership into core.
- Return one ordered vector of effective owned before/after changes only after
  a typed batch commits. Rollback returns no vector and preserves rows/indexes.
  No persistent feed or production subscription/bridge lifecycle is added.
- Preserve private v1 SQLite table DDL/data and API-0002 tags; use API-0003's
  checked additional tags and additive physical indexes. No reset, destructive
  migration, unsafe implementation or dependency change.

Explicit schemas avoid destabilizing the current persisted format or inventing
Phase 2's schema/version lifecycle. The accepted cost is repeating a schema
argument and retaining an unchecked raw storage escape hatch. Returned rows
are validated; nonreturned legacy rows need not be scanned. Public applications
will eventually use the data-model layer rather than calling unchecked storage.

## Verification and stop condition

Write TC-0025's shared executable cases before implementation and observe their
expected missing-behavior failures. Reuse the same assertions/factories for
memory and native SQLite, then execute the applicable suite in the real browser
worker with existing OPFS ownership/restart/cleanup restrictions. Preserve all
40 API-0002 workspace tests and the existing W01–W09/L01–L02 browser assertions.
Extend strict fixture decode, SQL-value binding, operational rollback and
process/worker restart checks for new values and typed batches.

P12 measures a bounded synthetic selective workload at recorded sizes, with a
nontrivial initial population and added unrelated rows. Show SQLite's actual
EXPLAIN QUERY PLAN index usage and memory candidate counts below a full scan
for the selective equality path. Separate first index build, repeated lookup,
write/update/rebuild and ordering work/timings. Repeat after changing indexed
fields, raw writes, rollback and reopen; show correct results and comparable
native/browser paths. Record ranges/scans honestly for other operators; this
slice does not promise every predicate is indexed or establish an invented
latency budget. It does not establish incremental live-query maintenance.

Run workspace tests, formatting, Clippy, core/memory WASM build, browser target
checks and real-browser tests, plus formal traceability and regenerated RTM. TC-0025 P13 verifies the configured
CI image/compiler and locked native/core/memory/browser compile paths while
preserving existing user runner tags; local commands are not a remote CI pass.
Document observed RED/GREEN, measurements, versions and limitations in the
implementation report. Stop at this acceptance boundary; do not add OR, offset,
cursors, aggregates, recursive/to-many traversal, schema migration or sync.

## Quality, threat review and rollback

Functional Suitability: shared TC-0025 outcomes. Performance Efficiency: P12
index build/update/query evidence; NFR-0001 has no numerical target yet.
Compatibility: old value semantics, DDL and owned rows retained. Interaction
Capability: internal Rust API only, no new user interface. Reliability: atomic
rows and change publication under ordinary failures. Security: REQ-0040 and
THR-0014 strict types/bound SQL; no network/logging. Maintainability: one common
contract/conformance suite. Flexibility: adapter-owned physical indexes behind
portable queries. Safety: no physical control or safety-critical use in this
bounded library slice. Existing NFR-0003 queue crash durability remains unproved.

Revert task code through review while preserving databases, user changes and
allocated artifacts. No cleanup/reset of user data is rollback. New-value rows
need a compatible reader; reverting to an older decoder can explicitly fail on
new tags. Optional index removal is a separate reviewed operation, not required
for logical rollback. No persisted application schema or schema reset is added.

## Review clarification — structural SQL limits

API-0003 records engine expression-depth/join limits for very long independent
predicate chains. SPEC-0028 fixes cardinality-dependent In expansion, without
promising unbounded arbitrary statement complexity.
