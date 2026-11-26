# Phase 1 foundation — initial storage subset

Historical slice evidence follows. The current typed-query, index and committed
change extension is recorded in [Phase 1 storage](phase-one-storage.md); its
verification supersedes the primitive/query limitations below.

The operator requested starting Phase 1 while Stage 0 browser/persistence gates
remain unresolved. This is the common initial slice of roadmap Phase 1 storage
and implementation-plan Stage 1 foundation. It does not close the earlier gates
or establish a complete production database.

The root workspace began with dependency-free `hoyadb-core` and
`hoyadb-storage-memory`. SPEC-0022 extends it with the native SQLite adapter;
all packages are unpublished, and core/memory remain dependency-free. Core owns
the typed storage contract; the memory adapter depends on core. Neither imports or
promotes spike code. All three spike crates remain independent Cargo projects.

The adapter supports explicit table creation and async insert/get/replace/remove
with owned rows and typed errors. Only string, i64 and boolean values are
implemented. Table/column schemas, other primitives, predicates, read transactions,
subscriptions, encryption, consumer APIs and production bindings remain
deferred. Native persistence is the separate SPEC-0022 extension below.
Do not use this subset as the full SPEC-0001 engine contract.

## Build and verification

Rust is pinned to 1.98.1 with rustfmt, Clippy and `wasm32-unknown-unknown` in
`rust-toolchain.toml`. A Rustup installation obtains those components when the
workspace is first used. Use `cargo fetch --locked` once to populate approved
locked dependencies;
subsequent offline builds/tests need no registry access. The baseline WASM command selects core/memory. The browser SQLite target also
requires a WASM C compiler and uses the separate SPEC-0023 check script.

Run from repository root:

```sh
cargo fetch --locked
cargo fmt --all --check
cargo test --workspace --offline --locked
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo build -p hoyadb-core -p hoyadb-storage-memory --target wasm32-unknown-unknown --offline --locked
python3 ai-governance/tools/trace_check.py docs/design
python3 ai-governance/tools/rtm_gen.py docs/design --check
```

The CRUD contract tests are named M01–M09 and reference
[TC-0019](../design/tests/cases/TC-0019-memory-storage-foundation.md) and
[API-0002](../design/contracts/API-0002-initial-rust-storage.md). They use a
small std-only test executor; production code requires no async runtime.
The trait returns Send futures that may borrow inputs; it is not a boxed
or dynamically dispatched adapter API.
The Send bound now compiles and runs for SQLite’s synchronous browser target
(SPEC-0023); arbitrary JS-backed asynchronous adapters remain unvalidated.

## Evidence and CI limits

Specification/API/cases were reviewed and committed in `2625faa` before code.
All nine cases compiled and failed on missing table/row operations before the
memory implementation. All nine subsequently passed, along with formatting,
Clippy with warnings denied and the WASM library build. This provides partial
TC-0016 memory write/read coverage; filesystem/platform calls were not monitored,
and complete REQ-0034 or cross-engine parity is not established.

`.gitlab-ci.yml` defines required workspace and isolated Rust spike checks,
plus traceability and RTM jobs. Official Rust 1.98.1 and Python 3.14.8-slim
image tags/digests were checked against Docker Hub before configuration.
These jobs are configured, not executed on GitLab in this task. Local YAML
validation is not GitLab server lint or proof of runner operation. The design
job checks out the governance submodule at its committed revision, without
`--remote`; its private GitLab project must allow the parent job token access.
No project token/access settings were changed. Swift desktop integration
remains a separately documented macOS check, not a Linux CI claim.

No production database/subscription handle contract, clock/ID injection, fake
transport or binding package was invented before a test needs it. Further
work must preserve the explicit storage and Stage 0 limitations.

## Optional atomic write batches

[SPEC-0020](../design/spec/SPEC-0020-memory-atomic-batches.md) and
[TC-0020](../design/tests/cases/TC-0020-memory-atomic-batches.md) add the separate
`TransactionalStorage` capability in API-0002. The original `Storage` method
set is unchanged; an adapter implementing it alone makes no batch guarantee.

`apply_batch` accepts ordered insert/replace/remove mutations across existing
tables. Memory storage clones its maps, uses the existing CRUD operations on
that private candidate, and publishes it only after every mutation succeeds.
The first ordinary storage error returns unchanged and leaves every original
table intact. Empty batches and missing-row removal on known tables succeed.

All seven T01–T07 tests first failed at the missing implementation with the
nine CRUD cases still passing; all 16 subsequently passed. Formatting, Clippy
with warnings denied and WASM compilation pass. The copy cost is proportional
to all stored data per batch; no performance budget or optimization is claimed.
This establishes memory write atomicity, not persistent queue durability,
read transactions, observers, concurrent-client isolation, or cancellation and
panic/abort recovery. No runtime dependency or persistent engine was added.

## Native SQLite and shared conformance — SPEC-0022

Reviewed spec/API/cases were committed in `5899da5` before code. All 16 original
memory assertions were extracted into `tests/conformance/storage.rs`, included
by both adapters, with only factory substitution. Memory stayed green. The same
16 assertions then failed against a SQLite scaffold; the additional native
cases failed for missing CRUD/open behavior and missing process persistence.

`hoyadb-storage-sqlite` implements the typed CRUD/batch subset using approved
rusqlite 0.40.2 with bundled SQLite, defaults disabled. Fixed private physical
tables store logical table/row/field names as bound values. Writes use real SQL
transactions, without a memory mirror or whole-map copy. Reads strictly validate
value discriminators, SQLite types, boolean range and NULL payload placement.
Existing nonempty files require the exact unreleased version-1 format/marker;
unrelated files and unknown/incompatible formats are rejected before mutation.

All 40 workspace tests pass: 16 memory, 16 identical SQLite conformance assertions
and eight SQLite-specific cases. Those include separate writer/reader processes,
failed-batch restart, SQL-looking/empty/NUL names, unchanged unrelated database
bytes, lock errors, actual COMMIT failure, injected partial-field write failures,
strict malformed-field rejection and connection ownership transfer. Independent
review found no blockers. Formatting, Clippy with warnings denied and explicit
core/memory WASM compilation pass. Native SQLite is not claimed to build for WASM.

Review found bundled SQLite enables URI interpretation globally even when
OPEN_URI is omitted. Both `file::memory:` and named `file:...?mode=memory` paths
actually opened memory databases in regression RED runs. Reviewed cases were
committed in `c8a5683`, then leading `file:` was rejected before open in both the
native spike and production adapter. Empty and `:memory:` paths also reject.
The scoped guard changes no global SQLite settings.

API-0002 adds `BackendFailure { message }` for operational/open/decode errors;
existing domain errors retain their exact variants and coordinates. Error text
is diagnostic and unstable. SQLite methods perform work synchronously before
returning ready Send futures, so awaiting them does not make them nonblocking;
discarding an unpolled future does not cancel an already completed mutation.
Core and memory remain dependency-free. No unsafe code or async executor was added to
the production workspace. The adapter has no shared concurrent-access guarantee.

Browser persistent storage is approved for implementation in
[SPEC-0023](../design/spec/SPEC-0023-browser-sqlite-persistence-proposal.md).
The [executed worker/OPFS experiment](browser-persistence.md) passes exact
restart persistence and the applicable shared conformance subset. Storage query/change reporting,
full schema/types, production bindings,
encryption/backup protection, crash/power-loss durability and remote CI remain
unproved. This subset does not close all Phase 1 or SPEC-0001/SPEC-0002 obligations.


Q09: the configuration check first failed because locked fetch steps were
missing. After adding them, YAML inspection confirmed each fetch precedes its
offline commands, native SQLite is excluded from the root WASM selection, and
all three user-added `home` runner tags are preserved in the worktree. Both
configured Rust job scripts then ran to completion locally with a fresh,
task-owned Cargo cache: workspace 40 tests, spike core 11, native eight,
formatting, Clippy and all declared WASM builds passed. The separate design
trace/RTM commands also pass. This is local execution of job commands, not a
container/runner execution or GitLab server lint. A read-only `glab ci status`
query found no pipeline for this feature branch and returned 404 for its MR
lookup. No push, merge, project-access change or remote pipeline run occurred.


Fresh-source follow-up: `git archive` of `4d2588b`, extracted without untracked
files/build outputs, passed 40 workspace tests, formatting, Clippy and explicit
core/memory WASM compilation. A rebuilt native static library and Swift host
passed N01–N05. Freshly generated browser bindings also passed all B01–B05 in
the real browser. Temporary servers/files were stopped/cleaned after verification.
The operator subsequently approved the exact SPEC-0023 browser graph. The
2026-10-22 resume resolved/reviewed the locks, built the shared adapter on WASM
and passed actual worker/page persistence checks; see the linked browser report.
