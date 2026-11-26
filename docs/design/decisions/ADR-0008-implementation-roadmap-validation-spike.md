---
id: ADR-0008
title: Sequence the work as an implementation roadmap whose first step validates the Rust core shape
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [roadmap, process, spike]
---

# ADR-0008: Sequence the work as an implementation roadmap whose first step validates the Rust core shape

Source: `legacy/PLAN_DECISIONS.md` D0.

## Context and problem statement

`legacy/VISION.md`'s "Riskiest assumptions" flags that the
Rust-core-to-WASM-plus-native-bindings shape (`ADR-0001`) has no precedent
anywhere in the 8 systems researched for this project. The question raised
in D0 was whether to prove that shape with code *before* investing further
in documentation, or to proceed with the documentation pass and treat it as
a risk to keep in mind.

## Considered options

- **A one-off illustrative code spike first**, before more documentation —
  prove a Rust core compiles to WASM and runs in a browser and in at least
  one native binding, then resume.
- **Proceed with the documentation pass**, treating the unprecedented core
  shape as a risk to keep in mind rather than a gate.
- **An actual implementation roadmap with the Rust/WASM/native-binding
  validation as its first step**, sequenced *after* the open decisions are
  resolved.

## Decision outcome

In the context of sequencing hoyadb's work against its least-precedented
architectural bet, facing the risk of building documentation on an
unvalidated core shape, we decided for **an implementation roadmap whose
first step is the Rust/WASM/native-binding validation, built after the
`legacy/PLAN_DECISIONS.md` questions are resolved** and against both a one-off
pre-documentation spike and simply proceeding while noting the risk, to
achieve a real validation of the riskiest assumption without letting it
displace the decisions that must be settled first, accepting that the
documentation pass completes before any code exists to falsify it.

### Consequences

**Good**
- Gives the two riskiest assumptions a place to be tested rather than
  declaring them resolved (`RISK-0001`, `RISK-0002`).
- Ordering is explicit: finish `legacy/PLAN_DECISIONS.md`, then build the roadmap,
  then execute its first step.

**Bad**
- The entire `legacy/docs/` tree and this artifact tree are written against an
  unvalidated core shape. If the spike fails, `ADR-0001` is what gets
  reopened, and everything depending on it moves.
- **Open, carried forward rather than resolved.** D0's own answer names two
  hard parts and settles neither:
  - *What data lives inside Rust versus outside it, and how reactivity
    crosses that boundary.* `legacy/docs/02-architecture/platform-bindings.md`
    calls this "the hardest open design question this split raises" and
    leaves it unresolved.
  - *How developer-authored command handler code is called back into from
    inside the core* — "maybe a callback mechanism or something similar."
    `ADR-0025` fixes the ordering/atomicity contract for handlers, but the
    concrete FFI/WASM mechanics of an async cross-language callback are
    explicitly still unresolved.
- The React Native binding candidate (craby.rs) is named in D0 as
  **unconfirmed, needs checking** — not adopted here, and evaluated as part
  of the spike rather than decided in advance.

### Confirmation

Confirmed when the roadmap's first step actually runs a Rust core in a
browser via WASM and in at least one native binding. Until then this ADR
records the sequencing decision only; `ARC-0002` carries the boundary
design the spike would validate.

## Traceability

- Parent: `OBJ-0002`.
- Depends on: `ADR-0001` (the shape being validated).
- Risks: `RISK-0001`, `RISK-0003`.
