# Phase 7 JavaScript SDK checkpoint

The private local `@hoyadb/client` and `@hoyadb/testing` packages passed the
JavaScript lane's release consumer matrix on 24 October 2026. This is a lane
checkpoint, not overall Phase 7 acceptance. The original TC16.8 unprefixed
surviving-command ID remains blocked by the recorded policy question; mobile
targets and performance budgets require their own evidence.

This snapshot predates the subsequent DEF-0009/DEF-0010 facade budget review.
Its hashes and results remain valid for the listed artifacts; it is not proof
of their later fixes. Matching rebuilt artifacts must pass the affected SDK
matrix before the parent treats the lane as final.

The [evidence matrix](phase-seven-web-pre-review.json) records every case, all 183 source
hashes, eight ordinary/instrumented generated-artifact hashes, tarball hashes,
raw-log hashes, runtime identity and cleanup proof. Exact source/artifact hashes
matched before and after execution. HEAD can advance for other lanes or docs;
the file hashes identify this checkpoint. The parent release-build commands are
retained in `target/phase-seven-web-release-{build,ordinary,memory,controls}.txt`.
The package's pack-time manifest is a source snapshot, not standalone proof of
which sources compiled an artifact.

## Implemented boundary

`bindings/javascript/client` delegates caller-defined schemas, typed CRUD,
queries and navigation, migrations, observations, awaited command scopes,
command/entity synchronization and passive merge proposals to the general
Rust facade/core. It contains host scheduling and ownership code, not a second
database or reconciliation engine. The portable `./host` entry does not require
browser globals, `structuredClone` or BigInt support. The browser entry manages
one explicitly initialized worker/runtime and sends encryption keys separately
as binary values. Failed opens, pending-open close, worker termination, stale
callbacks, queued scopes and retained transport replies have regression tests.

Managed observations schedule the initial drain and one unpostponed 16 ms count
flush. Core alone decides deadlines. UTC context time and monotonic observation
time are separate capabilities; missing uptime fails before subscription.
Passive merge hooks receive owned immutable inputs and return proposals checked
by core. Busy retries retain a proposal; stale pulls refetch from the unchanged
checkpoint. Close/reset invalidates host callbacks and in-flight hook lifetimes.

The testing package opens the real preloaded memory adapter, supplies explicit
clocks, and correlates application fake-transport responses using production
core codecs. Retained-memory handles preserve state through owner close/reopen
and authored migrations, then require explicit drop. No spike fixture is shipped
as the public SDK schema or dispatcher.

## Observed verification

| Check | Result | Evidence |
| --- | --- | --- |
| Portable values, translation, lifecycle and scheduler regressions | 69/69 Node tests | [Node output](phase-seven-web-pre-review-node.txt) |
| Application-only HTTP fixture | 3/3 Python tests | [Verification output](phase-seven-web-pre-review-verification.txt) |
| Source syntax | 37 JS/MJS files checked | [Verification output](phase-seven-web-pre-review-verification.txt) |
| Fresh installed local tarballs | Strict TypeScript public-export checks; actual WASM memory 11 groups | [Package output](phase-seven-web-pre-review-package.txt) |
| Original TC16/TC17 walkthrough in real memory | 15 pass, TC16.8 explicitly blocked | [Case matrix](phase-seven-web-pre-review.json) |
| Ordinary protected installed-package browser | 30 pass, same one policy block | [Case matrix](phase-seven-web-pre-review.json) |
| Instrumented security browser | 9/9 pass | [Case matrix](phase-seven-web-pre-review.json) |
| Real competing-worker initialization/retry | 1/1 pass | [Case matrix](phase-seven-web-pre-review.json) |
| Exact guide retained-memory example | Extracted first JavaScript block executed unchanged, exit 0 | [Verification output](phase-seven-web-pre-review-verification.txt) |

The independent consumer installed only the two local npm tarballs into a fresh
OS temporary directory. Its runtime imports resolve installed package exports,
including the bundled WASM and separate `.wasm` asset; no checkout-relative SDK
imports or registry downloads are used. Direct generated WASM TypeScript types
need `ESNext.Disposable` alongside `ES2022,DOM`. Portable host types do not;
runtime cleanup uses `free()` without a disposal polyfill.

The hermetic memory tests explicitly preload WASM, then make `fetch`,
`navigator`, entropy and crypto access fail. Their operations use the real
memory adapter. Protected browser tests separately exercise unrelated caller
schemas, all cell widths, relations, admitted large operand sets, authored
migrations, asynchronous hooks, managed callbacks, mixed command/entity HTTP
outcomes, actual page reload, durable queue state and exact frozen retry bytes.
Original walkthrough row, relation, subscription and queue assertions are
repeated through both actual adapters. `local:test-fixed-id-001` is tested;
this does not resolve the original unprefixed-ID policy gate.

Normal-package tests use the ordinary optimized release artifact. A separate
default-off test-controls artifact proves encrypted-file header/domain-canary
absence, exact bytes after wrong-key failure, tamper rejection/restoration,
active-file transfer/removal rejection and killed-worker recovery. Ordinary
exports are checked to exclude those controls. These are actual worker/OPFS
tests, unlike the separately labelled Node translation mocks.

Behavioral RED outputs are retained under `target/phase-seven-web/` and hashed
in the evidence matrix. They include unsupported compiling interfaces,
portable value boundaries, scope/lifecycle translation, deterministic IDs and
jitter, worker preflight, passive hooks, managed callback overlap and failed
OPFS initializer retry. Missing imports, setup failures and harness mistakes
were not counted as behavioral RED evidence. Existing legacy spike tests were
not replaced or weakened.

## Repeat and cleanup

From the repository root, after building the ordinary and test-control release
artifacts with the parent `bindings/web` commands:

```sh
npm ci --prefix bindings/javascript --offline --ignore-scripts --no-audit --no-fund
node --test bindings/javascript/client/test/*.test.mjs bindings/javascript/testing/test/*.test.mjs
python3 -m unittest discover -s bindings/javascript/consumer -p consumer_server_test.py
node bindings/javascript/scripts/build.mjs
node bindings/javascript/scripts/check-build.mjs
node bindings/javascript/scripts/freeze.mjs target/phase-seven-web/before.json
node bindings/javascript/scripts/packed-consumer.mjs --serve
```

The last command reports its owned loopback server and temporary directory.
Open its `browser.html`, reload once at `reload-ready`, then run `control.html`
and `worker-retry.html` sequentially after each preceding worker terminates.
Capture the named result globals and a second source/artifact manifest; compare
every hash. Terminating the reported parent stops only its child server and
removes its exact temporary consumer. These are test fixtures, not shipped SDK
operations.

For this run, all six task-owned browser tabs closed, verified server PIDs
33477/33716 and earlier fixture PID37988 stopped, ports 8877/8878 had no remaining
listeners, and the exact `hoyadb-sdk-consumer-HoM5vL` temporary directory was
absent. Ordinary cleanup asserted the original pool tuple. Instrumented cleanup
first asserted the neighboring encrypted canary's exact bytes and then the
original pool tuple. Numeric tuples were asserted but not emitted; none are
invented in this report. All cleanup targeted exact owned logical names, never
the whole shared SDK directory.

The fixed `hoyadb-v1` pool permits one active worker per origin, including when
logical store names differ. Multiple owners share that worker/runtime. A later
explicit open can retry a failed initializer after the competing worker is
terminated; failed key-bearing opens are never replayed automatically. The
default runtime is memoized, so replacing a terminated worker requires a page
reload or a caller-injected fresh worker. Browser backup reports `unsupported`.

Packages remain private local artifacts: project licensing/public distribution
authorization is unresolved. Notices include the reviewed engine notices and
39 locked Rust package records, including build/cfg-only entries. This report
does not claim mobile runtime support, browser heap or process-memory numbers,
latency measurements, numerical budgets, publication or complete Phase 7.
