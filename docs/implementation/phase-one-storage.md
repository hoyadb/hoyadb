# Phase 1 storage — SPEC-0025 execution evidence

This report covers roadmap Phase 1's internal storage foundation: typed queries,
adapter-owned indexes and atomic committed changes in memory, native SQLite and
the supported single-worker browser SQLite path. It extends the historical
[initial foundation](phase-one-foundation.md) and [browser persistence](browser-persistence.md)
reports. Acceptance is TC-0025 P01–P13 plus the unchanged API-0002 cases.

## Decisions and implementation

The operator approved engine-managed indexes in RFD-0001; ADR-0030 records that
decision. Reviewed specifications, API-0003 and TC-0025 were committed in
`8bf2a55` before implementation. No runtime dependency was added in this slice.
Core and memory still have no external runtime dependencies. Implementation
commit: `6b4820a` (specification/review checkpoints `8bf2a55` and `040ca79`).

The supplied storage schema validates exact value kinds, required/optional
fields and direct to-one paths. Queries support Eq, Ne, Lt, Le, Gt, Ge, In,
implicit AND, one order field and limit, with deterministic ascending own-id
ties. Values include string, i64, finite f64, boolean, millisecond timestamp,
reference and Null. Optional omission keeps its stored shape. Integer and
timestamp bridge payloads use decimal text; no i64 is narrowed through JS Number.

Memory maintains its own field lookup maps on every raw and typed write. It
uses equality/In candidates when operands exclude Null; other predicates and
ordering can scan. SQLite executes bound predicates, joins, order and limit in
SQL, using additive text/numeric indexes. The private v1 table DDL and old tags
remain unchanged. Finite floats use checked REAL/integer decoding in the existing
numeric-affinity payload column; nonrepresentable integer payloads reject.

Typed batches return owned before/after snapshots only after successful commit.
Repeated ids retain mutation order; identical replacements and absent removals
emit no entry. Memory clones its state/indexes per batch; SQLite uses a real SQL
transaction. Raw CRUD remains available without schema validation or change
publication. Returned source rows validate; nonreturned and related rows are
not exhaustively scanned.

## RED, review and regression

- Core's three validation/finite-float tests failed against stubs, then passed.
- Shared memory typed assertions failed against query/batch stubs (nine failures,
  with invalid-query rejection already passing), then all ten passed.
- Native P11/P12 fixtures failed against typed stubs; the typed process fixture
  failed for its unsupported scenario before implementation. The same shared
  P01–P10 assertions subsequently passed against SQLite.
- Real browser baseline cases passed before the new typed cases failed on the
  unsupported value discriminator. The new assertions subsequently passed.
- Independent review found missing physical relation-table validation in SQLite
  and inconsistent treatment of wrong-kind unchecked fields. Contract/case
  clarification was committed in `040ca79` before shared regression tests and
  adapter fixes. Consulted invalid fields match no predicate and sort as Null;
  complete returned source rows still validate.

## P12 bounded workload

Observed on x86_64 macOS 26.7.1, Rust 1.98.1, unoptimized test builds,
after the final parity fixes in `6b4820a`. These
synthetic timings include test/validation/allocation costs and are observations,
not latency budgets or fair cross-runtime speed rankings. Rerun:

```sh
cargo test -p hoyadb-storage-memory --test index_cost --offline --locked -- --nocapture
cargo test -p hoyadb-storage-sqlite --offline --locked p12_actual -- --nocapture
```

Both host fixtures use 256 then 2,048 rows, exactly one numeric/text match
(`00000042`, number 42, text `key-00000042`), and 25 repeated lookups per field.
Memory's observed candidate count is **1** at both sizes. Ordering visits all
256/2,048 candidates. SQLite's actual plan reports `SEARCH` through covering
`__hoya_fields_numeric` / `__hoya_fields_text`, row primary-key lookup and a
temporary B-tree for ordering; no claim that every query avoids scans follows.

| Observed work | Memory 256 / 2,048 | Native SQLite 256 / 2,048 |
| --- | --- | --- |
| First population/index construction | 2.362 ms for 256 (incremental, combined) | 17.916 ms raw population; 3.525 ms populated index rebuild on open |
| Add unrelated rows | 1.212 ms empty typed batch / 30.330 ms growth | 0.038 ms empty typed batch / 154.513 ms growth |
| 25 numeric lookups | 0.269 / 0.282 ms | 7.064 / 7.139 ms |
| 25 text lookups | 0.267 / 0.276 ms | 7.010 / 6.674 ms |
| Typed indexed replacement | 0.996 / 13.938 ms | 1.898 / 1.887 ms |
| Raw indexed replacement | 0.016 / 0.027 ms | 1.407 / 1.622 ms |
| Failed batch rollback | 0.967 / 11.835 ms | 0.806 / 0.828 ms |
| Descending order and limit 3 | 0.350 / 3.112 ms | 1.048 / 5.156 ms |
| Reopen and index check | Not persistent | 0.504 / 0.467 ms |

Tests assert complete selected rows, update old/new memberships, restore through
raw writes, fail a batch after a valid indexed prefix, and confirm preserved
membership. Native also closes/reopens and checks the real plan again. SQLite
P11 adds separate process restart, compatible indexless v1 opening, strict
malformed-value fixtures, aborting SQL triggers and an actual failed COMMIT.

## Verification status

The final host workspace run passed **70 tests**: three core validation tests,
27 memory cases and 40 SQLite cases. `cargo fmt --all --check`, workspace
Clippy with `-D warnings`, and the explicit core/memory wasm32 build passed.
The unchanged isolated host spike's 11 tests and native spike's eight Rust
cases also passed. Traceability reports zero violations, RTM is current and
`git diff --check` passes.

**TC-0025 P01–P13 pass; roadmap Phase 1 storage is complete at the specified
internal acceptance boundary.** The following are executed results, not release
or remote CI claims.

The real browser run passed all 16 first-stage and ten post-reload report
entries. This includes shared P01–P10 (with review regressions), P11 physical
fixtures, worker/page reads before insertion, exact new primitive payloads and
L01/L02. An actual retained-handle deletion error preceded acknowledged VFS
shutdown; namespace deletion then succeeded while the worker was still alive.
The successful test namespace was removed. Full observations, source/generated
SHA-256 hashes and tool versions are in
[browser results](phase-one-browser-results.json).

Browser runtime: Paseo 0.12.0 / Electron 44.2.0 / Chrome 152.0.7977.76 at secure
loopback, without cross-origin isolation or security changes. The final P12
workload matches host populations/complete rows and 25 numeric/text lookups.
At 256 / 2,048 rows, repeated numeric and text lookups each took 12 / 10 ms;
typed update 4 / 3 ms, raw update 3 / 3 ms, rollback 2 / 2 ms, ordering 2 / 8 ms,
and reopen 1 / 0 ms. Initial raw population took 39 ms, populated index build
6 ms, then added population 1 / 393 ms. Actual plans use both covering indexes
and retain temporary ordering work. Browser measurements use millisecond
`Date.now` resolution; zero means below that resolution, not zero cost.

P13 ran the configured pinned Rust image
`rust:1.98.1@sha256:a8a5f0a1e5fe7dfe1d352591e4a1c7dd2c08fd70475cae872cf3458ba0df0546`
locally with source mounted read-only and ephemeral target/cache directories.
The base image had no WASM C compiler. Installing Debian Clang/LLVM 19.1.7
inside the temporary container supplied it; the new browser compile CI job
records that preparation. All locked fetch, workspace format/test/Clippy,
core/memory WASM and browser adapter/harness format/Clippy/WASM build commands
passed. The [container output](phase-one-ci.log) ends in `P13_CONTAINER_GREEN`.
Local host `check.sh` also passed using LLVM 22.1.8 and wasm-bindgen 0.2.104.
The container compiles WASM; actual browser execution is the separate evidence
above. No new runtime dependency or host-global compiler change was needed.

A source-only archive of staged tree
`887cdd91c9f00ae75aa8aa1a0283e8ef1eb97e61` passed 70 tests, format and Clippy with
a fresh target directory. Its first WASM attempt selected Homebrew Rust, which
lacked WASM std despite Rustup having the target installed. Explicitly selecting
the checked Rustup 1.98.1 compiler made that build pass. Ensure the Rustup
compiler is selected when reproducing on a host with multiple Rust installs.
The configured-container run independently passed all selected WASM builds.

## Limits and next phase

This is an unpublished internal storage API. Supplied-schema storage validation
does not deliver Phase 2's public schema builder, relation navigation, migration
chains or reset. Returned changes are Phase 3 input, not subscriptions or
incremental query maintenance. Sync/queue crash durability, encryption,
backup/export protection, packaged bindings and platform breadth remain later
work. Browser support is one dedicated worker and one active connection with
acknowledged pool shutdown; no multi-tab, WAL, power-loss or concurrent-writer
guarantee is established. NFR numerical targets remain unset.

Remote GitLab CI, merge and release remain separate gates. No push, merge or
publication occurs here. Existing user runner-tag edits are preserved separately.
Reverting implementation preserves databases, but older readers reject new
value tags; it is not a destructive data rollback.
