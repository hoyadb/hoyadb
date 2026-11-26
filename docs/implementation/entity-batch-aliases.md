# Atomic prepared entity batches

The local Monstera/Backoffice migration requires prepared parent/child writes and
partial edits to retain the existing atomic behavior. [SPEC-0048](../design/spec/SPEC-0048-entity-batch-aliases.md)
and [TC-0052](../design/tests/cases/TC-0052-entity-batch-aliases.md) define the additive
capability. It introduces no dependency or stored format. Durable saved-ID
resolution is a separate [SPEC-0049](../design/spec/SPEC-0049-durable-entity-aliases.md)
opt-in implemented alongside this integration.

The [core alias helper](../../crates/hoyadb-core/src/command/entity_batch.rs) feeds
ordered explicit references through the existing [staged entity owner](../../crates/hoyadb-core/src/command/entity_owner.rs).
Both old and new batch paths retain the existing SyncPlan finish. Aliases exist
only during staging; the namespace allocator supplies every persisted new entity
ID. The new partial patch operation reads preceding staged writes, preserves
omitted current fields and queues explicit supplied-field intent even when the
value is unchanged. A local completion marker can share that commit.

The [shared facade](../../crates/hoyadb-bindings/src/entities.rs) owns exact ABI
parsing and [complete reply preflight](../../crates/hoyadb-bindings/src/budget.rs).
Only the new report variant computes its full conservative report once after
staging the size model; the original report path retains its existing prefix
checks. A new-only cumulative encoded byte lower bound stops expanded patch
rows during normalization and stops complete before/after images during report
construction. The final serializer still checks the complete reply. This avoids
both unbounded expanded snapshots and quadratic serialization of the new large
batch regression. The [portable host](../../packages/client/src/client.js) only supplies
wire defaults, one captured context and normal delivery. The application layer's
durable configuration and resolver methods delegate directly to that host.
No arbitrary query rewriting or wrapper identity ledger is introduced.

## Test-first evidence, 2026-11-07

- A01 public facade parent/child regression failed with InvalidRequest before
  `entity_batch_aliased` existed, then passed after implementation.
- A07 public facade partial edit plus marker regression failed with
  InvalidRequest before the new patch variant, then passed after implementation.
- The host alias regression failed before its forwarding method; a temporary
  explicit unsupported implementation also produced the expected failing runtime
  result. The implementation then passed the forwarding regression.
- D06 host/application durable delegates failed with missing methods before
  implementation, then passed. The facade durable regression compiled and
  failed with InvalidRequest before dispatch implementation, then passed.
- The bounded expansion regressions failed before the final guard: repeated
  large patch snapshots reached a late RowNotFound after already exceeding the
  reply cap, and before-image sizing read a fifth large row instead of stopping
  after the fourth exceeded 1 MiB. Both passed after the cumulative lower bound.

## Observed checks

The focused alias facade checks passed for parent/child allocation, partial
edit/marker behavior, preserved-field reply budgeting and generated reference
expansion plus alias-map budgeting. The latter input is below the wire limit but
its complete reply exceeds it: rejection leaves no row or pending work and retry
uses allocator position one.

All four [shared memory/SQLite cases](../../tests/conformance/entity_aliases.rs)
passed on each adapter: atomic mixed insert/replace/remove, parent dependencies
and canonical remapping after reopen, all invalid alias classes and capacity
rollback, same-value partial intent, marker rollback and successive staged patches after replacement. A patch after
removal rejects the full batch without altering rows or queue status.

The complete facade entity suite passed all 14 tests, including existing managed
links, protected SQLite, passive merge and reply-budget cases. All six facade
unit tests also passed, including before-image bounded expansion. Source consumer
TypeScript checks passed for portable and application entries.

Portable host entity API tests: 7 passed. React Native host transport tests:
10 passed. Public compatibility-tool tests: 22 passed; traceability-tool tests:
13 passed; packaging-tool tests: 9 passed; sync-proof negative-control tests:
20 passed. The formatting fixture and `git diff --check` passed. Traceability
reported zero violations; RTM was regenerated and checked current. The sync
proof source convention check was repeated and passed after final core edits;
this is not a Rust semantic proof.

Fresh ordinary WASM memory conformance passed all 17 reports, including the
new packaged integration case. The full npm suite passed 211 client and 11
testing tests. Its first integration attempt exposed an incorrect lexical float
expectation (`2.0` versus the core's canonical `2`); inspecting the actual stored
cell confirmed float kind and numeric value 2. The fixture now asserts both,
matching the existing float conformance convention. No production change was
needed.

The independent local tarball consumer also passed: its artifact report records
matching source manifests, ordinary WASM build, package build/check and packed
consumer exit 0 at
`/private/tmp/hoyadb-monstera-sdk-build-0pyvtezg/accepted-javascript.json`.
At that checkpoint, the sole post-compile change was the conformance fixture
assertion above.

Workspace all-target clippy passed with warnings denied after collapsing one
nested patch condition into a behavior-preserving let-chain. That source edit
requires refreshing the artifact inventory and repeating artifact checks; the
preceding WASM/npm/packed results are not final-source certificates. The core
and memory wasm32 library build passed with warnings denied. Global formatting,
sync-proof and diff checks passed on the final edited source.

The workspace executable suites passed 933 tests across 91 suites, including
55 protected-engine tests. The initial full command exited 1 at its first
doctest because overlapping earlier rebuilds left stale SQLite crate metadata.
A fresh sequential `cargo test --workspace --doc --offline --locked` rebuilt
the affected libraries and passed all six compile-fail documentation tests and
every crate doc harness. The complete binding package was then rerun on the
frozen final source: 59 executable tests and its doc harness passed. The final
SQLite durable suite with `ledger-convert-tool` passed all three cases. Logs are
`/private/tmp/hoyadb-migration-workspace-tests.log`,
`/private/tmp/hoyadb-migration-workspace-doctests.log`,
`/private/tmp/hoyadb-migration-final-bindings.log` and
`/private/tmp/hoyadb-migration-durable-sqlite.log`. This reports the actual split
verification; it does not label the initial full command as exit 0.

Final artifact refresh and native/mobile integration remain with the coordinated
artifact/application lanes. An earlier facade run passed 11 tests but took 465 seconds with the
superseded quadratic preflight; the corrected full 14-test suite completed in
3.63 seconds. Those intermediate runs are not final-artifact evidence. No mobile
execution is claimed by host tests.

## Source compatibility review

A separate candidate was generated at
`/private/tmp/hoyadb-migration-api-candidate-20261107.json`. It is not a passing
source check. The proposed declaration diff adds `AliasedWrite`,
`AliasedBatchResult`, `entityBatchAliased`, `configureEntitiesDurable` and
`resolveEntityId` to the host, and the last two methods to the application API.
All existing declarations, default options, C symbols, RN Codegen and factories
are unchanged. Root independently reviewed the exact seven additive declaration
lines, ABI rows, SPEC-0048/49 and host/application delegates, then approved only
these source additions on 2026-11-07. The current reviewed manifest was refreshed
from that candidate with the review recorded; retained historical snapshots
remain untouched. This is source compatibility acceptance, not runtime or native
acceptance.

The reviewed source drift gate passed. A fresh macOS Swift module/digester check
also passed, with only its two already accepted historical enum diagnostics;
`/private/tmp/hoyadb-migration-swift-api-20261107/report.json` records the exact
source hashes, commands and new dumps. No native runtime or binary ABI claim
is inferred from that source check.
