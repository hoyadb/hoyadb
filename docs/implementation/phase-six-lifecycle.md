# Phase 6 pending-open lifecycle correction

The JS client could publish an owner after `close()` had already resolved.
A second concurrent open could also overwrite its only handle and orphan the
first real owner. The isolated reproduction and output are retained in
`target/phase-six-client-lifecycle/probe.mjs` and `output.json`.
API-0010, SPEC-0032 and TC-0035 S10 were clarified/reviewed and committed as
`c3dfcf8` before permanent regression tests or implementation.

`CommandClient` now reserves one private open attempt before invoking transport.
Another protected or explicit legacy open rejects Busy. Close cancels publication,
waits for the original reply and, if it acquired a handle, waits for exactly one
acknowledged disposal. The cancelled opening rejects Closed. A failed backend
open needs no fabricated close. Failed disposal retains only the attempt's
result/handle for a later close retry; concurrent closes share in-flight cleanup.
No key/configuration is stored on this private attempt, and an old key-bearing
open request is never replayed. Active-owner generation invalidation remains
synchronous, preserving close-from-callback suppression and committed-finish
handling. An indefinitely suspended transport can keep close pending; no timeout
or underlying filesystem rollback is promised.

Five meaningful permanent regressions in
[`command-client-security.test.mjs`](../../spikes/storage-browser/command-client-security.test.mjs)
first failed for the observed behavior (**five RED, three existing passing**),
then passed with the minimal client-only change. The full suite passes
**38/38** including all earlier immutable-context/error/client assertions.
All **18 JS/MJS syntax checks** and the focused diff check pass.
[RED/GREEN output](phase-six-lifecycle-test-output.txt) retains actual results.
No crypto, dependency, Rust, WASM or generated binding change was made.

The separate [`lifecycle.html`](../../spikes/storage-browser/lifecycle.html)
page suspends actual worker replies after the Rust operation, rather than
substituting a storage engine. Four real groups pass:

- Protected and legacy opens each reject duplicate opens, never publish their
  late cancelled handle, wait for the held disposal acknowledgement, dispose
  once despite concurrent closes, and independently reopen exact committed rows.
- A real wrong-key engine failure has its reply suspended; close waits, then
  resolves without sending a close for a nonexistent owner.
- An explicitly simulated caller transport failure **before** close reaches the
  worker leaves its actual owner alive and blocks worker shutdown. Replacement
  opens remain Busy; two later concurrent closes perform one actual disposal.

The updated client also passes the five protected host groups including real
reload/tamper/nonmutation, **13** existing command groups including large-message
paging/three reload stages, and **11** entity groups including real HTTP/two
reloads. All four fresh test namespaces were closed and cleaned.
The first attempt encountered a stopped shared HTTP fixture before worker
initialization; restarting that same reviewed fixture restored execution. This
was infrastructure availability, not a passing runtime or a production defect.

[Runtime output](phase-six-lifecycle-results.json) and the separate
[source/artifact manifest](phase-six-lifecycle-source-sha256.json) preserve the
new execution. All **126 source + two generated artifact hashes** matched after
runtime. The generated SQLCipher WASM/bindings are byte-identical to the earlier
[full protected browser proof](phase-six-browser.md), whose frozen manifest and
79 unchanged shared assertions remain intact. Those Rust/shared tests were not
rerun solely for this JS lifecycle fix. The old manifest intentionally records
its earlier client source; it is not relabeled as the corrected JS source.

The actual runtime remained Paseo 0.12.0 / Electron 44.2.0 /
Chrome 152.0.7977.76, optimized WASM, at `http://127.0.0.1:8775`, with standard
browser security and application-supplied synthetic keys. No new platform,
backup/key-management guarantee or release action is introduced.

Cleanup completed after the narrow review passed: the four task-owned tabs
were closed after every runtime namespace reported successful cleanup. The
owned HTTP process was verified by exact command, repository working directory
and its task-specific stdout path, then stopped with SIGTERM (exit 143); port
8775 had no remaining listener. No unrelated process, tab or store was touched.
The [cleanup record](phase-six-lifecycle-cleanup.json) retains those checks.
