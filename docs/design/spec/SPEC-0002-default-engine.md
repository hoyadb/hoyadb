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

Source: `legacy/docs/04-storage/default-engine.md`.

## Purpose

Specify *a* default engine — not *the only* engine. A platform may bring a
different engine behind the same interface (`SPEC-0001`) without anything
above it changing (`ADR-0003`).

## Why SQLite-family

`legacy/NEEDS.md` leans toward "SQLite or an equivalent embedded relational engine
per platform" without finalizing it. hoyadb picks a SQLite-family engine as
the default specifically because the core is Rust (`ADR-0001`): the crate
expected behind this default is `rusqlite`, which keeps `legacy/NEEDS.md`'s SQLite
lean while fitting the core-language decision.

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

## Ecosystem check

Checked in the same session as the platform-language decision; none of it
was in the original 8-source reference research, since none of those
sources are Rust-based:

- `sled`, a once-popular pure-Rust embedded database, is **explicitly ruled
  out** — described as in maintenance mode and unsuitable for new projects,
  with `redb` named as its own suggested successor.
- `rusqlite` shows active 2026 commit history.
- `redb`'s crates.io listing describes it as stable with a committed upgrade
  path, though older docs pages still carry beta-era warnings **worth
  re-verifying directly before depending on it**.

This check rules out `sled`; it does not change the default.

## Open

- **Whether `rusqlite` or a hand-rolled relational layer over `redb` is the
  concrete crate is not finalized.** The ecosystem check narrows the field;
  it does not pick between the two remaining options (`ADR-0003`).
- **The exact cipher and library for at-rest encryption on this engine — the
  SQLCipher-equivalent named above — is explicitly deferred to
  implementation time** (`ADR-0019`, `legacy/VISION.md`'s "Deferred to
  implementation time"). The floor is fixed; the pick is not.

## Traceability

- Parents: `ARC-0001`, `ADR-0003`.
- Satisfies: `REQ-0030`.
- Depends on: `SPEC-0001`.
- Verified by: `TC-0015`.

## Current implementation selection — 2026-10-22

SPEC-0022/0023 record the operator-approved native rusqlite and browser SQLite
backend foundations. SPEC-0025/API-0003 extend their common storage execution
with typed queries and committed changes; ADR-0030 assigns physical indexes to
each adapter. This selects SQLite for the current implementation rather than
redb; encrypted storage remains a Phase 6 obligation, not an implemented claim.
The original ecosystem observations above are historical, not a new check of
external package state.
