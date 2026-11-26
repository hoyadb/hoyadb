# Phase 4 command sync — execution evidence

Status: Phase 4 / implementation Stages 5 and 6 complete for the unpublished
Rust owner and experimental Swift/optimized-browser targets. Native H01–H06 and
browser shared/host/reload/HTTP gates pass. Debug browser large-message timeout
remains explicitly failed evidence; the full optimized artifact passes with
identical workloads, assertions and deadlines. Scope and limitations are below.
Origin: operator request to resume Phase 4 after the stop recorded at 682ca06.
Branch: `feature/spec-0029-command-sync`, stacked on the preserved Phase 3 work.
Implementation snapshots: core `d465dc2`, size hardening `d619820`, native/shared
host `7578c2d`, browser host/runtime evidence `dc93546`.
No push, merge, publication, new external runtime dependency or user-store
operation performed.

The actual Phase 4 scope is IMPLEMENTATION_PLAN Stages 5 and 6: tracked async
commands, durable queue/dependency recovery, authoritative reconciliation,
explicit rollback/cancel, status/backoff and a host HTTP reference. Entity diffs,
pull/bootstrap and release security stay in later phases. Phase 5 has not started.

## Prepared artifacts

- [RFD-0003](../design/rfd/RFD-0003-command-sync-lifecycle.md): concrete policy
  resolution for decisions expressly open in accepted specs; operator
  approval recorded and adopted by ADR-0032.
- [SPEC-0029](../design/spec/SPEC-0029-command-sync.md): five ordered delivery
  slices, target matrix, non-goals, quality and rollback constraints.
- [TC-0032](../design/tests/cases/TC-0032-command-sync-conformance.md): twenty
  named acceptance cases, original-case mapping and actual host/restart gates.
- [REQ-0043](../design/requirements/REQ-0043-command-recovery-integrity.md) and
  [THR-0017](../design/security/threats/THR-0017-command-recovery-boundary.md):
  scoped recovery-integrity requirement and STRIDE/LINDDUN preparation.

No behavioral test or runtime implementation was added before policy approval.
ADR-0032 and API-0006 sections 1–2 were independently reviewed and committed at
19ab7cf before implementation. Six bounded-JSON behavior tests failed against a
compiling functional stub and then passed with the codec. The in-memory adapter
moved mechanically into core SnapshotStorage with its public facade preserved;
all four moved bodies match the originals after only crate/type substitutions.
This reuses query/index/relation/lifecycle behavior for upcoming staged handlers.
Independent review found no actionable implementation issue in these two lanes.

The initial value/staging checkpoint had **177 passing tests, zero failures**. Workspace fmt
passed. Workspace Clippy with warnings denied and core/memory WASM build passed
in the agent checks; the first intermediate WASM build contained harmless JSON
stub warnings, and final codec Clippy/WASM checks were clean. Section 3's exact
scope lifetime contract was separately reviewed before mechanism implementation.
These checks do not establish commands, durability or host sync integration.

RED/GREEN commands and outputs are in the local ignored target directory:
phase-four-json-red.txt, phase-four-json-green.txt and
phase-four-foundation-green.txt. The permanent regression tests carry the C05
case names. Retained JSON RED errors were ordinary assertion mismatches for
values/grammar/limits (six failures), not compilation or harness failures.

The later sections record execution of the remaining reviewed slices and their
actual native/browser gates under the approved RFD policies.

## Checks observed during preparation

`cargo test --workspace --offline --locked` completed with **171 passed, zero
failed across 17 test/doc-test result summaries**. Local output is retained at
`target/phase-four-prep-baseline.txt`; this is a baseline of existing code, not
Phase 4 coverage. No code changed, so no new native/browser runtime PASS is
claimed. Traceability checker reported **0 violations**; RTM was regenerated.

Initial and current user-owned `.gitlab-ci.yml` work consists of the same six
`tags: - home` additions. These are excluded from task commits. Accepted ADR
bodies, governance submodule, runtime dependency graph and Phase 0–3 code/tests
remain untouched.

## Preparation review

An independent read-only agent checked accepted contracts and the actual draft.
It identified real ambiguities in cross-store key uniqueness, receipt-before-
reconciliation recovery, map/duplicate retention, explicit rejection rollback
and capacity scope. Draft policies/cases were clarified; independent re-review
returned **PASS with no remaining blocking findings**. It also checked zero
trace violations and current RTM. This is design review only, not a production
code verdict or operator policy approval.

## Public Rust command checkpoint — 2026-10-23

API-0006 sections 3–5 and API-0007 were independently reviewed and committed
before their implementation. The public owner now stages existing model/query/
relation operations behind the process-wide FIFO permit, captures immutable
bounded args/context, and commits prepared rows plus queue/undo/counters in one
synchronous SyncStorage transaction. Memory publishes one complete cloned
candidate; SQLite uses its actual transaction and unchanged four physical tables.
No runtime dependency was added to core or the workspace.

Dispatch freezes canonical envelope bytes before returning a push batch, retains
backoff/deadlines and separates receipt from authoritative local application.
Reconciliation maps declared identities/references, validates application-defined
Put/Remove plans, releases dependency edges atomically and publishes complete
reactive snapshots before outcome delivery. Explicit inverse simulates reverse
ordered transitive undo with row guards. Cancellation stops retry even on a
conflict and retains sent tombstones. History/capacity and schema lifecycle gates
use the same validated recovery ledger.

The checkpoint command `cargo test --workspace --offline --locked` passed
**233 tests, zero failures across 23 test/doc-test summaries**. Full workspace
format check, all-targets Clippy with warnings denied, and core/memory wasm32
build passed. Trace: zero violations; RTM current. This validates Rust and native
SQLite paths, not actual foreign suspension or browser persistence yet.

Permanent tests are in command/{json,execution,records,owner,queue,reconcile,sync},
shared tests/conformance/{sync_storage,command_owner,command_sync}.rs, both real
adapter test entries, and the SQLite physical failure test. Shared public cases
cover dispatch/receipt, exact frozen retries, multi-parent mapping, schema
Reference versus ordinary String handling, transitive inverse, overlap conflicts,
explicit sent/unsent cancellation, history and live event delivery. Supplemental configured-500 live-capacity boundary runs passed on memory and
SQLite after the full workspace checkpoint. The complete public integration case
including that loop took 157.01 s on memory and 187.70 s on native SQLite in the
unoptimized test profile. These are measured synthetic suite costs, not an SLO
or production throughput claim; repeated full-ledger validation/copy/persistence
is expensive as the queue grows. Output: phase-four-capacity-500-green.txt.
The SQLite
fixture injects real SQL failure during correction, checks no false data/outcome,
then retries successfully and reopens the store. A separate deferred-foreign-key
COMMIT failure checks that neither application rows nor recovery metadata survive
an unsuccessful transaction.

### Retained behavioral RED evidence

Local ignored target logs retain commands and actual output. They are evidence
of observed intermediate runs, not shipped test failures:

| Slice | Observed compiling RED | Final targeted evidence |
| --- | --- | --- |
| Execution scopes | Eight expected stub assertion failures | phase-four-execution-green.txt |
| Atomic recovery | Shared memory/SQLite storage stubs failed contract assertions | phase-four-sync-green.txt |
| Public owner | Admission/enqueue behavior failed before implementation | phase-four-owner-final.txt |
| Push/receipt planner | Five stub behavior failures | phase-four-queue-green.txt |
| Public dispatch | Initial status registration rejected by stub | phase-four-public-dispatch-green.txt |
| Mapping/inverse | Four planner stub failures; public invalid map accepted by stub | phase-four-public-reconcile-green.txt |
| Retired local reference | begin returned true for rolled-back cancellation producer | phase-four-retired-reference-green.txt |
| Live undo retention | forget removed a still-required producer | phase-four-forget-retention-green.txt |
| Authoritative ID validation | Empty-ID Put was accepted | phase-four-correction-id-green.txt |
| Zero-history events | Final Applied event missing after immediate eviction | phase-four-zero-history-green.txt |
| Idle wake clock | New key regressed behind a persisted queue's idle wake sample | phase-four-idle-clock-terminal-fault-green.txt |

Independent review reproduced the retired-reference, undo-retention and empty-ID
issues; all now have permanent regressions. The parent also reproduced the
zero-history event loss and idle-clock regression before fixing them. Final
checkpoint logs: phase-four-core-checkpoint-tests.txt,
phase-four-workspace-clippy.txt and phase-four-core-checkpoint-wasm.txt.

### Host gate at the Rust checkpoint (historical)

Reviewed API-0008 defines one std-only fixture over the same real SQLite owner,
used by both experimental hosts with lossless tagged RPC values. Browser and
native host work was then uncommitted/in progress. The shared loopback HTTP reference
has three passing stdlib tests after two positive behavior tests failed against
its compiling stub; this is not yet a claim that browser fetch or Swift
URLSession command sync passed. Actual suspended handlers, disposal/callback
liveness, separate native processes, browser reload, HTTP and full target
regressions remain necessary before closing TC-0032 or Phase 4.

## Cross-provider checkpoint and preflight hardening

One bounded Claude Opus 5 review of d465dc2 returned PASS with no concrete core
correctness/data-loss finding; it independently passed all 50 then-current core
unit tests. No repeated Opus rounds were requested. Its nonblocking size note
was subsequently reproduced: the largest record accepted by enqueue preflight
could fail first-send encoding when attempt/deadline/state scalars grew.
Reviewed contract e9c4ed1 now reserves those maximum scalar widths without
changing stored values or dependency state. A binary-search boundary regression
failed for the expected first-send error, then passed send, maximum retry scalar
widths and receipt encoding after the size-only fix. Core unit tests now **51 passed**;
all-targets workspace Clippy remains clean. This later narrow delta received a
separate independent code review PASS; the earlier Opus verdict is not claimed
as a review of code it did not see. Evidence:
phase-four-preflight-width-{red,green,clippy}.txt.

## Experimental host bridges and bounded replies

API-0008 was reviewed with both host consumers and committed before the shared
SQLite fixture and foreign dispatchers. `spikes/common/command_fixture.rs` is
compiled by both the native C ABI and browser WASM bridge; it delegates to the
same production command owner. Swift and main-thread JavaScript own suspended
handlers, platform entropy, trusted-context sampling, callback delivery, timers,
HTTP and application reconciliation. They copy tagged values so exact i64,
timestamps and integral Float values survive foreign conversion. The native
bridge releases its registry lock before its call-scoped reply callback.

The bounded-reply amendment reproduced two real bridge faults: a finish could
report LimitExceeded after committing, and a drain could discard events when
the aggregate reply exceeded the one-MiB frame bound. Mutating fixture operations
now preflight the complete reply, including batch before-images. Prepared push
and event snapshots retain owned complete prefixes until consumed. Unsubscribe
filters cached events; an empty retained snapshot remains distinct from a new
drain. A single oversized event stays pending until its observer is cancelled.
Hosts send each prepared prefix as its own nonempty command array, rather than
joining pages into an oversized HTTP body. These bounds belong to the isolated
test bridge, not to the core query result size contract.

Real SQLite regressions first failed for the expected post-commit/consumption
behavior and then passed all four cases. Four pure complete-frame budget tests
also passed. The JavaScript host's nine deterministic tests cover FIFO,
cancellation after durable finish, retained Busy responses/plans, coalesced
wakes, paged callbacks and separate HTTP arrays. A focused independent review
returned PASS for the budget helper, actual router, C ABI/header, worker and
browser client; it independently reran all nine JavaScript tests successfully.

The Swift reference additionally retains a failed per-member receipt response
for explicit retry and surfaces Protocol independently of the successful outer
RPC. Its actual conflict-response regression failed before that correction.
Retaining responses and application-built plans across Busy uses the shared
coordinator's release signal, not repeated HTTP or mapper calls.

Final large-message verification corrected two harness assumptions without
changing product behavior: value-only edits require a named-column observation,
and aggregate multi-page completion needs a separate finite watchdog from the
five-second HTTP timeout. A browser comparison also normalized Object key order
instead of treating insertion order as JSON value semantics. Original payloads,
exact snapshots and per-request deadlines remain asserted. Native compiling
RED/GREEN commands and boundaries are recorded in
[COMMAND_RED.md](../../spikes/cross-platform/native/COMMAND_RED.md).

The native lockfile gains the existing SQLite workspace adapter and 22 package
entries; every added external package/version/checksum already exists in the
approved root lockfile. Core remains dependency-free. No third-party runtime
dependency outside the approved workspace graph was introduced.

### Acceptance coverage locations

| TC-0032 cases | Permanent shared/core assertions and foreign-host gate |
| --- | --- |
| C01–C04 | execution/owner units and shared command_owner; H01/H02 actual suspension, FIFO, cancellation and stale scopes |
| C05/C06 | JSON/records units, shared atomic-storage/owner cases; H03 tagged conversion and H05 independent lifetimes |
| C07/C09/C10/C11 | atomic-storage/sync graph and real SQLite transaction faults; H05 killed writers/readers and browser receipt/outcome reloads |
| C08/C13/C14 | shared command_owner and command_sync graph/correction_inverse; H06 application plans, mapping and guarded inverse |
| C12 | shared dispatch and per-member protocol assertions; H06 real reversed/missing/duplicate/conflicting HTTP results |
| C15/C17 | shared graph/lifecycle/retention assertions; H05/H06 durable state recovery and migration refusal/release |
| C16 | injected clock/jitter queue tests, shared dispatch; H06 host-owned timer and exact frozen retry |
| C18 | shared event ordering and zero-history assertions; H04 callback liveness/redaction and bounded snapshot pages |
| C19 | configured-500 shared capacity boundary, history/retention units, per-store capacity under shared admission |
| C20 | shared storage/owner/sync code compiled into actual OPFS; H01–H06 Swift/browser runtime and opt-in HTTP fixture |

The historical TC-0005–0010/0012/0014 mapping remains the command-only subset
specified by TC-0032. No executable Gherkin runner, entity-diff case, server
authorization or backend deduplication PASS is inferred from this table.

### Final native execution

The current-source Swift executable passed **H01–H06, six groups and zero
failures**, in 182.710 s on x86_64 macOS 26.7.1 / Swift 6.3.2, with deployment
target macOS 26.5. It uses actual suspension, independent SIGKILL writer/readers,
owned callbacks and URLSession through every reference HTTP mode. The large
four-command test observed bounded arrays of three members and one member.
Focused debug timings were 25.486 s for paged callbacks/oversized cancellation,
80.051 s for the large HTTP case and 5.236 s for deferred application plans.
The aggregate harness watchdog is 900 s; HTTP request/resource timeouts remain
five seconds. These are measured test costs, not throughput or latency promises.

Native Rust passed **20 tests** (15 unit and five persistence), fmt and all-targets
Clippy with warnings denied. Swift compiled with warnings as errors and passed
strict swift-format. The final source and artifact hashes were independently
checked against [COMMAND_EVIDENCE.json](../../spikes/cross-platform/native/COMMAND_EVIDENCE.json).
The successful harness removed its own root; eight explicitly recorded earlier
diagnostic/failed roots were removed only after their native processes closed.
Other directories were untouched, and RED output/source snapshots were retained.

The existing Swift N01–N05, N07–N09, four Phase 3/reactive scenarios and separate
persistence writer/reader processes also passed against the final native Rust
library (SHA-256 `7f1aba42ebca2c814adf8603616e1c259baffa070fa94e8ca3c080dc103d54d3`).
Exact commands/output: `target/phase-four-native-legacy-final.txt`.

### Browser profiles and retained debug failure

The debug artifact passed all 14 unchanged shared OPFS cases, including the
configured-500 boundary in 574,938 ms with identical empty pre/post file inventory.
The full older browser suite also passed its 36 first-page, 12 reload and 17
reactive report groups with this artifact.

The aggregate large-message command scenario did **not** pass debug: a
`receive_wire` RPC reached its unchanged 60,001 ms timeout, and a status request
queued behind it also timed out. The approximately 841 kB HTTP request completed in
33 ms; measured preparation took 13,419 ms and individual large finishes took
3–7.8 s. This establishes a slow synchronous local path, not a network stall;
it does not identify an internal hotspot without profiling. Cleanup waited for
the worker to finish. This failed run remains separate from final evidence.

Independent review approved API-0008's profile clarification (`1b884e0`): a
standard optimized artifact may run the complete unchanged matrix with fresh
hashes, while debug compile/lint/container gates remain required. The default
`check.sh` is unchanged. The browser README records exact additional release
build/bindgen commands. No payload, assertion, capacity, per-RPC deadline, HTTP
timeout, dependency, pool size or browser security setting is relaxed. Runtime
PASS applies only to the exact profile and artifact actually executed.

## Final frozen-container verification

The exact image
`rust:1.98.1@sha256:a8a5f0a1e5fe7dfe1d352591e4a1c7dd2c08fd70475cae872cf3458ba0df0546`
finished with exit zero and `PHASE4_CONTAINER_GREEN`. It used a frozen read-only
source snapshot copied into an ephemeral build directory, fresh Cargo caches
and targets, and the recorded Debian Clang/LLVM tools. All 92 Rust/Cargo/build
input hashes were rechecked against the working sources after exit. Capture
preceded the host commit; matching content hashes establish the tested source,
rather than claiming the earlier HEAD alone contained those files.

The run passed **234 workspace tests** across 23 summaries, plus 11 older spike
and 20 native Rust tests: **265 tests, zero failures across 30 summaries**.
Both runnable examples, workspace and isolated-spike fmt/Clippy with warnings
denied, and core/memory/SQLite/browser/older-spike WASM builds passed. The memory
and native SQLite shared command suites, each including the configured-500
boundary, took 487.20 s and 603.09 s in this unoptimized container run. These
are whole-suite observations under contention, not per-command latency or SLOs.

The [exact commands](phase-four-ci-commands.sh),
[complete normalized output](phase-four-ci-output.txt), and
[source/image manifest](phase-four-ci-source-sha256.json) retain the evidence.
Only terminal carriage returns/trailing whitespace were normalized. This is
local pinned-container verification; remote GitLab CI was not observed. Swift
and actual OPFS runtime evidence comes from the separate native/browser runs.

## Final optimized browser execution and closure

The standard release artifact passed **27/27 command report groups**: all 14
unchanged shared OPFS cases, real suspended main-thread handlers, callbacks,
keyed HTTP, bounded replies and three actual page reloads. The configured-500
case took 268.495 s and restored the identical empty pooled-file inventory.
The large-message host scenario took 38.323 s. It sent separate nonempty HTTP
arrays of 840,873 and 280,217 bytes and reconciled the five intended commands,
including a command whose wake arrived during awaited mapper work. RPC limits
remain 60 s, HTTP limits five seconds and aggregate/capacity watchdogs 900 s.
Every owner closed, VFS shutdown acknowledged and the owned namespace was removed.

The same optimized artifact passed the entire earlier browser matrix: 36 initial
page groups, an actual reload with 12 groups, and 17 reactive groups. The first
legacy page deliberately reports needsReload; its successor reports done.
Node's final nine deterministic client tests and the Python reference's three
stdlib tests pass. No assertion or workload was removed for the optimized run.

[Final browser results](phase-four-browser-results.json) preserve the explicit
failed debug run alongside the release PASS. The
[legacy results](phase-four-browser-legacy-results.json),
[release source/artifact manifest](phase-four-browser-source-sha256.json) and
[build/client-test output](phase-four-browser-test-output.txt) record commands,
runtime and boundaries. All 93 source hashes and both generated JS/WASM hashes
were independently checked against the final executed files. Historical debug
manifests retain their own hashes; they are not relabelled as release artifacts.
Runtime: Paseo 0.12.0 / Chrome 152.0.7977.76 / Electron 44.2.0, actual worker
OPFS at loopback, with crossOriginIsolated=false and the original 24-slot pool.

Traceability reports zero violations and the generated RTM is current. Core and
host implementations received independent reviews; one bounded Claude Opus 5
checkpoint reviewed the core, and later narrow corrections/host code received
separate focused review. No finding quota or repeated Opus rounds was imposed.
The user-owned six CI tag additions and untracked .agents directory are preserved.
Final independent closure review rechecked all manifests, counts, cleanup flags
and the profile-specific claims and returned PASS with no material blocker.
The task-owned HTTP server was stopped and frozen source snapshot removed after
verification; manifests, runtime results and diagnostic logs remain available.

Phase 4 stops here. Entity diffs, pull/bootstrap, production TLS/backend
authorization and deduplication, packaged SDKs, mobile/other browser targets,
independent concurrent writers and physical power-loss guarantees are not
established by these fixtures. Snapshot/ledger copy-validation costs and the
failed debug large-message deadline are material experimental limits. Source
rollback must preserve recovery stores; old readers reject their metadata rather
than deleting it. No release, merge or publication was performed.
