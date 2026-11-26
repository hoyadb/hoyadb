---
id: SPEC-0048
title: Atomic entity batches with temporary reference aliases
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-07
updated: 2026-11-07
parents: [ARC-0001]
satisfies: [REQ-0046]
depends_on: [API-0009, API-0011, SPEC-0031]
tags: [entity-sync, transactions, integration]
---

# SPEC-0048: Atomic entity batches with temporary reference aliases

Prepared parent/child creation needs one atomic entity
batch while the core remains the sole allocator of namespaced entity IDs.
This additive capability introduces no dependency, storage format, native symbol,
arbitrary entity ID, reservation or second durable queue.

## Contract and acceptance before implementation

The portable host adds `entityBatchAliased(writes)` returning
`{changes: Change[], aliases: Record<string,string>}`. The existing insert,
replace and remove shapes remain accepted in this new operation. Its additive
`patch` variant has `{op:'patch',table,id,fields,references?}`: it reads the current
staged row, merges only supplied fields, and records only supplied-field intent,
including same-value writes. This permits a partial migration edit and local
completion marker in one transaction without overwriting unrelated newer fields.
Inserts may
add a nonempty `alias`; inserts and replacements may add
`references: {field: earlierInsertAlias}`; patch accepts the same map. Remove keeps its existing shape.
Aliases only label configured entity inserts. Proposed IDs on configured inserts
remain rejected. Unconfigured ordinary inserts retain their existing proposal
semantics and may coexist with entity operations, but cannot declare an alias.

Reference declarations are explicit: they never rewrite ordinary String cells.
They must name declared Reference columns, cannot overlap supplied fields, and
must resolve to an earlier alias. A declared to-one relation must target the
alias producer's table. Duplicate, empty, missing and forward aliases reject.
Replace/remove targets remain existing IDs; they are not alias tokens.

The core resolves references during the existing staged batch, allocates IDs from
the same staged counter, and commits once through the existing SyncPlan path.
Only real IDs enter rows, recovery records, reference dependencies and envelopes.
Aliased parents precede children; existing parent confirmation/mapping gates
child delivery. Context is captured once. A failure publishes nothing and leaves
rows, ledger and allocator unchanged. Managed observations see the committed
batch, never partial parents or children.

The additive ABI1 operation `entity_batch_aliased` accepts exact
`{writes,context:Captured}`. Insert wire keys add `alias:String|null` and
`references:{String:String}` to the existing insert shape; replace/patch add only
`references`; remove is unchanged. The host supplies omitted null/empty defaults.
Its complete `{changes,aliases}` success frame is budgeted before mutation,
including generated-ID widths, rewritten reference fields, preserved patch fields
and alias keys. A cumulative encoded after-row lower bound rejects a definitely
oversized result while normalizing patches, before retaining unbounded expanded
snapshots; the final complete success serializer remains authoritative.
The existing operations, exact parsers, error semantics and limits are unchanged.

## Verification and rollback

[TC-0052](../tests/cases/TC-0052-entity-batch-aliases.md) requires a failing public
facade test before implementation, shared memory/SQLite atomicity and dependency
tests, binding budget tests, host forwarding tests and existing entity regressions.
Review shared client declaration additions separately before refreshing the
current source compatibility manifest. Historical snapshots remain unchanged.
Native/mobile artifacts must be rebuilt before consumers use this operation;
host tests do not certify mobile execution.

Rollback removes this additive operation and its caller use; no stored format
conversion is required. Preserve already committed rows and pending envelopes.
