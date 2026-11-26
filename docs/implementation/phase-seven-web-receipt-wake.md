# External receipt wake verification

DEF-0015 is resolved in the JavaScript SDK. A successful public `receiveWire`
wakes the existing sync driver under the captured owner/generation guard.
`preparePush` remains notification-only. A delayed receipt cannot wake or drain
a replacement owner.

After preparation commit94bc84b, the permanent real WASM/core regression failed
because the settled driver did not map the external receipt before its future
retry deadline. The minimal correction passed that regression with one original
push, one mapper, exact applied outcome and no prepare loop. Full matching gates
passed:84 Node tests,40 scoped syntax checks,3 fixture HTTP tests, strict
TypeScript,16 installed-package memory groups,16 exact walkthrough cases and
the guide's first JavaScript block (exit0). Protected browser execution passed
36 groups across a real reload,9 instrumented security cases and1 competing
worker initialization-retry case.

[Evidence JSON](phase-seven-web-receipt-wake.json) retains every case,195
source hashes,8 unchanged generated artifact hashes, raw-log hashes, package
hashes and cleanup evidence. [Node output](phase-seven-web-receipt-wake-node.txt)
and [installed consumer output](phase-seven-web-receipt-wake-package.txt) are
kept. Both WASM artifacts are the previously verified optimized release builds;
this JavaScript correction required no Rust rebuild. Runtime source closure is
recorded separately from artifact build provenance.

All three owned tabs, the exact consumer server processes and temporary root
were removed; port8878 was released. Actual cases assert exact owned-file
cleanup, unchanged neighbor canary and pool baseline. Numeric pool tuples were
not emitted. Earlier runtime reports remain untouched. No mobile or performance
result is inferred, and the interrupted full native warm baseline remains open.

[Actual WASM failure](phase-seven-web-receipt-wake-red.txt) and
[matching successful regression](phase-seven-web-receipt-wake-green.txt)
are retained byte-for-byte alongside the source correction.
