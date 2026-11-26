---
id: SPEC-0049
title: Opt-in durable entity aliases without implicit ledger conversion
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-07
updated: 2026-11-07
parents: [ARC-0001]
satisfies: [REQ-0046]
depends_on: [API-0009, API-0011, SPEC-0037, ADR-0038]
tags: [entity-sync, storage, compatibility, integration]
---

# SPEC-0049: Opt-in durable entity aliases

Monstera retains entity IDs in navigation and pending image uploads. An entity
create may be reconciled to an authoritative server ID. Existing normalization
uses retained command history, which can be evicted or explicitly forgotten;
it cannot serve those external references durably. A callback writing a second
store cannot atomically commit with reconciliation.

## Contract before implementation

Add a separate opt-in configuration operation for a fresh Hoya entity store and
a read-only entity ID resolver. Existing EntityConfig, EntityLedger and SyncLedger
public struct shapes, ordinary configuration, storage traits and default ledger
formats remain unchanged. Existing callers do not enable this capability.
An already configured format-2 or format-3 store cannot enable it implicitly.
Fresh means no prior entity ledger or live recovery: ordinary seeded legacy rows
and nonentity migration markers may already exist in application tables.
Reopening an opted-in store verifies its retained mode and configuration.

Only accepted mappings committed by the core's successful reconciliation enter
the durable alias authority. Mapping persistence, row/reference rewrites, recovery
updates and observer publication retain one atomic storage commit. Failed,
rejected or discarded reconciliation cannot introduce an alias. Identity
preserving mappings need no durable alias. Alias entries are independent of the
bounded command history and survive both trimming and explicit forgetting.

Resolution is table-aware and consults core state, never a JavaScript mapping
ledger or guessed server result. Known aliases resolve to their authoritative
IDs. An accepted canonical ID in one table is not reinterpreted through another
table's alias source with the same literal. Ordinary existing IDs and unresolved generated entity IDs retain their
existing meaning. The resolver does not imply that a row exists or that a
pending create has reached the backend. Callers must not send unresolved local
IDs to an endpoint that requires authoritative IDs.

## Persistence and compatibility boundary

The opt-in store has an explicit persisted ledger-format-4 marker. `entity:alias:`
packet IDs use the format-3 length-prefixed table/local identity encoding, with
canonical envelope format 2 and exact fields `table`, `local_id`, `canonical_id`.
The source is a positive allocated `entity:<namespace>:<16-hex-counter>` owned by
the ledger namespace and below the model's next allocator ID. Aliases are
immutable, table-bound and acyclic; distinct sources cannot claim the same
canonical target within one table. A source retained in recovery must match its
successful reconciled entity producer. Self mappings are not stored. It retains
format-3 identity metadata and adds independently bounded canonical alias
packets. Formats 1, 2 and 3 retain exact readers, serializers and behavior; no
open, reset, rebind or ordinary commit converts them. Unknown formats and mixed
packet formats continue to reject.

The implementation design uses private persisted-ledger bookkeeping containing
the existing public ledger and a separate alias sidecar. The private codec may
project the format-4 base header to format 3 solely for the unchanged base
validator, while retaining and validating all alias packets and the actual
persisted mode. It must never expose a decoded public SyncLedger that silently
forgets aliases. The existing public SyncLedger reader therefore continues to
reject format 4. Additive raw-inventory validation helpers allow bundled storage
adapters to validate the complete inventory without exposing mutable aliases.

Both full SyncPlan and validated Exclusive SyncDelta paths must enforce the
actual persisted format boundary, exact prior alias images, configuration,
canonical packet IDs, namespace ownership, bounded payloads and immutable
accepted mappings. A full plan introducing an alias must have an already
accepted, unreconciled entity producer in its expected inventory; existing
aliases remain valid after that producer is trimmed. The transaction journal records first alias images and
restores them on failure. Narrow alias changes must not clone or serialize the
entire alias inventory. Existing row protection and reconciliation rules remain
unchanged; ordinary Put writes into an application alias table are not enabled.

Explicit reset clears alias authority with the corresponding rows and recovery
state, retains format 4, and permits no alias leakage into a new account scope.
Aliases make a ledger nonpristine until that reset. Schema changes must not
strand aliases that refer to removed tables.

The existing format-2/3 converter rejects format 4 without writing. No downgrade
or conversion of an existing user's store is authorized by this feature.
An older binary cannot read an opted-in format-4 store; reverting the consumer
requires retaining a compatible reader. Preserve accepted ADR-0038 and all
historical fixture/report evidence.

## Acceptance and verification

[TC-0053](../tests/cases/TC-0053-durable-entity-aliases.md) covers eviction,
forgetting, restart, atomic failure, exact packet validation and legacy format
controls. A meaningful runtime failure must precede implementation. Public
declaration additions require the source compatibility checks; native artifacts
must be rebuilt before the application uses the new operations.
