# entity sync native entity and pull evidence

[API-0009](../../../docs/design/contracts/API-0009-entity-pull-reset.md) extends
the existing experimental Swift reference and shared real-SQLite Rust fixture.
The C ABI is unchanged. Swift owns transport, context capture, awaits and callback
lifetime; production Rust owns patches, durable recovery, field resolution,
pull checkpoints, progress and reset.

## Observed RED and intermediate verification

The added Swift reference methods initially compiled as functional stubs throwing
a typed host error. The executable compiled and linked with warnings as errors,
then `phase-five` exited 1 with all four named scenarios failing:

```text
FAIL E-N01: host dispatcher not implemented
FAIL E-N02: host dispatcher not implemented
FAIL E-N03: host dispatcher not implemented
FAIL E-N04: E-N04 writer reached committed SIGKILL barrier
native entities: 0 passed, 4 failed
```

E-N04's writer failed at the same configuration stub before its named committed
termination barrier. Actual output and compiling stub source are retained in
`target/phase-five-swift-red.{txt,swift}`. Compiler errors are not RED evidence.

The first actual implementation run passed E-N02/E-N03/E-N04 and exposed one
test assumption: an entity's updates remain blocked until its create mapping is
authoritative. The corrected E-N01 settles the create identity first, then sends
two edits to that same record together with a real command-scope write. No core
dependency assertion was weakened. The subsequent intermediate run passed all
four cases; final source-linked verification is recorded separately below.

The reviewed typed-correction extension added another real boundary regression.
Against the prior linked bridge the mixed HTTP correction was received, then
the new typed reconciliation operation returned `RPC InvalidRequest` (exit 1).
After rebuilding the shared production implementation it passed: optimistic
clock 1000 was replaced by backend-provided clock 25, a later pull at clock 24
kept the corrected value, and clock 26 updated it. The observed RED/GREEN and
compiling RED source are `target/phase-five-swift-correction-*`.

## Scenarios

- E-N01 captures trusted context once per ordinary mutation, verifies namespaced
  identities, exact same-value patches and explicit omitted optional fields,
  sends actual mixed command/diff HTTP arrays, confirms identity-preserving
  mapping and applies explicit delete cancellation with comparison undo. A real
  command-scope write to the configured table generates no extra entity diff.
  A typed backend correction verifies both authoritative field value and clock.
- E-N02 uses actual URLSession GET pages, preserves opaque continuation text,
  observes committed data and progress and reaches readiness after page two.
  A valid 360,264-byte UTF-8 response uses three bounded chunks; chunk upload
  alone publishes no rows. An invalid offset rejects without consuming progress.
- E-N03 retains the original uploaded page while a handler is suspended and
  applies it after explicit abort. A committed intervening write causes a fresh
  pull ticket and HTTP request. Failed resets preserve generation; successful
  reset suppresses remaining extracted callbacks and late pull replies. A held
  actual push response from the old generation cannot mutate or clear the new
  generation's separate transport pump.
- E-N04 runs independent writer and reader processes. The writer is SIGKILLed
  only after committing partial bootstrap and a frozen entity diff. The reader
  recovers checkpoint, rows, exact frozen bytes and retry deadline, resumes the
  second actual HTTP page and verifies reset cannot reuse entity identities.
  This is process-termination evidence, not power-loss evidence.

## Reproduction

Start the loopback reference in a separate terminal, then run from the repository
root on the validated x86_64 macOS host:

```sh
python3 spikes/common/command_http.py --port 8775
cargo build --manifest-path spikes/cross-platform/native/Cargo.toml --offline --locked
swiftc -target x86_64-apple-macosx26.5 -parse-as-library -warnings-as-errors -import-objc-header spikes/cross-platform/native/include/command sync.h spikes/cross-platform/native/tests/commands.swift spikes/cross-platform/native/target/debug/libhoyadb_native_spike.a -o spikes/cross-platform/native/target/native-commands
HOYADB_PHASE5_HTTP=http://127.0.0.1:8775 spikes/cross-platform/native/target/native-commands phase-five
HOYADB_PHASE5_HTTP=http://127.0.0.1:8775 spikes/cross-platform/native/target/native-commands
xcrun swift-format lint --strict spikes/cross-platform/native/tests/commands.swift
```

The optional `HOYADB_PHASE5_HTTP` points only at an HTTP `127.0.0.1` fixture;
the unchanged default is port 8774. Both push and pull requests retain five-second
request/resource deadlines. Raw domain responses and prepared envelopes are never
Foundation-decoded and re-encoded. Upload uses at most 128 KiB per UTF-8-safe
chunk, preserving API-0008's complete-frame bound even with six-byte escaping.

The executable creates its own canonical temporary root, passes it to children
at process startup and removes it on success. Failed runs retain their owned
root for diagnosis. Tests never reset existing user stores. Final results and
pre-runtime source/artifact hashes are in `ENTITY_EVIDENCE.json`.
