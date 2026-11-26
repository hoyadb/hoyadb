---
id: ADR-0003
title: Storage — one narrow pluggable interface, SQLite-family engine as the default
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [storage, engine, sqlite]
---

# ADR-0003: Storage — one narrow pluggable interface, SQLite-family engine as the default

## Decision

Use a narrow, pluggable storage interface, defaulting to a SQLite-family engine.
Schema, relations, reactivity and synchronization must depend on that interface
rather than on a concrete engine. Include an in-memory implementation for tests.

## Consequences

Adapter behavior can differ in queries, transactions and durability. Capabilities
and failure guarantees must therefore be explicit and tested. Encryption is an
adapter concern under ADR-0019; the generic interface does not gain crypto-specific
methods. Atomicity requirements remain governed by ADR-0016 and NFR-0003.
SPEC-0001, SPEC-0002 and their conformance cases define the storage boundary.
