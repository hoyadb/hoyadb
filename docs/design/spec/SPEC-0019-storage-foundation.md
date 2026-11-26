---
id: SPEC-0019
title: Establish the dependency-free storage storage foundation
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [ARC-0001, ADR-0003]
satisfies: [REQ-0034]
depends_on: [SPEC-0001]
tags: [foundation, storage, rust]
---

# SPEC-0019: storage storage foundation

Origin: operator instruction to start storage as far as possible while the
remaining Stage 0 browser and persistence gates stay open. This explicitly
authorizes the sequencing exception for preparation and a memory foundation;
it does not establish successful feasibility results or revise accepted ADRs.

## Scope and acceptance

Create a std-only root Cargo workspace containing `crates/hoyadb-core/` and
`crates/hoyadb-storage-memory/`, both `publish = false`. Explicitly exclude
`spikes/cross-platform`, including its native experiment, from the workspace.
Do not promote or depend on spike code. The memory adapter depends only on core;
core contains storage types/trait and has no host types or concrete adapter.

API-0002 defines the initial asynchronous typed CRUD subset. Memory storage uses
per-instance ordered maps and returns owned rows. TC-0019 verifies exact values,
ownership, isolation, typed errors, mutation preservation and table creation.
Only string, signed integer and boolean values are covered. This is not schema
declaration/validation, the complete primitive catalogue, query/filter execution,
subscription, transaction, persistent storage or database-opening functionality.

Before implementation, review and commit this spec, API-0002 and TC-0019;
observe meaningful failing behavioral tests, then implement the minimum. Run
workspace tests, formatting, Clippy and applicable documentation checks. Keep
spike tests separate. Pin Rust 1.98.1 with edition 2024, rustfmt, Clippy and
the `wasm32-unknown-unknown` target; installing this toolchain adds no database
runtime dependency. The operator's existing toolchains must still run the
isolated spikes separately; do not silently convert them into workspace members.

GitLab CI runs host tests, formatting, Clippy, a WASM library build and
traceability/RTM checks against the pinned governance submodule. A WASM library
build proves compilation only, not browser operation. The verified official
Rust image is `rust:1.98.1@sha256:a8a5f0a1e5fe7dfe1d352591e4a1c7dd2c08fd70475cae872cf3458ba0df0546`.
The root agent checked the [official release manifest](https://static.rust-lang.org/dist/channel-rust-1.98.1.toml)
and [official image tag](https://hub.docker.com/v2/repositories/library/rust/tags/1.98.1):
both returned HTTP 200; the release manifest matches installed rustc 1.98.1,
commit `48a229cea` (2026-09-01). Exact CI commands follow verified local commands.

TC-0016's first scenario is addressed only partially by Rust test functions
mapped through comments: typed writes/reads against memory storage. No filesystem
or platform API monitoring, complete database open, engine parity or full-product
acceptance execution is claimed. No external dependency or async runtime is added.

## Boundary assessment and rollback

Inputs are owned typed Rust values and borrowed table/ID strings; reads clone
owned data. No unsafe code, foreign pointers, identity, network, personal-data
fixtures or privilege boundary is introduced. STRIDE: invalid table/row state
is rejected by typed errors (tampering); other identity/audit/privilege controls
are outside this internal API. Synthetic bounded tests provide no resource-limit
or durable-storage guarantee. Existing native safety threats remain spike-scoped.

Rollback removes only this new workspace and its documentation links through a
reviewed change. Preserve the isolated spike and allocated artifact IDs. There
is no user data, migration, publication or shared-history rewrite to undo.
