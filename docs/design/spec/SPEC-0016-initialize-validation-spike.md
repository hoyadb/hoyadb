---
id: SPEC-0016
title: Initialize the cross-platform validation spike
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-20
updated: 2026-10-20
parents: [ADR-0008, ARC-0002]
satisfies: [REQ-0001, REQ-0003, REQ-0034]
depends_on: [SPEC-0003, SPEC-0004]
tags: [spike, initialization, rust]
---

# SPEC-0016: Initialize the cross-platform validation spike

Origin: operator request to initialize the project following
`IMPLEMENTATION_PLAN.md`, followed by “go”. This is the first Stage 0 slice,
not a claim that the complete cross-platform spike is implemented.

## Need and scope

Establish the smallest executable Rust experiment required by
[ADR-0008](../decisions/ADR-0008-implementation-roadmap-validation-spike.md).
Keep it isolated under the proposed `spikes/cross-platform/` path. Do not add
a root production workspace or public packages.

This slice covers one hardcoded typed table, insertion, a single equality
predicate, and hermetic host tests. Browser/native bindings, persistence,
subscriptions and async handlers remain subsequent Stage 0 slices. Relations,
sync, migrations, indexing and encryption remain outside this slice. Passing
these tests does not establish platform support or satisfy an entire product
requirement.

## Intended behavior and acceptance criteria

The experimental table contains an explicit fixture ID, a string `status`,
an integer `total_cents`, and a boolean `paid`. These are spike fixtures,
not a production schema or an ID-generation contract.

- AC-1: Insertion preserves all typed fields in the matching query result.
- AC-2: Equality on `status` returns exactly matching records. Result order
  is unspecified; tests compare membership and field values.
- AC-3: Empty stores and predicates with no matching rows return no records.
- AC-4: Independently opened in-memory stores do not share data.
- AC-5: A fresh checkout can run the isolated crate's tests, formatting check
  and lint check using documented commands. No new runtime dependency is
  needed for this slice.

Use unique fixture IDs. Duplicate-ID policy, arbitrary table declarations,
other column types and query operators are deferred, not silently specified
by this experiment. The core must not introduce host-runtime types, networking
or telemetry. Use synthetic data only; the experiment is not production-ready.

## Named test cases (before test code)

Each scenario is a host Rust test under the proposed isolated crate's
`tests/` directory; none is yet implemented or executed.

```gherkin
Scenario: S01 — insert preserves typed fields (AC-1)
  Given a new store and a row with a unique ID, pending status, total 1200 and paid false
  When the row is inserted and status equals pending is queried
  Then exactly that row is returned with every field unchanged

Scenario: S02 — equality filters membership (AC-2)
  Given rows with unique IDs and statuses pending, complete and pending
  And the pending rows have totals 1200 and 3500 and paid values false and true
  When status equals pending is queried
  Then exactly the two pending rows are returned with all their fields unchanged

Scenario: S03 — empty store (AC-3)
  Given a new store
  When status equals pending is queried
  Then no rows are returned

Scenario: S04 — absent value (AC-3)
  Given a store containing only a complete row
  When status equals pending is queried
  Then no rows are returned

Scenario: S05 — isolated stores (AC-4)
  Given two independently opened stores
  When a pending row is inserted into the first store
  Then the second store returns no pending rows
```

AC-5 is checked by executing the documented commands from repository root
after implementation. Record the initial expected test failure and subsequent
passing output; a compiler error unrelated to the missing behavior is not a
behavioral regression proof. The full formal schema/query cases remain owned
by TC-0001 and TC-0002; these scenarios cover only this experiment's subset.

## Execution and rollback

After specification/case review and commit, write the smallest behavioral test,
observe its expected failure, then implement only enough behavior to pass.
Do not install toolchains or adopt dependencies without their required review.
Record exact executed commands in the implementation tracker.

Rollback removes only this slice's isolated crate and its documentation links
through a reviewed change. It requires no user-data migration, publication or
shared-history rewrite. Keep artifact IDs allocated if an artifact is retired.

## Verified local inventory

Observed command output during this session:

- `rustc --version`: 1.92.0-nightly (7ac0330c6 2025-09-25).
- `cargo --version`: 1.92.0-nightly (f2932725b 2025-09-24).
- `rustup target list --installed`: includes `wasm32-unknown-unknown` and
  `x86_64-apple-darwin`.
- `wasm-pack --version`: 0.13.0; `wasm-bindgen --version`: 0.2.104.
- `node --version`: v22.18.0.
- `swift --version`: Apple Swift 6.3.2; target x86_64-apple-macosx26.0.

Installed tools do not prove successful builds, dependency compatibility,
browser persistence or native callback support. No toolchain pin is selected
by this inventory; repeatable target/toolchain choices require build evidence.
