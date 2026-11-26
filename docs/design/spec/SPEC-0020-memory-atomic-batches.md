---
id: SPEC-0020
title: Apply ordered memory mutations atomically
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [ARC-0001, ADR-0003]
satisfies: [REQ-0034, NFR-0003]
depends_on: [SPEC-0001, SPEC-0019]
tags: [storage, memory, transactions]
---

# SPEC-0020: Memory atomic batches

Origin: operator instruction to continue the Phase 1 storage foundation.
Extend internal draft API-0002 with optional `TransactionalStorage: Storage`
and owned insert/replace/remove mutations. Existing `Storage` methods remain
unchanged. Engines without the optional trait make no batch guarantee.

## Behavior and acceptance

Apply mutations in supplied order across existing tables. Success publishes
all changes; the first ordinary `StorageError` stops processing, returns that
exact typed error and leaves every table unchanged. Each mutation observes
earlier mutations in the same batch. Missing-row removal is a no-op, unknown
table removal remains an error, and an empty batch succeeds unchanged.
No table creation, reads or schema changes are batch operations.

The memory adapter clones its current maps into a private candidate, uses the
existing CRUD behavior against that candidate, and assigns the candidate maps
back only after every mutation succeeds. This is a deliberately simple baseline
with O(total stored data) copy cost per batch, not a production optimization or
a new runtime dependency. Do not expose partial candidate state or duplicate
CRUD validation/error policies in a separate implementation.

TC-0020 T01–T07 must demonstrate ordered successful writes, complete rollback,
original first-error identity and no-op behavior. Append named Rust tests to
the existing memory `tests/contract.rs` to reuse its std-only executor; retain
all nine original CRUD cases. Review/commit documents before tests, observe
meaningful RED before implementation, then run tests, format/lint and the
existing target/documentation gates.

## Limits, boundary and rollback

This provides only the memory atomic-write subset of SPEC-0001/NFR-0003.
It proves neither durability nor atomic queue/local-state persistence; no queue,
observer, isolation between concurrent clients, cancellation, panic/abort recovery,
persistent engine or database transaction API is introduced. Browser/Stage 0
gates and the provisional Send-future compatibility question remain open.
Inputs remain owned typed Rust values with synthetic fixtures: no unsafe code,
foreign boundary, identity, personal data, I/O or external dependency is added.
Cloning temporarily increases memory use; no resource-exhaustion guarantee exists.
Rollback reverts this optional API and adapter/test additions without altering
the original CRUD contract, user data, publication or shared Git history.
