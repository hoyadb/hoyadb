---
id: REQ-0046
title: Consumer bindings preserve general core semantics ownership and secure defaults
type: requirement
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [OBJ-0001, OBJ-0002, OBJ-0004]
depends_on: [NFR-0001, NFR-0002, NFR-0004, NFR-0005]
mitigates: [THR-0020]
tags: [phase-seven, bindings, compatibility]
---

# REQ-0046: Consumer bindings preserve general core semantics ownership and secure defaults

WHERE a platform SDK is supported, the system SHALL expose caller-defined schema,
authored migrations, all implemented query/relation/write/observation/command/
entity/pull/reset/status concepts by translating into the same Rust core. A fixed
test schema or independently implemented host query/sync/merge engine SHALL NOT
substitute for a general consumer binding.

The system SHALL retain secure key-required persistent opens and an explicitly
selected filesystem-free memory engine; keys SHALL use a separate binary channel.
Handles/scopes/events/merge sessions SHALL be owned, generation checked and
invalidated on close/reset/drop; host callbacks SHALL run without Rust borrows,
registry locks or global execution permits. Core validation and guarded atomic
commit SHALL remain authoritative across host awaits and cancellation.

The SDK SHALL expose persisted retry status/settings and deterministic testing
injections through the same production semantics, without payload-bearing safe
diagnostics or an alternate retry engine. Local operations SHALL not require a
successful network call. Consumer packages SHALL be installable outside this
checkout, with versioned API/ABI and truthful build/runtime support evidence.

Performance/memory/storage/boundary claims SHALL identify actual measured
artifact/platform/workload and distinguish baselines from numerical commitments;
numeric budgets SHALL follow measured evidence, not assumed examples.
Known configurable platform/wire/page/queue limits SHALL be documented alongside
measured complete-store footprint; no unmeasured capacity ceiling is promised.

RFD-0006's TC-0016 injected-ID prefix conflict remains an explicit pending operator
clarification; this requirement does not silently override API-0006's recovery IDs.
