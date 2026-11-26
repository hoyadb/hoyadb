# Phase 4 native command evidence

[API-0008](../../../docs/design/contracts/API-0008-command-host-spike.md)
contracts this experiment over the production `CommandDatabase<SqliteStorage>`.
`tests/commands.swift` is a reference host and executable test, not a packaged SDK.
No native/core/runtime dependency was added by the Swift host.

## RED before implementation

The native Rust C ABI and common fixture already compiled. A compiling Swift
`CommandHost.submit` stub returned a typed host error without invoking the
handler. Running the linked executable produced:

```text
FAIL H01: host dispatcher not implemented
FAIL H02: host dispatcher not implemented
FAIL H03: host dispatcher not implemented
FAIL H04: host dispatcher not implemented
native commands: 0 passed, 4 failed
```

Exit status was 1. The copied compiling stub and actual output are preserved
in `target/phase-four-swift-red.swift` and `target/phase-four-swift-red.txt`.
Earlier compile diagnostics were corrected before this behavioral RED run.

The separate compiling URLSession transport stub then produced H01–H05 PASS
and `FAIL H06: host dispatcher not implemented` (exit 1). Replacing that stub
with real HTTP made the keyed outcome/correction/inverse cases pass.
A compiling retry-pump stub produced `FAIL H06: host wake deadline exceeded`.
The implemented pump passed the 30 ms retry case with identical frozen bytes.

A further deterministic regression held an async Swift handler active while a
real URLSession response arrived. Immediate delivery attempted a core receipt
under that active permit and lost the response:

```text
PASS H01
PASS H02
PASS H03
PASS H04
PASS H05
FAIL H06: H06 overlapping HTTP final was dropped
native commands: 5 passed, 1 failed
```

A final actual conflict-response pump regression initially failed with
`H06 per-member receipt error was hidden`: the outer RPC succeeded while an
individual member reported Protocol. The host now inspects every outer member
report before clearing its owned response. The permanent test asserts one HTTP
call, preserved successful membership, and two surfaced Protocol errors after
one explicit retry of the same body. Its behavioral RED is preserved in
`target/phase-four-swift-member-red.{txt,swift}`.

The fixed dispatcher queues owned inbound work until both active and queued
command requests have released the app-wide coordinator. The native host does
not retry Busy in a loop or retransmit an already received response. Failed
receipt persistence retains the owned body for an explicit wake and surfaces
its error. Closing the owner cancels the host pump; durable sent work remains
available on reopen. Actual RED outputs/snapshots are in
`target/phase-four-swift-{http-red,timer-red,overlap-red}.{txt,swift}` (only the
HTTP/overlap runs have copied source snapshots).

The reviewed bounded-reply amendment (`7c3e822`) added four permanent native
regressions, compiled and run separately against the old linked bridge:

```text
h03-bound exit=1: failed oversized finish committed rows or envelope
h04-pages exit=1: RPC LimitExceeded
h06-pages exit=1: aggregate push failed after Sent persistence or skipped cached prefixes
h06-plan exit=1: async application plan was not retained during active handler
```

These were behavioral failures, with actual output/source in
`target/phase-four-swift-budget-red.{txt,swift}`. The revised consumer uses
`{events,has_more}` finite drain pages and page-aware prepared pushes. Drain
holds one recursive guard across every prefix, and stops on close. Application
plan work uses an awaited idle operation with owner generation, explicit queued
cancellation and close invalidation; no automatic business mapper is added.

## Reproduction

Run the loopback reference in a separate terminal from the repository root:

```sh
python3 spikes/common/command_http.py --port 8774
```

Then build and run on the validated x86_64 macOS host:

```sh
cargo build --manifest-path spikes/cross-platform/native/Cargo.toml --offline --locked
swiftc -target x86_64-apple-macosx26.5 -parse-as-library -warnings-as-errors -import-objc-header spikes/cross-platform/native/include/phase4.h spikes/cross-platform/native/tests/commands.swift spikes/cross-platform/native/target/debug/libhoyadb_native_spike.a -o spikes/cross-platform/native/target/native-commands
spikes/cross-platform/native/target/native-commands
xcrun swift-format lint --strict spikes/cross-platform/native/tests/commands.swift
cargo test --manifest-path spikes/cross-platform/native/Cargo.toml --offline --locked
cargo fmt --manifest-path spikes/cross-platform/native/Cargo.toml --check
cargo clippy --manifest-path spikes/cross-platform/native/Cargo.toml --offline --locked --all-targets -- -D warnings
```

With no `HOYADB_PHASE4_ROOT` set, the executable creates a fresh harness-owned
canonical temporary root, supplies it only in the child process's startup
environment, and removes only that created root after success. It uses POSIX
`realpath` so macOS `/var` aliases do not weaken the Rust canonical-path check.
On failure it retains and prints the synthetic root for inspection. An external
harness may instead provide a fresh root matching API-0008; that harness then
owns cleanup. No RPC or in-process environment setter changes the fixture root.

The first complete run after the reply amendment passed H01/H02/H03/H05 and
exposed two harness issues. H04 had requested membership-only rows while
expecting a label-only update; API-0005 requires `columns:["label"]` for that
assertion. The corrected test preserves the same three old/three fresh complete
snapshots and passed in 25.486 seconds. H06's aggregate four-message waiter was
five seconds, while a live stack sample showed actual SQLite receipt validation
working on its large records. Its finite aggregate watchdog is now 900 seconds;
every URLSession request/resource timeout remains five seconds and all payloads
and functional assertions are unchanged. The four-command case then passed in
80.051 seconds with HTTP arrays of three and one members. The async plan case
passed in 5.236 seconds. These debug-host timings are evidence, not budgets.
Diagnostic outputs are `target/phase-four-swift-{pages,push}-diagnostic.txt`,
`target/phase-four-swift-pages-green.txt`, and
`target/phase-four-swift-plan-green.txt`; the first complete failed run/source
are preserved as `target/phase-four-swift-first-final.{txt,swift}`.

## Verified scenarios

- **H01:** a real Swift handler stages an order, suspends, inserts two related
  items after resumption, and reads its staged relation. Ordinary reads stay
  committed and usable. The next same-owner request sees the entire commit;
  exactly one changed row snapshot is delivered and its callback sees both
  items. One bounded handler-start-to-drain timing sample is printed per run;
  it includes explicit suspension and dispatch and is not a performance budget.
- **H02:** distinct SQLite files share FIFO admission. Queued cancellation,
  thrown handlers, actual caller `Task.cancel`, explicit active cancellation,
  and owner close release admission without awaiting an uncooperative handler.
  Nested submission rejects; stale/cross-owner handles reject. A rejected repeat
  begin preserves its original active scope. Cancellation after the copied
  durable finish reply reports the committed success.
- **H03:** tagged outer JSON preserves Integer extrema, integral/nonintegral
  Float kind, timestamp extrema, Unicode/NUL strings and copied trusted context.
  Missing context, malformed Float, invalid UTF-8, duplicate keys and oversized
  input reject before handler invocation. A valid near-bound finish request
  whose complete success frame cannot fit leaves rows, envelope and counter
  unchanged. C reply callbacks copy bytes, remain
  on the calling thread and reenter after registry unlock.
- **H04:** real data/status/outcome callbacks run outside C/Rust calls. Reentrant
  reads work; unsubscribe or close suppresses remaining extracted deliveries;
  recursive drains reject; a throwing callback cannot change durable success
  or prevent unrelated callbacks. Queued/sent/acknowledged/applied outcome
  payloads omit wire routing keys and HTTP codes. Multiple complete ~400 KiB
  snapshots span pages; cancellation and close suppress cached deliveries,
  while callback-generated fresh events wait for the next explicit drain.
  A single oversized event remains observable as LimitExceeded until its
  registration is cancelled; an empty cached snapshot never pulls fresh work.
- **H05:** independent writer processes are SIGKILLed at named before/after
  commit barriers. Independent readers find no staged rows/envelope/counter
  consumption before commit; after commit they recover queued/sent/receipt-only
  and received-but-unprocessed work, exact pending results, the retry deadline
  and frozen bytes. Reconciliation resumes; live recovery rejects migration
  without altering data. This is process-termination evidence, not power loss.
- **H06:** actual URLSession one/many command-only POSTs exercise applied, mixed,
  partial, equal duplicate, conflicting duplicate and 503 responses. Bodies are
  joined opaque canonical strings, never Foundation-decoded domain text.
  Exact i64/Float echoes remain tagged after Rust decoding. Key correlation,
  missing-member retries, no early retry, application correction, ID mapping,
  rejection rollback and durable cancellation conflict guards are checked.
  Recovery-free authored migration then succeeds. A host timer retries after a
  fake failure through the same byte seam into actual HTTP; cancellation and a
  five-second request/resource timeout are configured. A real HTTP response
  overlapping a suspended command is retained until FIFO execution becomes idle.
  Per-member receipt errors are surfaced, and an explicit retry retains the
  response without performing a second HTTP transmission or recursively waking.
  Four ~300 KiB envelopes use multiple bounded nonempty actual HTTP arrays;
  async application-built plans wait for idle, preserve unrelated commits,
  and reject queued task cancellation or owner disposal without applying.

Final observed results, source/build hashes and runtime details are recorded in
`COMMAND_EVIDENCE.json`; transient full outputs are under `target/phase-four-*`.
The older native ABI/Swift executables remain separate regressions; parent-run
legacy matrix output is `target/phase-four-native-legacy.txt`.

This validates the experimental desktop host only. Mobile/other architectures,
packaged SDK ergonomics, arbitrary foreign pointers, multi-process concurrent
writers, forced task termination and crash/power-loss guarantees remain unproven.
An abandoned handler holds its slot until explicit cancellation or owner close;
no handler watchdog or automatic application correction is introduced.
