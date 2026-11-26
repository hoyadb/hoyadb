---
id: RISK-0005
title: Bootstrap / initial sync for a fresh install is unspecified
type: risk
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-24
parents: [OBJ-0003]
tags: [sync, disclosed-gap]
---

# RISK-0005: Bootstrap / initial sync for a fresh install is unspecified

## Description

Everything the sync design specifies is **incremental catch-up from a
checkpoint**: `REQ-0026`'s membership-scoped pull, the transport's
`pull(since)` operation, and the opaque checkpoint token. A fresh install
with no local data at all is not addressed anywhere.

`ARC-0001` names it directly as "a real, disclosed gap, not
an oversight — needs its own decision before implementation," and lists
what is missing:

- pagination for a large first sync,
- how reactivity behaves while that first sync is still in flight,
- how a large bootstrap interacts with the "small, mobile-appropriate
  dataset" framing.

## Category

Technical risk (non-security), sync scope.

## Impact if it materializes

Every client's very first run hits this path, so it is not an edge case:

- **Unbounded first pull.** `REQ-0026` describes pulling changes since a
  checkpoint; with no checkpoint, the natural reading is "everything in
  scope," with no pagination designed. That collides directly with
  `NFR-0004`'s deliberately mobile-appropriate footprint.
- **Reactivity during bootstrap is undefined.** `REQ-0007` and `REQ-0008`
  specify subscription behaviour against local data; what a subscription
  emits while the local store is still filling — nothing, partial results,
  or results that churn as data streams in — is unstated. An application
  cannot distinguish "no matching orders" from "orders haven't arrived
  yet."
- **No progress surface.** `REQ-0021`'s queue introspection covers
  outbound writes, not inbound bootstrap progress, so an app cannot show a
  meaningful first-run loading state.

## Response

**Disclosed and deferred — no design exists and no ADR resolves it.**
Recorded here so that a gap affecting 100% of first runs is visible in the
artifact tree rather than only in a prose "What's still open" section.

This needs its own decision before implementation. It interacts with
`NFR-0004` (footprint) and `RISK-0004` (indexing, since a large initial
write burst is exactly where lookup cost bites).

## Traceability

- Parent: `OBJ-0003`.
- Disclosed in: `ARC-0001`, `ADR-0005`.
- Affected: `REQ-0026`, `SPEC-0012`, `SPEC-0013`, `NFR-0004`.

## entity sync response

The original absence of a decision is resolved by operator-approved RFD-0004,
adopted ADR-0033 and API-0009, committed before implementation. Bootstrap now
uses bounded pages, atomic checkpoint/data/comparison commits, visible partial
data and explicit readiness. TC-0034 E11–E15 requires native process interruption,
real browser reload and stale/conflict preservation. The risk remains active
for integration-specific dataset size, server pagination consistency, membership
authorization and retained comparison/tombstone growth; no universal footprint
or production-server guarantee is inferred from synthetic acceptance tests.
