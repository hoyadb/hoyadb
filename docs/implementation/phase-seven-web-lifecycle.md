# JavaScript admission wake and stale sync checkpoint

The portable SDK now accepts the reviewed optional payload-free native release
subscription. It registers before admission/Busy retry, preserves the original
operation, and retires listeners on completion, cancellation, reset and close.
Browser channels can omit the capability. Native platform emission and mixed
Swift/Kotlin/RN execution are covered by their separate target reports.

DEF-0011 was reproduced with delayed asynchronous Channel replies: after
close/reopen, an old prepared envelope reached the replacement transport and an
old pending reconciliation reached its mapper. The correction rechecks captured
owner/generation after owned awaits and before mapper publication. Late errors
are published only by a live run. Tests also preserve current receive/reconcile,
current-owner errors and the replacement run's scheduling flags.

The Node checkpoint passes **82/82** tests, including nine admission-release and
four stale sync cases. JavaScript syntax checks passed. The full output is
[retained here](phase-seven-web-lifecycle-node.txt); source and log hashes are in
[the evidence manifest](phase-seven-web-lifecycle.json). This is host/lifecycle
translation evidence over a controlled Channel, not a new storage-engine test.

Chronology: success-path prep1d809b9 preceded its authentic two-case RED.
The first late-error probe was exploratory before follow-up prep429ea25. After
that commit, removing only the new catch liveness guard reproduced both late
error failures; restoring it passed four cases and the full workspace. The
original release-bus RED/GREEN logs remain distinct in the manifest.

Matching packed npm, actual WASM memory and protected browser reruns are pending
this checkpoint. The earlier [web SDK report](phase-seven-web.md) remains the
unchanged eb8dab9 runtime snapshot. No core, cipher, persistence format, ABI or
WASM artifact was changed by this lifecycle correction.
