---
id: ADR-0033
title: Correlated entity diffs atomic paged pull and explicit recovery reset
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0023, REQ-0024, REQ-0025, REQ-0026, REQ-0027, REQ-0029, REQ-0044]
depends_on: [RFD-0004, ADR-0032, ADR-0007, ADR-0015, ADR-0017]
tags: [entity-sync, entity-diff, bootstrap, reset]
---

# ADR-0033: Entity synchronization recovery

## Context and problem statement

The operator approved RFD-0004's reviewed P1–P6 bundle when requesting complete
roadmap entity sync. Earlier accepted artifacts leave repeated-record correlation,
entity dependencies, resolver shape, bootstrap and reset disposition explicitly
open. RecordId-only outcomes cannot disambiguate repeated writes or late replies.

## Considered options

- One discriminated durable queue, explicit per-write correlation, field metadata,
  atomic bounded page/checkpoint commits and caller-selected recovery discard.
- Serialize one diff per record and guess late replies from array position.
- Automatically rebase optimistic work or replay application command handlers.
- Stage an entire initial dataset and hide all partial query results.
- Discard recovery implicitly whenever resetting or switching membership.

## Decision outcome

In the context of entity synchronization and first-run recovery, facing ambiguous
repeated-record outcomes and undefined reset/bootstrap behavior, we decided for
**RFD-0004 P1–P6: opt-in generated patches with writeId correlation, a shared
durable queue and guarded comparison metadata, authoritative field resolution,
serialized atomic resumable pull, explicit progress/scope binding and total
reset with preservation by default**, and against guessed correlation, automatic
rebase or handler replay, unbounded hidden bootstrap and implicit loss of queued
work, to achieve exact recovery and backend authority, accepting conservative
pull retries/conflicts, retained tombstones and explicit destructive reset choice.

## Consequences

Diff envelopes still have no idempotencyKey. Their writeId/table/recordId tuple
is correlation, not a new server deduplication obligation. Backend record identity
must nevertheless make naturally idempotent entity retry safe. Existing command
keys/frozen bytes remain unchanged. Client and foreign hosts never choose equal
timestamp winners or perform membership authorization.

Pull applies only under its original checkpoint/generation/revision ticket.
An intervening data/comparison change requires a fresh request from the unchanged
checkpoint. Live optimistic overlap and unresolved mapping collisions reject
the entire page. Partial bootstrap is visible and explicitly not ready; each
page's data/checkpoint/progress is one transaction. Reset never reverses backend
effects and only explicit discard can remove live local recovery.

No runtime dependency, production data operation, encryption, new binding target,
independent concurrent store writer, push/merge/publication or later roadmap phase
is approved. Verification uses synthetic owned stores and the existing hosts.

## Confirmation

SPEC-0031/API-0009/TC-0034 require meaningful RED/GREEN, shared memory/SQLite/OPFS
behavior, physical faults, actual Swift/browser async and restart/reset lifetime
tests, regression gates and independent issue-focused review. Approval records a
policy decision, not a passing implementation or a remote CI result.
