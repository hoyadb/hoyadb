# Phase 2 — public Rust data model execution evidence

Roadmap Phase 2 is implemented for the approved Rust scope: owned schema,
typed writes and generated IDs, composable one-shot queries, lazy relations
with managed many-to-many links, and atomic schema lifecycle. Memory and
native/browser SQLite use the same core. This completes the Stage 3 data-model
slice in IMPLEMENTATION_PLAN; the roadmap and that broader plan number their
stages differently. It does not complete reactivity, sync, security hardening
or packaged platform SDKs.

Implementation commit: `45e7c5c` on `feature/spec-0026-data-model`.
The operator approved RFD-0002's policies; ADR-0031 records them. Reviewed
SPEC-0026/API-0004/TC-0026 and REQ-0041/THR-0015 were committed in `52bb980`
before implementation, with recoverable-open clarification `2c26b16`.
Permanent regression descriptions preceded their tests/fixes in `0942e82`
and `a0054bb`. No runtime dependencies or lockfiles changed. Core and memory
remain dependency-free. No existing user database was opened or migrated.

## What is implemented

`hoyadb_core::model` owns normalized schema definitions, implicit Reference keys,
the six portable value kinds and the API-0004 public Database/QueryBuilder.
Default generated IDs use a persisted `local:<counter>` sequence; an injected
generator supplies exact IDs without silent retries. Managed row versions are
persisted and stripped from public owned rows and committed changes. Inserts
and the default-ID counter commit together. A failed batch consumes no
committed counter state; external generator side effects are not rolled back.

To-one, has-many and many-to-many queries defer all storage reads until fetch.
Explicit queryable through tables identify named source/target relations;
link/unlink manage membership, payload agreement and duplicate removal.
Target filtering, ordering and limit follow deduplication. The existing engine
query interface executes ordinary predicates, direct to-one joins, ordering and
limit; core does not replace it with lifecycle table scans.

Reserved logical `__hoya_model` holds strict canonical schema, application
version and the positive ID counter. Application rows hold hidden Integer
`__hoya_schema_version`; `__hoya_` is reserved. Length-framed UTF-8 canonical
encoding includes the complete normalized declaration and relation operands.
It rejects malformed/noncanonical metadata rather than assuming a fresh store.
SQLite keeps its private version-1 four-table DDL, marker, tags and indexes.

Opening an equal version requires an identical definition. Empty unversioned
stores initialize at a positive u32 version; populated Phase 1 stores adopt
only as fully validated version 1. Complete authored ascending chains support
AddTable, DropTable, AddColumn, DropColumn, RenameColumn, Backfill and
SetRelations. All endpoints, rows, metadata and versions commit as one plan.
Missing complete chains reset only known supported queue-free logical stores,
retaining the counter and physical format. Downgrades, schema drift, malformed
or future metadata, unknown reserved bookkeeping/queues and invalid migration
descriptors reject unchanged. An authored-step failure never becomes reset.

Memory validates a candidate before swapping state. SQLite compares expected
metadata and executes the entire plan in one real transaction. The recoverable
open returns the owned engine after failure for inspection/retry. Public async
futures are owner-affine; storage traits retain their Send contract. There is
no new runtime, shared-connection or concurrency guarantee.

## Source and acceptance mapping

The historical Gherkin behavior is implemented by explicit shared Rust
assertions, not by claiming a Gherkin runner executed Markdown/feature files.

| Source behavior | TC-0026 cases and executable evidence |
| --- | --- |
| SPEC-0003 / DM-0001: declarations, primitive kinds, implicit fields, reserved names | D01/D02/D03/D13/D14 in `tests/conformance/data_model.rs`; core schema/metadata unit tests |
| SPEC-0003: three lazy relation kinds and managed through mappings | D05/D06; read-counting adapter proves construction makes no read |
| SPEC-0004: seven operators, AND, direct to-one paths, ordering/limit | D04/D05/D06; exact expected rows, all six types, optional/null and invalid-query partitions |
| SPEC-0005: equal-version, adoption, seven authored primitives, rollback/reset | D07–D12; S01–S05 in `tests/conformance/schema_storage.rs` additionally prove lifecycle candidate validation and stale-state refusal |
| Physical SQLite failures and private-format preservation | D15: five native tests in `crates/hoyadb-storage-sqlite/tests/model_cases.rs`; actual browser SQL abort/retry fixtures |
| Independent owner restart and persisted counter | D16: `tests/conformance/model_restart.rs`, native process test and real browser worker/page restart |
| Original TC-0001 scenarios | D01/D02/D03/D05/D06 |
| Original TC-0002 scenarios | D04/D05/D06 |
| Original TC-0003 scenarios | D02/D07/D09/D10/D11/D12/D16 |
| DEF-0002 / TC-0027 | Core shared-reference alias regression plus D14 on all adapters |
| DEF-0003 / TC-0028 | Standalone shared unselected-descriptor regression on all adapters |

D01–D14 plus S01–S05 and TC-0028 are **20 shared Rust cases per adapter**.
Each fixture checks complete expected values/state, not only row counts or
agreement between implementations. The original storage regressions remain.

## RED, correction and review

- Core declaration tests failed against missing normalization/codec behavior;
  metadata tests failed before strict decoding. Shared lifecycle S01–S05 failed
  against both adapter scaffolds before candidate/transaction implementation.
- Public initialization/IDs/adoption cases failed before Database open/write
  behavior. Query/relation cases failed before fetch/link/unlink implementation.
  Authored-chain and missing-chain reset tests failed before lifecycle planning.
  The successful D09 chain prevents blanket rejection from satisfying D10.
- Native D16 first failed because the process fixture had no `model` scenario;
  it now invokes separate writer and reader processes. Reader assertions inspect
  committed migrated rows, relations, canonical metadata/version and counter
  before inserting the next actual generated ID.
- The real browser baseline passed before the new model request returned
  `BackendFailure: stale or disposed handle` from the missing dispatcher branch.
  [Browser RED notes](../../spikes/storage-browser/RED.md) and the
  [recorded observations](phase-two-browser-results.json) retain that failure.
  An earlier unchanged-baseline timeout was rerun idle; the 20-second request
  timeout and assertions were not weakened.
- Review found two aliases could share a key but target different tables.
  TC-0027 reproduced this failure before the normalizer began checking the
  complete target/optionality mapping. Matching aliases remain valid.
- Review also found invalid nested names in an unselected migration descriptor
  could pass open. TC-0028 reproduced the defect before all descriptors gained
  context-independent nested-name/structural checks. Selected edges still
  validate their context-dependent source/endpoint constraints.
- A separate agent reviewed schema codec, query/relation handling, ID counters,
  migration/reset boundaries and both adapters. It reported no additional
  concrete findings beyond the two recorded defects; it did not execute tests.
  Root inspected the final implementation/test diff and preserved the existing
  user CI runner tags outside task commits.

## Observed verification — 2026-10-22

Host: x86_64 macOS 26.7.1, Rust `1.98.1 (48a229cea 2026-09-01)`.
The final workspace run passed **122 tests**, zero failures/ignored:
9 core, 46 memory conformance, 1 memory index-cost, 46 SQLite conformance,
5 native model physical-fault, 4 native process, 6 SQLite-specific and
5 typed SQLite-specific tests. The recorded
[host output](phase-two-host-tests.txt) contains the exact suite results.

Executed successfully:

```sh
cargo fmt --all --check
cargo test --workspace --offline --locked
cargo run -p hoyadb-storage-memory --example data_model --offline --locked
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo build -p hoyadb-core -p hoyadb-storage-memory --target wasm32-unknown-unknown --offline --locked
CC_wasm32_unknown_unknown=/usr/local/opt/llvm@22/bin/clang \
AR_wasm32_unknown_unknown=/usr/local/opt/llvm@22/bin/llvm-ar \
sh spikes/storage-browser/check.sh
```

The runnable example prints `Phase 2 example: 1 order; schema version 2`.
It exercises generated IDs, lazy relations, an authored migration and a
filtered/ordered/limited public query. CI's workspace job now executes it.

The actual browser run used Paseo 0.12.0 / Electron 44.2.0 /
Chrome 152.0.7977.76 at secure loopback port 8769, crossOriginIsolated false.
All **19 first-page and 12 reload-page report entries passed**. The first-page
model entry executes all 20 shared Rust cases, including both corrected
regressions. D15 uses real SQL trigger aborts after successful prefixes for
initialization, adoption, insertion, migration and reset, followed by recovery.
It is not labeled as an actual browser commit failure; the native held-reader
fixture separately proves real COMMIT failure and counter-preserving retry.

D16 reads exact managed data after worker restart and full page reload before
fixture insertion, then allocates the actual next ID `local:3`. Actual OPFS
deletion is blocked while the VFS pool owns handles, succeeds after acknowledged
shutdown and precedes worker termination. Successful namespace
`hoyadb-spec23-76c91afa-5a7e-4178-912b-164966038730` was removed. Failed harness
namespaces were retained; no origin-wide deletion occurred.
The [complete browser results](phase-two-browser-results.json) and
[source/generated artifact hashes](phase-two-source-sha256.json) identify the
executed inputs; generated JS/WASM remain build outputs outside Git.

The exact pinned CI image
`rust:1.98.1@sha256:a8a5f0a1e5fe7dfe1d352591e4a1c7dd2c08fd70475cae872cf3458ba0df0546`
also passed from read-only source with fresh ephemeral Cargo caches/targets.
It installed the configured Debian Clang/LLVM build tools, ran workspace tests,
example, fmt/Clippy and core/memory/SQLite/browser WASM builds, and exited zero
with `PHASE2_CONTAINER_GREEN`. See the
[exact commands](phase-two-ci-commands.sh) and
[complete container output](phase-two-ci-output.txt). This is local container
execution; remote GitLab CI was not observed.

A fresh `git archive 45e7c5c` extracted under a task-owned target directory
passed all 122 workspace tests, the runnable example and format checks. It had
no generated browser package and used a fresh local target directory.
`trace_check.py docs/design` reported **0 violations** and
`rtm_gen.py docs/design --check` reported the RTM up to date.

## Remaining roadmap boundaries and rollback

Live observations, incremental maintenance, commands/transport/queues, server
ID remapping, encryption, backup exclusion, multi-client access and packaged
TypeScript/Swift/Kotlin APIs remain later phases. The browser is the approved
experimental single-owner OPFS path; this work proves ordinary SQL rollback
and owner restart, not forced crash/power-loss durability, quota/eviction or
cross-browser support. No new performance budget or release/security claim is
made. Unknown future bookkeeping is refused rather than erased.

Rollback is a reviewed source revert plus a compatible reader, preserving
database files and allocated artifacts. A raw Phase 1 reader does not enforce
Phase 2 lifecycle protections. Do not downgrade metadata, clear an origin or
invoke missing-chain reset to undo this implementation. No push, merge,
publication or production data migration was performed.
