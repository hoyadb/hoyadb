# Phase 3 reactive queries — execution evidence

Follow-up: [review corrections and three Claude verification rounds](phase-three-review-resolution.md)
records the independently reproduced large-query and reactive recovery faults,
their corrections and the expanded regression evidence. The original execution
record below remains a historical snapshot.

Operator request: continue the next roadmap phase until its end. Preparation
SPEC-0027/API-0005/TC-0029 was independently reviewed and committed as `f7f6002`.
Implementation is committed as `58101a9`.
Review corrections DEF-0004/0005 and TC-0030 were reviewed and committed as
`18be46a` before their ordering/deadline regression correction.

## Implemented behavior

The public Rust QueryBuilder now supports fetch, observe, observe_count and
observe_with_columns through an exclusive ReactiveDatabase owner. Equivalent
normalized queries share projected candidate IDs and ordering keys. Ordinary
commits re-evaluate affected IDs; consulted to-one target changes find source
IDs through indexed reverse foreign keys. Source/pivot navigation changes may
rebuild that constrained relation key set. Complete rows materialize only for
row subscribers when an exact owned snapshot must emit. Default observation
compares visible ID sets; named columns additionally compare stored values.
Atomic net-zero and failed batches remain silent; separate commits preserve
separate snapshots in order.

Counts never materialize full records, coalesce to the latest unequal count,
and retain a 16 ms minimum between successful deliveries, including recovery.
The caller supplies a monotonic clock and schedules drains. Core owns no timer,
thread or callback runtime. Errors invalidate pending count state without
resetting the deadline. Equivalent registration while dirty rejects until
explicit refresh or a later committed write recovers the shared cache.
Recovery republishes exact current state, including an unchanged count after
an error. Explicitly requested projected cells validate strictly; separate
ordering keys preserve API-0003 Null normalization of wrong-kind logical cells.

Cancellation removes queued events and final cancellation releases the cache.
Subscription IDs never repeat across owners in one core instance. The native
and browser test bridges check event liveness at delivery and keep callback
execution outside Rust state/registry borrows. No external runtime dependency
or persistence format changes. The native fixture adds only project-local core
and memory path dependencies.

## Cases and actual RED/GREEN

| Prepared case | Executed assertions |
| --- | --- |
| R01–R04 | owned initial/full snapshots; membership; cancellation/close; net atomic batches and rollback; named columns, missing/Null and invalid registration |
| R05–R07 | deterministic count deadlines/backward clock; shared normalized predicates/In values across modes; ordered/limited refill, ties and reorder silence |
| R08–R09 | indexed direct to-one dependencies, dangling/cyclic targets, unrelated writes; all three relation navigations, duplicate pivots and projected navigation count |
| R10 / TC-0030 | post-commit projected/materialization failures remain observation errors; retry current state; no stale count/duplicate dirty cache; recovery preserves deadlines |
| R11 / TC-0030 | every primitive type, Unicode/bound names, strict requested fields, normalized order key, invalid path/type/table including limit zero |
| R12 | no-op replacement/link/unlink; reopen retains data but no subscriptions |
| R13 | browser worker/main-thread callback lifecycle and generation validation |
| R14 | actual Swift callbacks over production core/memory, owned rows/counts/columns, reentrant read/write, recursive drain/cancel/close and stale owners |
| R15 | targeted projected work, zero count materialization, shared observers; repeatable native/browser callback timings |

Shared assertions live in `tests/conformance/reactivity.rs` and `projection.rs`.
Both memory and native SQLite contract suites execute them; browser builds reuse
the exact source. SQLite also has a physical corrupted-unrequested-field fixture
proving count/key projection does not decode full records. Existing query
validation remains strict for ordinary full row reads.

The initial core smoke test failed at the unimplemented observation registration
before implementation (`target/phase-three-core-red.txt`). Five shared R11
projection cases were observed failing independently on both adapter stubs.
Additional review tests reproduced InvalidRow on a valid related ordering query
and a count emitted before the retained deadline: **10 passed, 2 failed**
(`target/phase-three-review-red.txt`). The earlier R10 stale pending count
regression reproduced two notifications where only the error was expected.
All remain permanent R08/R10/R11 assertions under TC-0030.

## Host verification

`cargo test --workspace --offline --locked` passes **158 tests**, preserving all
122 Phase 2 tests. The workspace format and Clippy checks with warnings denied
pass. Core and memory build for wasm32-unknown-unknown. Both runnable examples
pass:

```sh
cargo run -p hoyadb-storage-memory --example data_model --offline --locked
cargo run -p hoyadb-storage-memory --example reactive --offline --locked
```

The latter checks a shared cache, exact owned row event, count delayed until
16 ms and final cancellation release. Its command is included in the workspace
CI job. Local execution is distinguished from unobserved remote GitLab CI.

R15 uses 8 and 64 unrelated rows with two equivalent count observers: each
ordinary edit touches at most one candidate, all maintenance projection queries
include the changed ID restriction, and no full query or lifecycle scan occurs.
R08 separately checks indexed reverse-target lookup and unrelated-table silence.
This proves restricted work at the adapter boundary, not constant total CPU:
the current cache clones/sorts projected keys, and adapters retain their own
transaction/query costs. Initial candidate loading is explicit.

## Native delivery

The [actual native RED/GREEN record](../../spikes/cross-platform/native/REACTIVE_RED.md)
contains exact build/link/run commands. Three new Swift scenarios pass on
macOS 26.7.1 with Swift 6.3.2 and Rust 1.98.1. Eight native Rust tests, eight
older Swift scenarios and separate persistence writer/reader processes pass;
format and Clippy checks pass. Per-event cancellation/close checks suppress
already-extracted events, and copying plus reentrant mutation/close preserves
payload ownership. The wrapper rejects recursive delivery and retains no
foreign callback/context beyond the call.

A final rebuild after the core corrections produced twelve insertion-to-callback-entry
samples of 198,401–426,365 ns, with an
explicit immediate drain on the same OS thread as each callback. Samples include
insertion, core maintenance and drain; exclude copying and callback queries.
Increasing result size contributes to the samples. These are observations,
not an average, performance budget or automatic UI dispatch promise.

## Browser and reproducible validation

The [actual browser RED](../../spikes/storage-browser/results/phase-three-red.json)
failed at the missing reactive worker operation before bridge implementation.
The same real browser then passed [7 standalone reactive report groups](../../spikes/storage-browser/results/phase-three-green.json),
including 17 shared Rust case mappings (R01–R10, five R11 partitions, R12/R15)
and main-thread delivery tests. The complete harness passed
[26 first-page report entries](../../spikes/storage-browser/results/phase-three-full-stage1.json)
(19 prior plus 7 new) and [12 after an actual reload](../../spikes/storage-browser/results/phase-three-full-stage2.json).
Worker/page restart, exact typed rows, rollback, read-before-write, generated-ID
continuity and acknowledged VFS shutdown/namespace deletion remain green.
Only the unique synthetic test namespaces were removed; no origin-wide wipe.

Runtime: Paseo 0.12.0 / Electron 44.2.0 / Chrome 152.0.7977.76 on secure loopback
port 8770, crossOriginIsolated false. Five actual write-to-main-callback samples
were 14.3–24.4 ms, including a worker write acknowledgement followed by an
immediate explicit drain. These include transport, storage, maintenance and
scheduling; they are not comparable to the in-process Swift boundary timings.
Count delivery uses explicit deterministic clock samples and materialized zero
full records. Generation replacement, unsubscribe/close after extraction,
recursive/concurrent drain rejection, callback exceptions and post-commit
observation error recovery all passed. The owned HTTP server was stopped.

The [source/build manifest](../../spikes/storage-browser/results/phase-three-build-manifest.json)
hashes 48 runnable source files and four generated JS/WASM artifacts, recording
tool versions and the exact build command. All 52 hashes matched the final
executed files when independently checked. Generated package contents remain
ignored build outputs. The documented `check.sh` WASM target, fmt, Clippy and
JavaScript syntax checks passed. A host-target browser-spike check encounters
the existing SQLite WASM shim/Mac SDK typedef conflict; the supported browser
WASM build and actual runtime are the validated path.

The exact pinned image
`rust:1.98.1@sha256:a8a5f0a1e5fe7dfe1d352591e4a1c7dd2c08fd70475cae872cf3458ba0df0546`
passed from frozen read-only source and fresh ephemeral Cargo caches/targets.
It installed the configured Debian Clang/LLVM tools and passed workspace tests,
both runnable examples, fmt/Clippy and core/memory/SQLite/browser WASM builds.
Exit zero and `PHASE3_CONTAINER_GREEN` are preserved with the
[exact commands](phase-three-ci-commands.sh) and [output](phase-three-ci-output.txt).
Terminal progress carriage returns/trailing whitespace are normalized in the
committed output. This is local container execution; remote GitLab CI was not observed.

Independent reviews found no remaining blockers in core/projection semantics,
native ownership or browser generation/callback delivery. The core reviewer
also independently passed all 35 focused reactive/projection tests.

A fresh `git archive 58101a9` extracted under a task-owned target directory
passed all 158 workspace tests, the reactive example and format checks with a
fresh target directory. No generated browser package was needed for host tests.
The [host test output](phase-three-host-tests.txt) and [final native callback
output](phase-three-native-output.txt) retain the observed results.
`trace_check.py docs/design` reports 0 violations and `rtm_gen.py docs/design
--check` reports the RTM current. The bounded roadmap Phase 3 is complete;
Phase 4 command sync has not started.

## Remaining scope and rollback

No command sync, durable queue, transport, initial bootstrap semantics,
cross-client propagation, packaged SDK, mobile validation, pagination,
encryption or backup exclusion is introduced. Slow row consumers can retain
owned snapshots until drain/cancel/drop; hosts own scheduling and resources.
The bounded browser/Swift bridges do not choose a production binding generator.
No forced crash/power-loss durability or remote CI result is claimed.

Rollback is a reviewed source revert preserving database files. Subscriptions
are ephemeral, so there is no subscription migration. No origin-wide deletion,
production data operation, push, merge or publication is performed. The six
preexisting user CI runner-tag lines are preserved outside task commits.
