---
id: SPEC-0002
title: Default storage engine — SQLite-family behind the storage interface
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-22
parents: [ARC-0001, ADR-0003]
satisfies: [REQ-0030]
depends_on: [SPEC-0001]
tags: [storage, engine, sqlite]
---

# SPEC-0002: Default storage engine — SQLite-family behind the storage interface

## Purpose

Specify *a* default engine — not *the only* engine. A platform may bring a
different engine behind the same interface (`SPEC-0001`) without anything
above it changing (`ADR-0003`).

## Why SQLite-family

HoyaDB uses a SQLite-family default engine behind its pluggable storage interface (`ADR-0003`). Current engine selection and protected-open behavior are specified in `SPEC-0032` and `API-0010`.

## Responsibilities

- Durable on-disk storage.
- Executing queries against the declared schema, translated into its own
  native representation (`SPEC-0003`, `SPEC-0004`).
- Transactions — which is what makes `NFR-0003`'s strong durability case
  (state transition and local mutation committing together) hold on this
  engine rather than degrading to best-effort.
- Providing the encrypted-storage capability `SPEC-0014` relies on for
  at-rest encryption — an equivalent of SQLCipher for this engine
  specifically, handled inside this adapter and invisible above the
  interface (`REQ-0030`).

Everything above the interface — schema declaration and typing, relation
navigation, query subscriptions, sync — is engine-agnostic and does not
change based on this engine being in use versus another.

## Concrete engine contract

The selected adapters and dependency versions are recorded in workspace/bridge
manifests and lockfiles. `SPEC-0022` and `SPEC-0023` define native/browser SQLite
storage contracts. `SPEC-0032` and `API-0010` define protected engine selection,
key admission and diagnostics. Do not substitute another engine based on a
comparison or assume encrypted storage from a plain SQLite build.

## Traceability

- Parents: `ARC-0001`, `ADR-0003`.
- Satisfies: `REQ-0030`.
- Depends on: `SPEC-0001`.
- Verified by: `TC-0015`.

## Current implementation selection

SPEC-0022/0023 record the operator-approved native rusqlite and browser SQLite
backend foundations. SPEC-0025/API-0003 extend their common storage execution
with typed queries and committed changes; ADR-0030 assigns physical indexes to
each adapter. This selects SQLite for the current implementation rather than
redb; encrypted storage remains a security obligation, not an implemented claim.
The original ecosystem observations above are historical, not a new check of
external package state.
