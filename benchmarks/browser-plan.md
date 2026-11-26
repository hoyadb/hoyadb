# Browser JavaScript SDK benchmark proposal

Status: **reviewed for implementation; partial harness under validation; no accepted measurements**.
Authority: [TC-0037](../docs/design/tests/cases/TC-0037-performance-baseline.md),
[API-0011](../docs/design/contracts/API-0011-general-platform-sdk.md) and the
reviewed [parent protocol](PLAN.md). The exact proposed metadata is
[browser-workloads.json](browser-workloads.json). Existing native benchmark
sources and workload metadata are owned by the parent lane and are unchanged.

## Scope and stop condition

Add separate `browser-sdk/memory` and `browser-sdk/opfs-protected` classes.
Every timed operation starts at the public installed JavaScript SDK, traverses
its production host/worker channel and ordinary optimized WASM, and ends at the
decoded SDK return or actual managed callback. These are not timings of direct
Rust `Facade` calls. No SDK implementation, cipher profile, queue algorithm,
schema, sample count or callback deadline changes for the experiment.

The acceptance condition is a complete identified raw matrix, independent
correctness proofs, unchanged-artifact functional/security gates, cleanup and
strict existing summary-tool acceptance. Numbers remain observations until the
operator reviews repeatability and approves numerical budgets. Mobile classes
are separate gates; this work cannot declare them supported.

## Exact dataset, counts and identities

The metadata copies the parent workload's literal Schema A, deterministic seed,
51 orthogonal cohorts and counts exactly, and records the source workload SHA256.
Review rejects parent/schema/cohort divergence. The axes remain N100/1000/10000,
real durable Q0/10/100 and fanout1/10/100 equivalent/distinct observers with
membership/columns/count modes. This is not a Cartesian expansion.

Every warm cohort has five untimed warmups and 30 measured samples in each of
five independently instantiated worker/run identities. One fresh ordinary
worker is used per cohort/run, never five records from one worker relabelled as
independent runs. Count samples use real monotonic elapsed time and the actual
SDK timer; deterministic clocks are confined to retry correctness. The
unchanged capacity500 stress remains a separate functional gate managed by the
parent, not a latency sample or silently replaced by a smaller test.

Seed authors then articles through public SDK batches of at most100 writes;
keep every frame below the existing1MiB limit. All other tables remain present
and empty. Validate ordered cursor pages<=100 against the independent literal
typed oracle before and after a cohort. Preserve absent optional `note` versus
present Null, Unicode and exact Float/Integer/Timestamp/Reference tags. Never
ask for an all10k-row response. The boundary cohort alone adds the named i64
extrema/integral Float probe and removes it afterward, recording N+1 explicitly.

Use the parent's fixed test namespace, key shape and capacity/history/backoff
settings. Keys stay binary and never appear in URLs, query strings, JSON,
logging or manifests. Names have the exact `phase-seven-sdk-owned-bench-`
prefix and a bounded class/cohort/run/sample suffix. Ordinary SDK ID injection
only proposes already-declared IDs; entity IDs remain core-owned.

The parent workload base revision is commit `5a69083`; the browser metadata carries
its exact batch fieldsets and Sent-queue operation details, plus the explicit
boundary-fixture clarification in this packet. That probe matches existing
native code exactly: ID `benchmark:extrema`, baseline article0 fields with
score i64MAX, timestamp i64MIN, Float42.0 input/canonical Float42 output,
title `東京🌿` followed by NUL and `extrema`, and a present Null note. It is one
additional row (N+1) removed after the schema/protection/point read. Native `wait4`
rusage and RSS/1 remain native-only measurements. Browser readiness and resource
checkpoints below identify their own transport and metrics instead of relabelling
native process evidence.

## Public SDK operations and endpoints

| Cohort | Public operation and postcondition |
| --- | --- |
| point/selective/sorted-related | `db.query(...).fetch()` with parent predicates/order/limits; exact ordered typed rows |
| batch-insert/update/remove | One `db.batch` with ten changing writes; inspect results then restore outside timing |
| optimistic-command | `db.execute` admission, captured context, scoped replace across an actual `setTimeout(0)` host turn, finish; resolve/forget and restore outside timing |
| entity-patch | Configured `db.patch`; exact changed scalar and diff/status; cancel/forget restoration outside timing |
| non-noop-pull | `db.pullOnce` using an application transport returning the parent's100 changing authoritative field updates; real staged comparison/checkpoint commit, newer restoration page outside timing |
| reset | `db.reset('preserve_recovery')` on configured populated data/comparison state; assert progress reset, re-seed/bootstrap outside timing |
| reactivity | Production managed observation callback on the parent mutation/query/window; no polling or explicit manual drains to advance it |
| boundary-known | Fixed valid schema/protection/point-row operations with exact input/output byte counts and complete encode/channel/decode duration |

Queue10/100 consists of genuine pending no-op commands created by `execute`,
marked sent using normal `preparePush` as in the parent workload. No shadow
queue or private ledger reset. Each measured command is terminally received,
reconciled and forgotten outside timing, restoring Q. Each measured entity diff
is cancelled/forgotten outside timing. Check exact logical queue state and
cardinality, not only returned result count. Do not start background network
sync during latency cohorts.
The baseline no-op commands are Sent with attempts1 and due2000 after preparing
at1000 with jitter0. A sample's new command is prepared at1000 while the baseline
remains unchanged. Batch insertion uses article:N..N+9 IDs with exact fieldsets
from the last ten baseline rows and valid existing authors; updates change the
first ten titles to `changed-<iteration>-<index>`; removals target those exact
first ten rows. Assert exact before/after fieldsets and QueueStatus restoration.

Reactivity uses the parent's bounded score-descending top-ten membership/column
windows and unlimited count predicates. Register callbacks first and await the
real managed initial deliveries before timing. Insert a highest-sequence row
for membership/count; replace the top title for columns. Equivalent queries
are identical and distinct query score thresholds are0..fanout-1. Every expected
callback fires exactly once for the measured changing mutation, with exact
current payload; restoration events are consumed outside timing.

Record invocation→first/last callback and successful core mutation reply
observed by the host→first/last callback. The latter timestamp is captured by a
transparent channel wrapper when the mutation reply arrives, before the SDK
schedules post-commit delivery; it is not a claim to instrument the SQLite
commit instant. Also retain decoded SDK return time and callback relative
ordering, so a callback occurring before an outer caller resumes is visible.
Do not clamp or silently reinterpret signed relative order. The existing
60000ms watchdog bounds one awaited callback only; it is not an SLO or an entire
cohort/setup timeout. Missing/extra/wrong callbacks fail the sample.

The main page uses real `performance.now()` for all endpoint timestamps and
captures10000 consecutive clock reads to report the minimum observed positive
delta. Store original finite millisecond durations and separately convert to
rounded integer nanoseconds for the existing summary format; check safe integer
and nonnegative bounds. Nanosecond representation is not clock precision or
hardware resolution. Empty/unobservable resolution is `unknown`, not zero.
UTC synthetic context clocks and deterministic retry clocks are labelled
separately and never used as performance clocks.

## Cold classes

Ten fresh worker executions for each declared class: empty create for memory
and protected OPFS; protected same-key populated reopen at N100/1000/10000;
protected v1→v2 optional String `benchmark_extra` upgrade at N1000. Prepare each
protected closed fixture outside timing and use each fixture exactly once for
its reopen/upgrade measurement. Do not copy a live file. A fresh worker creates
a fresh facade/SQLite connection; browser module/download/OS caches remain
uncontrolled, so call this worker-cold, never cold disk.

Record worker-creation→SDK-ready separately from SDK-open-invocation→decoded
ready. The former includes actual ready/bootstrap/asset/KDF/validation cost;
the latter starts after worker readiness but before protected initialization
and key-bearing open. Memory has no cross-worker reopen. Any retained in-worker
memory reopen/migration is labelled a different warm functional/resource class.
Emit a harness-only READY/1 notification after decoded SDK open and before any
full-row proof. The controller receipt endpoint includes browser message/HTTP
scheduling; it is not the worker's inner open timestamp or native process-ready
metric. Record both endpoints without substituting proof-completion time for
actual readiness.

## Independent proof and repeatable orchestration

Proposed new files, after review: `tools/javascript/benchmark/` for a page,
SDK operation harness, tests, and a separate untimed resource worker;
`benchmarks/browser_server.py` for a task-owned stdlib loopback controller and
raw evidence capture. There is no new npm/framework/browser automation
dependency. The parent can operate the local browser with existing Paseo tools.

The controller imports the existing `benchmarks/run.py` `ProofCapture` and
literal oracle rather than implementing another expected dataset. Outside
timed endpoints, the page posts bounded batches of canonical PROOF/1 lines
(<=100 records per message, existing frame bounds). The Python receiver checks
the literal expected typed value/order, completeness and SHA256 incrementally;
only counts/hashes enter durable results. A browser-side digest alone cannot
prove correct values. Keep all endpoints loopback-only and all payloads
synthetic; output static failure kinds rather than application row dumps.

Source/package/lock/workload hashes are frozen before any run, then rechecked
afterward. Record UA, OS/architecture/CPU/memory when obtainable, SDK/ABI/profile,
actual cipher capability, worker/run IDs, exact bootstrap and measurement
commands, clock observations and visible-page state. Power/thermal/background
conditions unavailable to the harness are `unknown`. Unexpected concurrent
build/test/agent load marks the entire affected cohort contaminated. Retain it
and repeat in a coordinated quiet window; never remove slow successful samples.

No timed run begins before parent quiet-window GO. A first complete pilot
cohort uses unchanged counts and is retained as a pilot with actual wall time;
the parent reviews observed cost before the full matrix. Pilot data is not
silently pooled into the accepted five runs.

## Untimed resource and footprint classes

Timed cohorts use the unmodified ordinary package worker. Separate resource
cohorts may use a harness-only worker importing the same ordinary generated
module and production worker-host controller to observe the real instantiated
`WebAssembly.Memory.buffer.byteLength`. Normal SDK operations still traverse
the public host. Label these harness-worker measurements separately; never
pool their timings with ordinary-worker samples. Linear-memory allocation is
not live Rust allocation, JS heap, renderer RSS or total browser memory.
This is explicitly the harness worker's allocation. The ordinary SDK worker's
linear-memory metric remains unknown when its production interface cannot
expose it; matching module bytes do not prove identical per-worker allocation.

Feature-detect `performance.measureUserAgentSpecificMemory()` in a separately
identified secure cross-origin-isolated fixture. Preserve its actual returned
types/attribution and total; do not assume its implementation provides disjoint
JS/WASM numbers or subtract a guessed total. If unavailable, rejected or timed
out, record that metric as unknown. The [primary WICG proposal](https://wicg.github.io/performance-measure-memory/)
allows implementation-specific grouping, including combined worker/page
attribution. The [Chromium implementation guidance](https://web.dev/articles/monitor-total-page-memory-usage)
requires feature detection and notes measurements may await GC. Resource
collection is never inside latency clocks; no browser flags or forced-GC
speed configuration is added. Thirty open/observe/close cycles record actual
ready/post-close samples and retained deltas, without labelling all growth a leak.
Those cycles use N100, one membership observer, explicit baseline/after_open/
after_close checkpoints and a bounded BROWSER-RESOURCE/1 checkpoint/CONTINUE
exchange. Validate the complete fixture each cycle and retain independent
first/final PROOF/1. These are browser metrics, never Darwin RSS units.

The two browser resource classes have declared, different store semantics.
`browser-sdk/memory` opens a fresh in-worker memory database each cycle.
`browser-sdk/opfs-protected` is a same-owned-store reopen class: one SDK pool
owner may exist at a time and the pool has 24 slots, so thirty fresh protected
names cannot be removed between cycles without a second pool owner. Cycle0
creates and seeds one exact owned closed logical store with N100; cycles1–29
reopen that closed store with the same key, without reseed, reset or deletion.
Every cycle validates the complete fixture; seeding precedes the cycle0
after_open checkpoint and is not a resource endpoint. Controls verify exact
cleanup, neighboring canary and pool only after the final close. This class is
never pooled or compared with the native thirty-fresh-store resource class.

Read-only OPFS resource collection records actual regular-file sizes under the
task origin's SDK pool, including unassigned preallocated pool files. Report
physical pool bytes separately from closed logical encrypted main/journal/WAL
lengths obtained through exact-name default-off controls. Missing journals are
explicitly absent, not unsupported metrics invented as zero. Measure at named
closed states; active-file export remains forbidden. Never call JSON length
or `navigator.storage.estimate()` an exact SQLite file size. Never remove the
whole SDK root. If an API does not expose a metric, record unknown.

Package footprint includes actual local tarball compressed bytes, installed
JS/WASM asset/notices bytes, and fixed-mtime gzip size with the command recorded.
Do not substitute the Rust executable or source tree size for a JS package.
All memory/storage/package observations identify their own artifact and tool.

The task origin permits only one active SDK/controls worker at a time. After
each protected run close all owners, terminate the ordinary worker, then inspect
and remove only its recorded owned names through controls. Prove neighboring
canary bytes and pool assignment before/after; terminate controls before the
next worker. Origin-wide deletion and changes to production pool sizing are
out of scope.

## Behavioral REDs before implementation

1. Metadata validation rejects altered Schema A, any missing/extra cohort,
   modified5/30/5 or10-cold counts, and accidental Cartesian products.
2. A compiling operation-interface stub fails exact expected point/batch/queue/
   checkpoint results; the implementation calls actual SDK methods. Managed
   reaction tests fail when an expected callback is missing, extra, wrong or
   supplied by a manual polling shortcut.
3. Bounded proof receiver uses the existing independent oracle and rejects wrong
   Float kind, absent-versus-Null changes, repeated/missing pages, wrong IDs and
   mismatching source/artifact hashes. No checksum-only correctness claim.
4. Known clock traces pin endpoint conversion, relative callback order and
   observed resolution; nonfinite/negative/unsafe durations reject. Real timer
   sample collection is distinct from these deterministic unit tests.
5. Cold orchestration requires ten distinct worker identities and retains
   bootstrap/open endpoints and failed launches; one reused worker cannot pass.
6. Unsupported memory capabilities produce unknown, exact linear memory stays
   separately labelled, and resource cleanup rejects foreign/path names.
7. Existing strict summarizer tests remain unchanged; missing, failed,
   contaminated, mixed-profile/runtime/workload or incomplete proof cases cannot
   produce an accepted baseline. No budget file is generated or modified.

After these RED/GREEN tests, run unchanged relevant ordinary/instrumented SDK
functional/security checks on matching release artifacts, then request only the
already-planned coordinated quiet-window run. This proposal neither starts
measurement nor invents a numerical performance threshold.


## Implementation checkpoint

The SDK operation fixture, exact dataset proof transport, warm orchestration and
independent loopback collector have initial behavioral tests. The matching SDK
functional/security package matrix is recorded separately in
[the receipt-wake report](../docs/implementation/phase-seven-web-receipt-wake.md);
those checks are not performance samples. Storage-cleanup implementation and
callback-entry/native-payload alignment first failed on oracle-cost timestamping
and the two differing native mutation payloads. The callback timestamp is now
captured on entry before validation/accounting; the membership/count row is
exactly `benchmark:observation`, and columns use the native `observed-<iteration>`
title. Browser insertion uses a public one-write SDK batch; its class is
distinct from the Rust facade's insert call. Earlier checkpoints recorded 34
harness, 14 collector/HTTP and 14 actual WASM/core fixture cases, then 46
harness cases and 35 Python discovery cases.

Later behavioral RED/GREEN cycles added: cold HTTP admission; ten-worker cold
platform/flow with an untimed closed fixture seeded by a separate retired worker,
same-key reopen, the reviewed v1→v2 upgrade and a post-open schema-version check;
the two resource classes above with a harness-only worker reporting linear memory
over a separate MessagePort, 61 acknowledged checkpoints and per-cycle fixture
validation; profiler records beyond the bounded frame recorded unknown with their
size; exact-name closed journal/WAL observations; a read-only physical pool
inventory taken only while no worker holds the pool; actual package tarball,
installed-file and fixed-mtime gzip footprint; and the
[between-worker admission protocol](admission-plan.md) with a contaminated default.

Actual outside-checkout installed-package verification with measurements
disabled and a synthetic endpoint clock passes point and all three one-observer
modes on both engines, all six declared cold classes and both resource classes,
with SQLCipher4.19.0, exact closed-file cleanup, unchanged neighboring canaries
and pool `[24,0]` before/after. In the verification browser host,
`measureUserAgentSpecificMemory` exists but rejects, so that metric is unknown.
A bounded admitted integration accepted two contaminated cold memory-create
workers with a held state between them, rejected an unpermitted page start
without creating a worker and verified both proofs in the independent collector.
These are correctness evidence, not latency or resource baselines.

Open before measurement: final frozen-source re-verification and the matching
package functional/security gates, then the coordinated quiet-window pilot and
full matrix through stdin admission. No browser timing or resource baseline
exists. The interrupted native warm experiment remains open; its completed
records and failed in-flight record are preserved. Existing native workload/
schema/cohort/count metadata is unchanged by this browser lane.
