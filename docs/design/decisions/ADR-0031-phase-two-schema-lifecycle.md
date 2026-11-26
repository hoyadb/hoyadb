---
id: ADR-0031
title: Explicit relations and atomic managed schema lifecycle
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [REQ-0001, REQ-0002, REQ-0006, REQ-0033, REQ-0041]
depends_on: [RFD-0002, ADR-0002, ADR-0009]
tags: [schema, migration, relations, phase-two]
---

# ADR-0031: Phase 2 schema lifecycle

## Context and problem statement

The operator requested full roadmap Phase 2 and explicitly approved RFD-0002's
recommended policies. Accepted earlier ADRs leave relation visibility and
unsupported schema-state recovery open; core and both storage engines need one
concrete portable lifecycle without risking implicit data deletion.

## Considered options

- Explicit application-queryable through tables with library-managed relations,
  separate canonical schema metadata and validated atomic schema transitions.
- Implicit through tables with new visibility/ownership rules.
- Guessing migrations or resetting unknown, downgrade or queue-present stores.
- Changing SQLite's private format/version to store application schema state.

## Decision outcome

In the context of Phase 2's portable data model, facing incomplete relation and
schema lifecycle policies, we decided for **explicit queryable through tables,
separate `__hoya_` application metadata and atomic validated chain-or-reset for
supported queue-free forward upgrades**, and against implicit join-table
generation, inferred migrations, resetting unknown/downgrade/queue state or
repurposing private SQLite format versions, to achieve predictable ownership
and preservation under failure, accepting explicit join declarations, lifecycle
scan costs and rejection of unsupported states.

To-one relations declare implicit Reference keys; named has-many/many-to-many
relations provide lazy navigation and managed link/unlink. Public declarations
cannot use the established reserved `__hoya_` namespace. Application versions,
canonical definitions and ID counters persist separately from library/private
format versions. Each persisted row has managed schema-version bookkeeping.

Complete authored upgrades commit atomically; any step/endpoint/commit failure
preserves the original state. Missing complete forward chains retain ADR-0009's
reset policy only for known supported queue-free stores. Downgrades, same-version
drift, malformed/future metadata and unknown bookkeeping reject unchanged.
Populated Phase 1 stores adopt only as fully validated version 1; empty stores
initialize at any positive declared version. No actual user store is migrated
by this task; implementation and verification use owned synthetic fixtures.

## Consequences

The existing SQLite four-table v1 DDL, marker and old value tags remain intact.
Explicit authored endpoints and local scans make migrations repeatable, with
whole-state atomicity instead of guessed transitions. Earlier accepted ADR bodies
remain unchanged. A future queue must extend the supported metadata and reset
policy explicitly; this decision cannot decide recovery for an external queue.

Rust supplies the common public library API in this phase. Published platform
SDKs, subscriptions, sync, encryption and crash durability retain their roadmap
phases. Reverting implementation preserves files but requires compatible readers
for managed stores; deletion or metadata downgrade is not a rollback mechanism.

## Confirmation

API-0004/SPEC-0026 define exact operations. TC-0026 covers original
TC-0001/0002/0003, strict metadata, namespace barriers, expected-state checks,
atomic failure recovery and independent native/browser restart observations.
The operator's explicit approval of RFD-0002 is recorded in its discussion.
