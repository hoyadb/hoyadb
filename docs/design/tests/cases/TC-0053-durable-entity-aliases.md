---
id: TC-0053
title: Durable entity aliases survive history eviction and restart
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-07
updated: 2026-11-07
parents: [TP-0001]
verifies: [SPEC-0049]
tags: [entity-sync, storage, compatibility, integration]
---

# TC-0053: Durable entity alias acceptance

- D01: create, accept an authoritative ID, forget the producer, reopen, and
  resolve the old ID to the authoritative ID. Repeat with automatic zero-history
  eviction. Confirm the stored row and reference fields agree with resolution.
- ADR-0010: malformed or colliding reconciliation and injected storage commit failure
  publish neither alias nor remapped data; retry commits both. Exercise validated
  Exclusive delta and conservative full-plan adapters.
- ADR-0011: formats 1, 2 and 3 retain their exact readers, bytes, defaults and ordinary
  behavior. Existing public struct literal construction still compiles. Enabling
  durable aliases on an existing entity ledger rejects without conversion.
- ADR-0012: format-4 alias packets round-trip canonically; duplicate, conflicting,
  malformed, wrong-namespace, wrong-table, unknown-format and mixed-format packets
  reject. The public legacy reader and converter reject format 4 without losing
  or rewriting its alias packets.
- ADR-0013: trim and forget never remove accepted alias authority. Rejected/discarded
  outcomes never create it. Reset clears it with row/recovery state while retaining
  the chosen format; reconfiguration cannot leak aliases into another scope.
- ADR-0014: binding and JavaScript exposure delegate to the core; no wrapper mapping
  queue is introduced. Partial updates through an earlier ID use canonical core
  identity and retain ordinary error behavior. An accepted canonical ID that
  equals another table's alias source stays scoped to its original table.
- ADR-0015: fixed-size alias commits on large fixtures touch only their own alias
  packets; full-inventory open/reset costs remain explicit. Legacy scaling and
  guarded reconciliation checks remain enabled.

These acceptance cases do not authorize rewriting baseline evidence or altering
an application's existing store. Bind each execution result to current inputs.
