---
id: RFD-0001
title: Choose ownership of initial storage query indexes
type: rfd
status: committed
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [ADR-0003, ADR-0027, OBJ-0005]
depends_on: [SPEC-0001, SPEC-0003, SPEC-0004, SPEC-0006, RISK-0004, API-0002]
tags: [storage, query, indexing, storage]
---

# RFD-0001: Initial storage query index ownership

## Need and verified starting point

Origin: the operator requested completion of Stage 0 and storage. The bounded
Stage 0 experiments and SPEC-0023 persistent browser experiment have executed
results. API-0002 implements only CRUD/atomic batches for three value types;
it has no schema, query execution or change-reporting methods. Its 40 host
tests and worker subset therefore cannot complete SPEC-0001/SPEC-0002 or
roadmap storage.

SPEC-0004's Open section requires an indexing decision before implementation;
accepted ADR-0027 preserves that open question and RISK-0004 explains the
query/incremental-reactivity consequences. This spans core, memory and SQLite,
so ARCH-3 requires an RFD before an adopting ADR. No accepted ADR body is edited.
The original proposal requested no new runtime dependency; the operator-approved
follow-on is bounded by ADR-0030 and SPEC-0025.

## Recommendation: engine-owned physical indexes

Choose physical index ownership inside each storage adapter. The application
schema initially contains no index declarations; the core query model contains
semantic predicates, ordering and limit, without backend index hints. Engines
execute that shared model and own their physical lookup structures. Core
validates the model/schema; the same conformance suite verifies results and
errors against memory, native SQLite and the supported browser connection.

This keeps the accepted engine substitution boundary intact: installing an
adapter does not require application query or schema changes. It also avoids
committing an unmeasured automatic index-advisor/cache policy. Secondary index
layout, creation cost and query plans are implementation details, but must be
measured with the concrete workload before performance/reactivity acceptance.
A full scan cannot silently substitute for the accepted incremental live-query
requirements. Those remain a separate mechanism and verification gate.

A native and browser SQLite target may use the same SQL query/index planning
code while retaining their different SQLite builds and lifecycle restrictions.
The in-memory implementation owns its own equivalent lookup structures and
remains free of filesystem/platform calls. Physical indexes do not become
schema-portable user data or cross the platform message boundary.

## Alternatives

| Option | Benefit | Cost / compatibility consequence |
| --- | --- | --- |
| Engine-owned physical indexes (recommended) | Keeps the initial schema/API small and query semantics portable; each engine uses its own storage mechanisms. | Physical performance can differ; shared behavioral tests alone cannot prove costs. Requires explicit workload/plan evidence per engine. |
| App-declared portable indexes | Makes expected lookups explicit and gives integrators control. | Adds a cross-engine schema contract now, including index validation, persistence and migrations, before a measured workload exists. |
| Automatic inference/advisor | Can adapt to observed query use without app declarations. | Introduces lifecycle, eviction, rebuild, resource-limit and reproducibility policies that are not designed. More code and measurements before the foundation closes. |

A shared core that scans/copies all rows instead of engine query execution was
also considered. It conflicts with the implementation plan's engine-execution
boundary and would not establish incremental query maintenance. It is not the
recommended shortcut for closing storage.

## Follow-on contract gates

After an ownership choice, prepare and review the next SPEC/API/TC slice before
implementation. That slice must define at least:

- Exact storage query/result/error signatures for the accepted operators `=`,
  `!=`, `<`, `<=`, `>`, `>=`, `in`, implicit AND, ordering and limit. No OR,
  offset/cursor pagination, new aggregates or has-many predicate traversal.
- Schema/type prerequisites, including float/timestamp/reference representation,
  optional-value and mixed-type comparisons, string comparison, invalid query
  behavior and deterministic ties under an ordered limit. DM-0001 and SPEC-0003
  name these types but do not supply these exact storage semantics.
- Committed change information sufficient for Rust reactivity to maintain
  result membership, including batch/rollback ordering and owned snapshots.
  A storage change feed is not itself the public live-query implementation.
- Disposal/handle ownership and scheduling for production bridges. The current
  browser harness is test-only and does not establish subscription ownership.
- Private-format compatibility and index lifecycle. Do not reset an existing
  compatible database or claim an unreviewed schema migration.
- Workload and observable query-plan/cost evidence for every supported engine;
  no invented numerical performance budget. Browser simultaneous connections
  and native thread transfer remain separate capabilities.

Clock/ID/fake-transport seams remain tied to their first executable consumer;
adding unused scaffolding now would not satisfy REQ-0034. Encryption, backups,
crash durability and remote CI remain explicit later gates; this indexing choice
cannot close them.

## Planned verification and rollback

Before code, named Gherkin cases must cover every supported operator, multiple
predicates, relation restrictions, optional/type errors, ordering/limit ties,
complete owned rows, equivalent outcomes across adapters and committed-change
publication/rollback. Each case requires genuine behavioral RED then GREEN.
Index measurements must distinguish initial construction, update cost and query
lookup work from result semantics; query parity alone is insufficient.

No code, dependency, persisted format or user data changes with this RFD.
Rejection/alternative selection is recorded in the discussion; retain its ID.
An adopted direction gets a new ADR linked to this RFD, then reviewed concrete
contracts/cases. Existing accepted ADR bodies remain unchanged.

## Discussion

- 2026-10-22 — Raised during the authorized resume after real browser persistence
  verification. Recommendation is engine-owned physical indexes. Operator's
  choice remains pending; no implementation or performance claim follows.

- 2026-10-22 — Operator approved the engine-owned recommendation by instructing
  continuation to finish storage. ADR-0030 accepts this direction; this RFD
  advances through ideation/discussion/publication to committed in the same
  recorded review. SPEC-0025/API-0003 and TC-0025 make the follow-on semantics
  concrete before implementation. This status records ownership approval, not
  passing conformance, performance or public reactivity acceptance.
