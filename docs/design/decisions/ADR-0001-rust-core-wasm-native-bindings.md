---
id: ADR-0001
title: Platform & language — one Rust core, compiled to WASM and exposed via native bindings
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [architecture, platform, rust, wasm]
---

# ADR-0001: Platform & language — one Rust core, compiled to WASM and exposed via native bindings

Source: `legacy/refined-vision/01-platform-language/alternatives.md` (Options A/B/C,
Option B chosen 2026-09-13), forked into `legacy/docs/02-architecture/overview.md`.

## Context and problem statement

hoyadb needs a core — data model, storage dispatch, reactive query diffing,
sync state machine — that runs on the browser, native mobile, React Native
and desktop, without the client's sync behaviour differing per platform
(`OBJ-0002`). `legacy/NEEDS.md`'s "Scope" asks for a generic core "portable across
at least mobile and web, independent of any specific platform runtime,"
wrapped per target. The choice of core language determines both the
performance ceiling and how much machinery sits between the core and each
platform.

## Considered options

- **Option A — TypeScript/JS core, portable across JS runtimes.** The
  best-precedented choice: 7 of the 8 systems researched (RxDB,
  WatermelonDB, PouchDB, and the client SDKs of Zero and PowerSync) ship a
  JS/TS core.
- **Option B — Rust core, compiled to WASM for web plus native FFI
  bindings for Swift/Kotlin/Node.** No precedent in this project's own
  reference set; cr-sqlite is the closest adjacent case, and it is a SQLite
  loadable extension, not a full client core.
- **Option C — Separate native reimplementation per platform** (Swift,
  Kotlin, JS, ...).

## Decision outcome

In the context of choosing hoyadb's core language and per-platform
delivery shape, facing the need for one behaviourally identical
implementation everywhere without a JS-thread performance ceiling, we
decided for **a single Rust core compiled to WASM plus native bindings**
(Option B) and against a TypeScript/JS core (Option A) and per-platform
reimplementation (Option C), to achieve one shared sync/storage behaviour
with no JS-thread or GC ceiling on any platform, accepting that this shape
has **no precedent anywhere in this project's own research** and carries an
upfront FFI/WASM binding cost on every target.

### Consequences

**Good**
- One implementation, so client/backend sync behaviour cannot drift between
  platforms — the failure mode Option C makes structurally likely.
- No JS-thread overhead or GC pauses for large local datasets.
  WatermelonDB's entire "lazy loading + off-JS-thread SQLite" design exists
  to work around exactly that ceiling rather than avoid needing to.

**Bad**
- The FFI/WASM binding layer is real, upfront, per-platform cost — accepted
  deliberately as the price of avoiding drift, not discovered later.
- Chosen on a stated preference for Rust's performance ceiling, **not**
  because the shape is de-risked by precedent. This is one of the two
  compounding bets `legacy/VISION.md`'s "Riskiest assumptions" names; tracked as
  `RISK-0001`.
- The React Native binding candidate named in `legacy/PLAN_DECISIONS.md` D0
  (craby.rs) is explicitly unconfirmed and is **not** adopted by this ADR.
- Propagating reactivity change notifications from Rust-core state across
  the FFI/WASM boundary into a platform's own UI-reactivity system is
  named in `legacy/docs/02-architecture/platform-bindings.md` as the hardest open
  design question this split raises. **Unresolved here** — see `ADR-0008`.

### Confirmation

Confirmed by `ARC-0001` and `ARC-0002` describing the layering and the
core/platform boundary, and by `ADR-0008`'s roadmap making a
Rust-to-WASM-plus-native-binding spike its first step. The concrete FFI
mechanism per platform (raw C ABI, UniFFI, or another generator) is
implementation-time detail this ADR does not fix.

## Pros and cons of the options

### Option A — TypeScript/JS core
- Good: best-precedented; 7 of 8 researched systems do this.
- Bad: inherits a real ceiling for large local datasets from JS-thread
  overhead and GC pauses.

### Option B — Rust core (chosen)
- Good: no JS-thread ceiling on any platform; one implementation everywhere.
- Bad: no precedent in the reference set; binding cost on every target.

### Option C — Per-platform reimplementation
- Bad: N implementations is N chances for sync-protocol behaviour to break
  silently. Every researched source that could have gone this route
  explicitly avoids it. Not seriously considered.

## Traceability

- Parent: `OBJ-0002`.
- Referenced by: `ARC-0001`, `ARC-0002`.
- Risk: `RISK-0001`.
