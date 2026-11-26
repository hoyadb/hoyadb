---
id: TC-0052
title: Aliased entity batches preserve allocation atomicity and recovery
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-07
updated: 2026-11-07
parents: [TP-0001]
verifies: [SPEC-0048]
tags: [entity-sync, transactions, integration]
---

# TC-0052: Aliased entity batches

- A01: one parent and multiple children commit together using only real core
  IDs; returned alias mapping and stored reference fields agree. Ordinary text
  matching an alias remains unchanged. Mixed existing-ID replace/remove works.
- A02: parent entity creation is initially sendable while children await its
  mapping; confirmation with an authoritative remap rewrites their references.
  Reopen retains exact rows, dependencies and pending envelopes.
- A03: duplicate/empty/unknown/forward aliases, alias on a nonentity insert,
  wrong reference kind/target, overlapping fields and proposed entity IDs reject
  without rows, queue work, events or consumed allocator IDs.
- A04: late schema failure and queue capacity exhaustion roll back the whole
  batch; retry uses the original allocator position. Observation publishes the
  atomic final snapshot only.
- A05: complete binding reply over budget rejects before any durable mutation,
  including expansion from alias references and the returned alias map. Repeated
  tiny patches to large rows and large before-images stop at a cumulative byte
  lower bound, before retaining later expanded rows. A preceding replacement
  participates in later patch sizing.
- A06: portable JavaScript forwards exact normalized wire shapes, captures one
  context, returns the core mapping and follows existing mutation delivery.
  Existing entityBatch and proposed-ID rejection tests remain passing.
- A07: patch reads the current staged row and preserves fields changed remotely
  since the application's snapshot; only supplied fields enter its envelope.
  Same-value patch still queues. Patch plus local marker commits or fails together;
  missing rows reject and large preserved fields are included in reply budgeting.
  Successive patches read preceding staged replace/patch results; a patch after
  removal rejects the complete batch without publishing an intermediate row.

Execution results and limitations are recorded in the
[implementation report](../../../implementation/entity-batch-aliases.md);
these acceptance cases are not a claim of completed native/mobile validation.
