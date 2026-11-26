# Phase 7 benchmark protocol — reviewed for implementation

Authority: TC-0037, API-0011, ADR-0022 and
`target/phase-seven-prep/benchmarks.md`. This experiment was reviewed for
implementation in da7a897 and 5a9f16c;
no latency, footprint or numeric regression budget has been measured here.
The numerical budgets remain unapproved and unmeasured.

## Files and build isolation

- `benchmarks/native/`: unpublished standalone Rust2024 benchmark executable,
  depending only on existing path crates `hoyadb-bindings` and `hoyadb-core`.
  No new external runtime or benchmark framework. Parent adds
  `benchmarks/native` to the root workspace exclusion list; the package has its
  own `[workspace]` so it cannot accidentally join/inherit the main workspace.
- `benchmarks/run.py`: stdlib process orchestration, environment/source/artifact
  manifests, raw capture, cold process launches and task-owned cleanup.
- `benchmarks/summary.py`, `benchmarks/test_summary.py`: standalone stdlib
  validation/statistics with meaningful P04 RED tests before implementation.
- `benchmarks/workloads.json`: reviewed workload and endpoint definitions,
  hashed before any measured build/run. No source/artifact changes during runs.
- Raw results/logs go to `target/phase-seven-benchmarks/<owned-run>/`; reviewed
  results can later be copied into a durable implementation report. No generated
  file may silently overwrite an accepted baseline.

Proposed command after implementation:
`python3 benchmarks/run.py --output target/phase-seven-benchmarks/<run>`.
It builds once with `cargo build --release --manifest-path
benchmarks/native/Cargo.toml --offline --locked`, records the exact command,
then launches the frozen executable. Parent must review the standalone lock;
its external graph must be a subset of already-approved locked workspace graph.
No install, global environment change or new crypto provider is needed.

## Consumer classes and identity

The first independently runnable classes are `rust-facade/memory` and
`rust-facade/native-protected`, not Swift/JNI/ReactNative/browser SDK timings.
Each timed facade endpoint includes request serialization, `Facade` call, reply
parse and checked successful decode, unless explicitly labelled internal-open.
These classes do not complete the separate platform-package/FFI/mobile gates.
Same-key persistence/restart is native only; memory is never reported as durable
or encrypted. The root platform lanes must supply their matching consumer classes.

Use normal release settings and the unchanged production cipher/KDF/page/integrity
configuration. Synthetic binary key is fixed test-only32bytes and is never put
in JSON, process arguments or logs; namespace is fixed lower-case32hex. Native
stores live only in a newly owned temp directory. Facade `protection` must report
actual encryption/provider/version. Its native backup field remains
`host_managed`, not an asserted OS backup guarantee. No direct plaintext factory,
validation bypass, custom fast cipher profile or database page-cache flush.

Manifest includes source/artifact/lock/workload hashes; compiler/profile/features;
OS/architecture/CPU/memory/runtime; engine and actual cipher capability; namespace
and workload seed version; timer and units; actual process IDs; power/thermal/
background observations where accessible. Unsupported observations are `unknown`.
`std::time::Instant` exposes durations but not hardware resolution: record an
observed minimum positive tick separately as an empirical resolution observation,
never call its nanosecond storage unit a hardware guarantee. Python outer process
clock metadata uses `time.get_clock_info('monotonic')`.

## Exact synthetic dataset

The exact caller Schema A is the literal already used in TC-0036 and
`packages/client/test/schemas.mjs`; it is repeated as data in
workloads.json, never installed in production facade code. N=100,1000,10000
articles, plus N/10 authors. Integer index i=0..N-1; author index floor(i/10).
IDs `article:<zero-padded6decimal>` and `author:<zero-padded6decimal>`. Version1:

- authors: required String `name` = `author-<zero-padded6decimal>`; no extra relation.
- articles: required String `title` = `東京🌿-<i>`; Integer `score`=i;
  Float `ratio`=i+0.25; Boolean `active`=even(i); Timestamp `at`=1700000000000+i;
  optional String `note` omitted for even i and `note-<i>` for odd i;
  required ToOne `author`→authors through Reference `author_id`.
- tags: required String `label`.
- article_tags: no authored scalar columns; required ToOne `article`→articles
  via `article_id` and `tag`→tags via `tag_id`. articles.tags is ManyToMany through
  article_tags with source_relation article and target_relation tag.
- Tags and through rows are empty in baseline cohorts; the exact schema remains
  present. No accounts/items fixture C or added scalar field.

Validate every complete dataset with ordered cursor pages of at most100rows,
`score Gt previous_score`, `score Asc`, limit100 (and author IDs similarly using
id cursor). Never issue an all10k-row wire fetch. Assert exact ID/field equality
per page, total N/N10 counts and empty tags/through tables. The boundary cohort
alone adds one named extra article probe using i64 extrema in score/at plus
Unicode/integral-Float42.0, explicitly records N+1, and removes it afterward.

Seed creation is outside timing through the public general facade in bounded
batches, each at most100writes and preflighted below the existing1MiB limit.
Expected IDs, counts, sorted windows, typed values, event/subscription counts,
queue states and canonical-result checksums are validated outside timing.
Artifact manifests use SHA256 via Python hashlib. Workload results use exact IDs
and typed equality plus SHA256 of canonical returned fixture bytes outside timers.
No checksum substitutes for operation/state assertions.

## Finite matrix (orthogonal sweeps, not an accidental Cartesian product)

Every warm cohort below gets5 untimed warmups, then30 measured iterations,
in each of5 independent child processes/run IDs. The scalar sampling protocol
is identical for both first-stage engines. All sizes/fanouts/queue levels appear;
unused axes are fixed and recorded, not silently omitted. Cold classes use10
fresh child executions each. The unchanged500-capacity correctness stress is a
separate functional workload and contributes no warm latency percentile.

| Cohort | Swept axis; other axes | Exact timed operation |
| --- | --- | --- |
| point | N=100/1k/10k; Q0, no observers | fetch ID article:N/2, exact1row |
| selective | N; Q0 | `score Eq floor(N/2)`, exact1row; call it indexed only with observed engine index evidence |
| sorted-related | N; Q0 | articles order `author.name Asc`, limit10; stable core ID tie-break verified |
| batch-insert | N; Q0 | one atomic10article insertion batch; remove those IDs outside clock |
| batch-update | N; Q0 | atomically replace10fixed rows; restore exact prior rows outside clock |
| batch-remove | N; Q0 | atomically remove10fixed rows; reinsert exact rows/IDs outside clock |
| optimistic-command | N with Q0, plus N1k with Q10/Q100 | request/admit, capture context, genuine cooperative Pending→resume at caller (manual std-only future driver; not a claimed network wait), scope replace one existing row, durable finish; terminal receipt/reconcile/forget and row restore outside clock |
| entity-patch | N; Q0 | configured partial patch changes one scalar, exact diff/row/status; cancel/forget restoration outside clock |
| non-noop-pull | N; Q0 |100field updates, one bounded complete page, data+comparison+checkpoint commit; restore using a fresh newer authoritative page outside clock |
| reset | N; Q0 | explicit preserve-recovery reset of real configured populated rows+comparison state; restore/rebootstrap outside clock |
| reactivity | N1k,Q0; fanout1/10/100 × equivalent/distinct × membership/columns/count | actual mutation, drain/decode, invoke real Rust consumer closure; endpoints below |
| boundary-known | N100,Q0 | schema/protection/point row call with fixed valid wire values; record complete input/output bytes, total encode/call/decode |
| deterministic-retry correctness | N100; Q10/Q100 | injected-clock exact persisted attempts/deadlines; **not** performance-clock evidence |

Batch-insert uses new IDs article:N..N+9 and the last10baseline article field
sets (including their existing valid authors); insert never times missing-author
setup. Batch-update changes only first10titles to changed-<iteration>-<index>.
Batch-remove removes the first10exact rows. Every returned change is checked
against its exact table/ID/before/after; count-only equality is insufficient.

Q10/Q100 are real durable pending no-op commands, not an auxiliary/shadow queue.
They are dispatched once at injected1000ms with jitter0, retained Sent with
attempts1/due2000ms so per-sample pushes at1000ms send only the new command.
Entire baseline QueueStatus equality is required after each sample cleanup.
Per-sample command records are terminally resolved/forgotten outside timing so
baseline Q is restored. Every cohort checks its initial/after cardinality and
logical state. Public command key progression is recorded; no private ledger reset.
A fixed data operation reused across iterations may alternate known values or
restore outside timing; never substitute a no-op for a declared changing write.

For reaction membership/column windows, order score Desc +limit10 bounds callback
payloads; insert highest-sequence row for membership and change top-row title
for columns. Equivalent observers share one exact query; distinct observers use
different score lower bounds0..fanout-1. Count observers use the corresponding
unlimited count predicates, making insertion increment every expected count.
Record invocation→first/last consumer callback and durable return→first/last
callback, decoded bytes and exact callback count. Incomplete delivery is a
failure, not a zero-latency sample. Count polling is only this raw Rust consumer harness endpoint, not the packaged
SDK scheduler or a background service guarantee. It uses real elapsed monotonic
milliseconds and an explicitly reported1ms poll interval; the existing16ms policy
is retained. Deferred callbacks are not supplied an invented clock to accelerate
performance. Initial and restoration events are drained outside timing.

Cold native classes: empty protected create (N0), same-key populated reopen
(N100/1k/10k), authored v1→v2 upgrade (N1k, add optional String column).
Each reopen/upgrade child receives a separate closed fixture copy owned by the
runner. All post-open row/ledger checks are required. Record both outer process
launch→parent-observed decoded-ready and inner serialize/open→decoded-ready.
A harness-only `READY\n` line is emitted immediately after successful owner
decode, before PROOF pages; the parent records its receipt with monotonic_ns.
The outer endpoint includes pipe and host scheduling. Full row proof stays outside
both endpoints. These are process-cold
with uncontrolled/warm OS caches, never claimed cold disk. Memory has only a
fresh-process empty create class; retained in-process reopen/migration is a
separately labelled warm cohort, never a cross-process memory reopen.

## Resource/footprint measurements

Separate untimed resource cohorts avoid adding profiler subprocess contention to
latency samples. The Python parent uses `os.wait4(exact_owned_Rust_pid,0)` to collect that
worker's direct rusage outside the timed endpoint. Darwin `man2getrusage` defines
ru_maxrss in bytes; other OS units remain unknown until primary-source verification.
No cumulative unrelated-child resource count or compiler process is included. Current native RSS is sampled using the named OS
`ps` command at ready/post-close checkpoints over30open/observe/close cycles;
record bytes and retained delta, not a claim that all allocator growth is a leak.
The separate untimed RSS/1 worker uses N100/fanout1 membership for30 actual
open/seed/observe/close cycles. It emits bounded baseline/after_open/after_close
checkpoint JSON, flushes and waits for fixed CONTINUE from its parent. Parent
samples only the owned worker PID using `ps -o rss= -p PID`; local Darwin
`man1ps` defines rss in1024byte units. Keep raw unit/value and normalized bytes;
other OS units remain unknown until verified. First/final PROOF/1 is independently
checked; intermediate cycles explicitly validate the entire fixture internally.
No RSS measurement is aggregated as a latency sample.
Unavailable metrics are unknown. Browser JS/WASM metrics remain for browser lane.

Complete regular-file sizes inside the owned store directory are recorded at
open/closed states (main/journal/WAL if actually present); no fake zero for absent
unsupported metrics, no JSON-length-as-disk claim. Do not enable WAL/change journal
mode just for speed. Package bytes include the actual measured harness artifact
labelled as such; published-style JS/WASM/XCFramework/AAR/architecture-slice sizes
must come from their final package lanes, not this executable's footprint.

## Raw data, statistics and failure policy

One raw file per class/cohort/run contains exact manifest identity and every
warmup/measured outcome, monotonic elapsed integer nanoseconds, endpoint, input/
output bytes, sample index and correctness status. Failures/timeouts remain
recorded; no fabricated duration or success percentile. Operational watchdogs
and rejected samples are visible, and a cohort with missing/failed measurements
cannot become an accepted baseline. No sample is excluded merely for being slow.
Unexpected concurrent task workload marks the entire affected cohort contaminated;
retain it and repeat later. The initial implementation reports wall time after
a pilot complete cohort before launching the full frozen matrix, allowing root
to identify actual contention/cost without inventing a performance limit or
silently reducing counts. Any count/matrix change requires documented review.

P04 tests precede summary implementation:

1. [1..30] → median15.5, nearest-rank p95=29; odd [1,2,100] → median2,p95=100.
2. Single value, repeated values, unsorted samples, exact non-interpolated rank.
3. Empty/malformed/missing samples, negative/non-finite values and Boolean-as-number reject.
4. Mixed ns/ms units, debug/release profiles, engine/runtime/artifact/workload
   identity changes reject instead of pooling incompatible classes.
5. Missing run/sample indices, duplicate samples, failed/contaminated samples or
   absent correctness proof block an accepted summary, while preserving raw data.
6.5run summaries preserve each run's own median/p95 and observed upper envelope;
   do not label pooled warm/cold or5independent runs as one independent tail.
7. Gate reads an explicit reviewed budget, rejects a synthetic exceedance and
   never updates budget/baseline files. Budget format/gate only becomes binding
   after actual P05 operator review of measured repeatability.

P05/P06 remain open until observed numbers and a same-class repeat run exist.
Candidate Target is central run-level figure; candidate Must is observed repeated
upper envelope with paired same-machine control rerun. No guessed SLO/percentage.
Unexecuted required mobile/browser/package/metric classes remain explicit open
Phase7 acceptance gates.

Primary timing/resource references consulted during preparation: [Rust Instant](https://doc.rust-lang.org/std/time/struct.Instant.html), [Python time](https://docs.python.org/3/library/time.html), [Python resource](https://docs.python.org/3/library/resource.html). Latest Rust docs expose durations/methods but no resolution getter; the release currently served is1.99.1, so its platform clock table is not proof of this project's pinned1.98.1 implementation. Actual local Python3.14.8 Darwin clock info reported mach_absolute_time, monotonic/nonadjustable and nominal1ns resolution; benchmark must capture it anew, not hard-code that observed host value. The version-pinned1.98.0 documentation URL was unavailable through the browser tool.

Approved operational callback control: callback_watchdog_ms=60000 applies only
to one awaited callback delivery, never to an entire35sample cohort,10k setup
or restoration. It is a failure watchdog, not a latency SLO or altered core
count deadline. A timeout retains a failed raw sample and prevents an accepted
summary. No performance measurements precede the reviewed metadata commit.

Approved harness-only proof protocol PROOF/1: before/after cohort, actual public
fetch cursor pages<=100 feed one bounded canonical record at a time to worker
stdout, framed with PROOF_BEGIN/PROOF_END and PROOF prefixes. Record is
{table:String,row:typedRow|null}; order articles scoreAsc, authors idAsc, then
one verified-empty null marker for tags and article_tags. Python consumes
linewise, checks an independent literal-A typed oracle and hashes canonical UTF8
record+newline with hashlibSHA256 (prefix excluded). Persist only hashes/counts
and static failure information, never unbounded proof payloads. Business numeric
values remain typed decimal strings, so no host float roundtrip. Both phases
are outside all timed endpoints. Unexpected output, error, timeout, incomplete
proof or wrong typed value fails acceptance; hashing two matching bad mappers
without independent oracle is insufficient. No new SDK streaming/export API,
handwritten crypto or runtime dependency is introduced.
