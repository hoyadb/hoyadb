---
id: TC-0017
title: Every documented local operation works with no network
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [NFR-0002]
tags: [test, offline-first, availability]
---

# TC-0017: Every documented local operation works with no network

## Purpose

Verify `NFR-0002`'s Must and Target — **zero** of hoyadb's documented local
operations fail when no network is available. This is the case that
establishes `OBJ-0001`, the project's first objective.

The operation set measured is exactly the one `NFR-0002`'s Meter names:
schema declaration and migration, one-shot query, live subscription, command
execute with optimistic apply, entity-diff write, and queue introspection.

## Method

Run against the in-memory engine with a fake transport whose every `push`
and `pull` rejects — not merely a transport that is slow or absent, so that
a silent dependency on connectivity would surface as a failure rather than a
hang.

## Scenarios

See `TC-0017-offline-operability.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `NFR-0002`.
