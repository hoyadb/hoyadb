---
id: RISK-0001
title: The Rust-core-to-WASM-plus-native-bindings shape has no precedent in this project's research
type: risk
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [architecture, delivery, riskiest-assumption]
---

# RISK-0001: The Rust-core-to-WASM-plus-native-bindings shape has no precedent in this project's research

## Description

`ADR-0001` commits hoyadb to a single Rust core compiled to WebAssembly for
the browser and exposed through native FFI bindings on mobile and desktop.
**None of the eight systems studied for this project uses that shape for
its client core.** All eight ship a JS/TS core; cr-sqlite is the closest
adjacent case and it is a SQLite loadable extension, not a full client
core.

`legacy/VISION.md`'s "Riskiest assumptions" names this directly as one of two
compounding bets, and is explicit that it "was chosen on stated preference
for its performance ceiling, not because it's de-risked."

## Category

Delivery and technical risk (non-security), per
`ai-governance/framework/02-artifacts.md`'s RISK type.

## Impact if it materializes

`ADR-0001` is what gets reopened, and everything downstream of it moves:
`ARC-0001`, `ARC-0002`, `SPEC-0002`'s Rust-crate assumptions, and the
per-platform binding surface. Because this entire artifact tree and the
whole `legacy/docs/` tree were written against an unvalidated core shape, the
blast radius is the architecture layer, not one component.

Two specific unknowns sit inside this risk, both currently **unresolved**:

- **Reactivity across the core/platform boundary.**
  `legacy/docs/02-architecture/platform-bindings.md` calls this "the hardest open
  design question this split raises" — how live subscription notifications
  propagate from Rust-core state across FFI/WASM into a platform's own UI
  reactivity system efficiently. Named in `legacy/PLAN_DECISIONS.md` D0, not
  answered anywhere.
- **The command-handler callback mechanism.** `ADR-0025` fixes the
  ordering and atomicity contract for handlers, but the concrete FFI/WASM
  mechanics of an async callback across the language boundary are
  explicitly still open.

## Compounding factor

`legacy/VISION.md` makes a point this artifact repeats rather than softens: this
risk and `RISK-0002` are "a materially bigger combined bet than either
alone." Building a from-scratch relational, reactive, syncing database is
already large; doing it on an unprecedented core-language shape *and* on a
sync model with an unsolved correctness property compounds.

## Response

**Accepted, with a validation step scheduled — not mitigated.**
`ADR-0008` sequences an implementation roadmap whose *first step* is a
minimal spike proving a Rust core compiles to WASM and runs in a browser
and in at least one native binding. The roadmap runs after the
`legacy/PLAN_DECISIONS.md` questions are settled, not instead of settling them.

Also unresolved within this risk's scope:

- The concrete FFI mechanism per native platform (raw C ABI, UniFFI, or
  another generator) is implementation-time detail.
- The React Native binding candidate named in D0 (craby.rs) is explicitly
  **unconfirmed** and is not adopted by `ADR-0001`; it is a candidate to
  evaluate during the spike.

## Traceability

- Parent: `OBJ-0002`.
- Arises from: `ADR-0001`.
- Response scheduled by: `ADR-0008`.
- Affected: `ARC-0001`, `ARC-0002`.
