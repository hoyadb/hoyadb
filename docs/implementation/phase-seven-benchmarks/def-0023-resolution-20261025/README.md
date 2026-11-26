# DEF-0023: waiter retirement across direct worker starts

Defect preparation1046fb8 precedes this regression and correction. The original
permit-transport capture and41-file closure remain unchanged. This new cut is
`0dd6d44db46ce65379036fc37c7f5ad568d0fdc21c59c2321ca87d4148cdfaa8`;
root independently rehashed all41 recorded files without mismatch.

Deterministic tests hold the collector lock across ADMIT and direct start,
forcing the previously registered waiter to reacquire afterward. The behavioral
RED has five failures, including the old waiter claiming a later permit when
the first worker both starts and finishes before the waiter wakes. All three
start routes and the newer-registration cleanup case are covered.

The correction increments a start generation in the common consume path and
wakes waiters. Each waiter checks its captured generation and clears only its
own object token. An intervening direct start retires it with the existing
non-blocking active-worker rejection, even after that worker already finished.
Consume-before-plan was deliberately left outside this correction.

The first post-fix run exposed a test-only control-log path error. Its corrected
path preserves the assertion and original behavioral RED. A later real-loopback
HTTP test covers the second-client trigger; it was written after GREEN and has
no claimed RED-first history.

Root reran the full benchmark Python discovery:90 tests, exit0. Root also reran
all JavaScript benchmark test files:79 tests,79 pass,0 fail, exit0. These checks
verify behavior only. They are not performance measurements.

The actual bounded browser integration remains contaminated. It runs an admitted
cold worker and resource worker, reaches held after each, then stops on CANCEL.
Its source hashes are unchanged; tab/server/owned temporary cleanup is complete
in the author's report. Same-tab exclusive admission prevents the direct-start
race there; the HTTP regression specifically tests a second client instead.

Copied logs/manifests/raw results retain their original bytes. Package archives
are omitted; their recorded identities remain available. The focused independent
review of this correction is pending; full Phase7 matching-artifact review and
accepted performance measurements also remain separate gates.
