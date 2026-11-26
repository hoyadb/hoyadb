---
id: ADR-0030
title: Engines own physical indexes behind the portable storage query contract
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [OBJ-0002, OBJ-0005, REQ-0003, REQ-0004, REQ-0005]
depends_on: [RFD-0001, ADR-0003, ADR-0027]
tags: [storage, query, indexing, phase-one]
---

# ADR-0030: Engine-owned physical indexes

## Context and problem statement

ADR-0027 fixes the minimal query catalogue while leaving indexing open.
RFD-0001 compares index ownership across core, memory and SQLite. API-0002's
CRUD-only foundation cannot yet execute portable typed queries or supply the
committed changes needed by later reactivity. The operator approved the RFD's
engine-owned recommendation and instructed continuation to finish Phase 1.

## Considered options

- Engine-owned physical indexes: portable semantic schema/query inputs, with
  each adapter choosing lookup structures and measured costs.
- Application-declared indexes: explicit integrator control, but adds a schema,
  validation and migration contract before a measured workload exists.
- Automatic inference/advisor: adapts to usage, but adds policy for observation,
  resource limits, eviction and rebuild before that policy is needed.

## Decision outcome

In the context of completing the initial portable storage query layer, facing
an unresolved indexing boundary across memory and native/browser SQLite, we
decided for engine-owned physical indexes and against application-declared
indexes and a core automatic index advisor, to achieve a small portable query
contract with adapter-native execution, accepting engine-specific physical
costs and the need for workload and plan evidence per adapter.

### Consequences

Core validates supplied schema/query types; engines execute filters, direct
to-one lookup, ordering and limits. Applications specify no index hints or
portable index metadata. SQL names/operands remain bound values. Index creation
and maintenance must preserve existing compatible data and rollback semantics;
no reset or destructive format migration is authorized. SPEC-0025/API-0003
supply concrete typed storage contracts and TC-0025 supplies behavioral and
index-cost proof. No new dependency is adopted by this decision.

This fills ADR-0027's explicitly deferred indexing choice; it does not replace
its minimal operator catalogue or change accepted ADR bodies. RISK-0004's
ownership uncertainty is resolved, while costs and incremental live-query
maintenance remain explicit validation work. A correct one-shot scan alone
does not establish incremental result-set maintenance or Phase 3 completion.
Phase 2 public schema/relation APIs, migrations, encryption and sync remain
separate roadmap work.

### Confirmation

The operator's 2026-10-22 instruction to finish Phase 1 followed the recommendation
in RFD-0001 and approves this ownership choice. Before implementation, review
SPEC-0025/API-0003 and TC-0025. Confirm behavior with the same conformance cases
in memory/native/browser SQLite; confirm costs with measured construction,
update and selective lookup work plus actual native/browser SQL index plans.
No performance or completion claim is made by accepting this decision.
