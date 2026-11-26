# Swift SDK foundation checkpoint (Phase 7 remains in progress)

The local unpublished SwiftPM HoyaDB product consumes the general owned native
facade and caller-defined schemas. This checkpoint records current macOS tests
and packaging evidence, not completed Phase7 or current iOS runtime support.
No remote package publication, physical-device execution or arm64 macOS support
is claimed. The33-source-file inventory/hashes are in
[swift-foundation-evidence.json](swift-foundation-evidence.json); generated native
libraries remain ignored.

## Observed current checks

- [Full Swift suite](swift-full-formal-current.txt):35 tests,0 failures, compiled
  with swift test --package-path bindings/swift -Xswiftc -warnings-as-errors.
- [Strict formatting](swift-format-current.txt): FAILED. The actual output flags
  ObservationTests.swift:19 OnlyOneTrailingClosureArgument. An earlier checkpoint
  summary incorrectly called this exit0/empty; that reporting error is corrected
  here. Preserve the failing output, fix the closure style after the quiet window
  and source checkpoint hold, then rerun lint and affected verification.
- DEF-0012 preparation33a9d97 preceded the
  [formal regression RED](swift-prepare-observe-formal-red.txt): removing only
  the prepare_push observation trigger produced the expected missing Sent
  callback. Restoring it produced the35-test GREEN above. Earlier exploratory
  evidence under target is distinct and is not presented as post-preparation.
  The trigger schedules observation delivery without waking the sync driver
  merely because it checked its due queue.
- The shared post-C-ABI Apple release bus has
  [corrected behavioral RED](swift-admission-bus-red-valid-setup.txt) and
  [two GREEN cases](swift-admission-bus-green.txt). Actual foreign CABI release
  arrived while a false begin reply was suspended, then the Swift command
  resumed without polling. A failed terminal attempt emits a payload-free
  pulse, reads do not. This uses the common native bus, not an actual RN runtime.
- [Ordinary outside-checkout macOS build](swift-final-macos-build.txt) succeeded
  with warnings as errors. Its [independent writer/reader runs](swift-final-macos-results.json)
  both exited0, covering public API-only schemas, scoped async cancellation,
  observers, real retained memory migrations, entity/merge/reset, protected
  persistence, lossless data, frozen retry and guarded mapping. Raw
  [writer](swift-final-macos-writer.json) and [reader](swift-final-macos-reader.json)
  outputs remain separate as lossless JSON stdout wrappers, preserving their original hashes and final blank lines. The caller root is task-owned and recorded in the
  command result, not a user data directory.
- [Optimized Apple artifact manifest](apple-artifact-build-manifest.json)
  records60 stable source inputs, commands and output hashes for macOS x86_64,
  iOS arm64 and universal x86_64/arm64 simulator slices. A build-only slice is
  not a runtime support claim. The current manifest includes readonly ABI1
  open preflight; the Swift owner still uses the checked binary-key open path.

## Current iOS runtime cause is unknown

The current outside-checkout iOS simulator
[build](swift-final-ios-build.txt) succeeds with warnings as errors. Its current
runtime rerun is unverified: the first install timed out after180s; an owned-only
reboot was followed by a successful install, then simctl launch --console timed
out after90s. The [latest exact traceback](ios-final-runtime.txt),
[completed commands](ios-final-commands.json), and
[first attempt](ios-final-first-install-timeout-runtime.txt) are retained.
No writer/reader result file was found in the task-owned simulator app data.
These observations alone do not distinguish simulator control from application
startup/hang. The task-owned simulator was shut down; user simulators were not
operated on. Investigate the actual process/startup/crash logs before retry or
any current runtime claim.

An [older advanced iOS run](ios-advanced-runtime.txt) did execute independent
writer/reader processes and [clean up its app and owned simulator](ios-advanced-verification.json).
That older binary/source cut remains historical and does not prove current
release-broker/prepare-delivery changes. Its binary hash is preserved separately.

## Remaining acceptance

The strict formatting correction/rerun, full current iOS runtime investigation/rerun, public testing product and
public injectable observation timing, original TC-0016/0017 package walkthrough,
actual mixed Swift/RN consumer, final platform/dependency/license manifests and
full Phase7 review remain. [The testing completion packet](../../../bindings/swift/TESTING_PLAN.md)
consumes the existing API-0011 scope; it has not been implemented or tested at
this checkpoint. No benchmark claim is derived from these unit-test timings.
