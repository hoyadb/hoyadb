---
id: ADR-0037
title: Serialize handlers per database while independent databases progress
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [OBJ-0003, REQ-0011]
depends_on: [RFD-0007, DEF-0035]
supersedes: ADR-0025
tags: [purpose-audit, commands, concurrency, isolation]
---

# ADR-0037: Per-database command serialization

## Context and problem statement

DEF-0035 records that an active or merely queued handler on database A makes
independent database B's writes Busy. ADR-0025 deliberately serialized handlers
across the application. That boundary lets one stalled host handler prevent
unrelated offline work. RFD-0007 presented both the existing guarantee and the
compatibility cost of replacing it.

## Considered options

- Per-database handler FIFO, preserving isolation within each database.
- Retain app-wide handler FIFO and scope only ordinary mutation permits. This
  improves plain writes but retains cross-database command blockage.

## Decision outcome

In the context of independent offline databases, facing cross-database blocking
from queued or stalled handlers, we decided for per-database handler FIFO and
against app-wide serialization, to achieve independent local progress,
accepting cross-database host-handler interleaving and application-owned
coordination of cross-database invariants.

The operator answered "can be per database if it's what 1 means" to the
RFD-0007 I-A question. Root clarified that this means the same logical database
remains serialized and independent databases may progress. This supersedes
ADR-0025's app-wide boundary; async handlers, read-your-writes, invisible staging
and one atomic commit are retained.

## Consequences

- Same-database requests remain FIFO. Direct writes, entity mutations, pull
  apply and reconciliation cannot bypass that database's active scope.
- Independent databases do not share a host queue or idle-mutation gate. Owner
  and request identities remain globally unambiguous; stale/cross-owner tokens
  cannot become valid in another coordinator.
- Native persistent databases require demonstrated exclusive ownership before
  an owner is published. Two owners of one file must not gain independent
  coordinators. The lease also excludes external readers until close.
- Browser store-scoped admission requires a supported-facade duplicate-owner
  rejection test. Until demonstrated, retain the conservative browser global
  coordinator; no cross-tab or multi-writer guarantee is introduced.
- Apps lose app-wide host-handler serialization. They must coordinate their
  own invariants spanning databases. There is no cross-database transaction.
- A nonterminating handler may still stall its own database. Existing explicit
  cancellation, abort and disposal remain available; arbitrary non-yielding host
  code cannot be preempted by the library.

## Confirmation and rollback

RFD-0007's reviewed detailed spec and named regression cases precede code.
Tests must suspend or queue a handler on A while B writes and executes, retain
same-database FIFO and atomicity, reject duplicate ownership and stale tickets,
and release admission on failure, close and drop. Deterministic tests establish
behavior, not a latency or throughput claim.

This decision is approved, not implemented evidence. No store is opened,
migrated, converted, reset or deleted. Rolling back the implementation restores
the old scheduling boundary without changing persisted application data.
