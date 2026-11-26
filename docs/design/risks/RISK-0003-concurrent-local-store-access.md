---
id: RISK-0003
title: Concurrent access to one local store from two tabs or windows is unaddressed
type: risk
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [architecture, concurrency, disclosed-gap]
---

# RISK-0003: Concurrent access to one local store from two tabs or windows is unaddressed

## Description

Two browser tabs, or two desktop windows, opening the same underlying file
or OPFS store at once is **unaddressed anywhere in the doc tree**.
`legacy/docs/02-architecture/platform-bindings.md` names this as a real, disclosed
gap and lists what is missing:

- no file locking,
- no leader election for which instance drives sync,
- no cross-tab or cross-window reactivity propagation.

That doc is explicit that this "is a routine scenario on exactly the
platforms this doc targets (web, Electron/Tauri desktop), not an edge
case."

## Category

Technical risk (non-security), architectural scope.

## Impact if it materializes

Several of hoyadb's core guarantees are stated per-process and would not
hold across processes:

- **`REQ-0011`'s handler serialization** guarantees only one command
  handler runs at a time. Two tabs each enforce that locally; neither
  excludes the other, so two handlers can interleave against one store.
- **`REQ-0018`'s queue state machine** could be advanced by two instances
  at once, with the same command sent twice or its state transitions
  interleaved.
- **`NFR-0003`'s durability guarantee** rests on engine transactions
  scoped to one connection; concurrent writers are outside what that
  guarantee covers.
- **`REQ-0007`'s subscriptions** would not see another instance's writes,
  since there is no cross-instance change propagation.

## Response

**Disclosed and accepted for now — no design exists.** This is not
mitigated by any requirement in this tree, and no ADR resolves it. It is
recorded here so it is visible as a live gap rather than discovered during
implementation.

It interacts directly with `RISK-0001`: whatever mechanism eventually
propagates reactivity across the core/platform boundary is likely the same
layer that would have to carry cross-tab propagation, so the two are best
designed together rather than separately.

Needs its own decision before implementation on web or desktop targets.

## Traceability

- Parent: `OBJ-0002`.
- Disclosed in: `legacy/docs/02-architecture/platform-bindings.md`.
- Affected: `ARC-0002`, `SPEC-0001`, `SPEC-0009`.
- Related: `RISK-0001`.
