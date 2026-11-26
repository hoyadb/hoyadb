# Swift public testing and lifecycle checkpoint

This checkpoint extends the earlier [35-test foundation](../phase-seven-swift-foundation/README.md).
It is partial Phase7 evidence. Current iOS runtime acceptance and the React
Native/Kotlin final platform matrix remain open; this is not Phase7 completion.

## Verified current cut

- [44 tests](44-green.txt) passed with warnings as errors after formatting.
  This includes the original35, six ordinary-import HoyaDBTesting scenarios,
  and three held-success lifecycle regressions.
- [Strict formatting](strict-format-green.txt) returned exit0 with empty output.
  The old foundation's failed formatting evidence is preserved unchanged.
- An actual [outside-checkout consumer](outside-consumer-results.json) compiled
  with public HoyaDB/HoyaDBTesting imports and ran independent writer/reader
  processes. It exercised real memory/protected stores, the original TC16/17
  walkthrough, authored migrations, async commands/cancellation/FIFO, entity
  pull/merge/reset, wrong key, lossless values and frozen recovery. Its
  [build command](outside-consumer-build-command.json) and
  [output](outside-consumer-build.txt) are retained.
- [38 source-file hashes](source-sha256.json) identify this Swift cut. Nested
  .build directories and generated native Artifacts are excluded. The
  [native build manifest](native-build-at-run.json) records exact compiled Rust
  inputs and framework bytes. Every recorded native artifact hash was checked;
  [native-source-drift.json](native-source-drift.json) also shows no source drift
  against that manifest at this checkpoint. [Verification](verification.json)
  records the observed checks and remaining limits.

## TDD chronology and corrections

The public testing preparation was reviewed/committed at53330df; DEF17/18
preparation was committed atdac055f before these runs.

1. [Initial compiled RED](initial-red.txt): ST01 ignored the injected timing;
   memory/manual helper stubs failed; DEF18 hid a real committed finish with
   Closed after close/reopen. The first DEF17 fixtures incorrectly supplied
   null where command arguments require an object. Those two initial failures
   do not count as reproductions of DEF17.
2. Correcting only those fixture arguments produced the
   [two DEF17 behavioral REDs](stopped-driver-red.txt): real held prepare and
   pending replies newly invoked retired transport/mapper callbacks.
3. After the minimal lifetime guards and memory/timing helpers,
   [those cases passed while FakeTransport remained RED](helpers-unblocked-fake-red.txt).
   The latter failures reached its actual invalidEnvelope stub; no harness or
   compiler failure is counted as fake implementation proof.
4. The pure fake was implemented with production lossless JSON helpers and
   exact command/entity correlation. [All44 passed](44-green-before-format.txt),
   then formatting and the final44-test rerun passed separately.
5. An initial [CLI consumer invocation](consumer-missing-root-failure.txt)
   omitted its required HOYA_CONSUMER_ROOT. It completed the memory walkthrough
   before the CLI harness intentionally rejected the missing result root.
   A fresh recorded canonical root passed; the final outside-consumer
   writer/reader used their own recorded root. This was a caller setup error,
   not a protected-store defect.

The production changes are narrow: a stopped driver's successful suspended
stage checks cancellation/liveness again before another application callback;
a submitted finish returns its actual native outcome across close while only
the original live owner schedules notifications/sync. Existing Busy retention,
persisted retry deadlines, stale-scope rejection and pre-submit cancellation
remain covered. The public helper product adds no dependency, alternate
database, queue planner or conflict resolver.

## Remaining platform gate

Current iOS runtime acceptance is **OPEN**. The preserved
[earlier launch timeout](../phase-seven-swift-foundation/ios-final-runtime.txt)
has an unknown cause; it is not evidence of either an app failure or a simulator
control failure. Older iOS PASS results cover their explicitly older source cut.
No new44-source simulator or physical-device result is claimed here.
