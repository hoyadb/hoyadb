# Phase 6 browser security verification

The real OPFS worker passed **40/40 protected groups** on 2026-10-24 in
Paseo 0.12.0 / Electron 44.2.0 / Chrome 152.0.7977.76 on macOS. The page was
served from `http://127.0.0.1:8775` in a secure context with
`crossOriginIsolated=false`; no browser security setting was changed.

[Protected runtime output](phase-six-browser-results.json),
[legacy runtime output](phase-six-browser-legacy-results.json),
[pre-runtime source/artifact hashes](phase-six-browser-source-sha256.json) and
[compile/test output](phase-six-browser-test-output.txt) retain the evidence.
All 123 source and two generated-file hashes matched again after every runtime
flow. The profile was optimized release, built with Rust 1.98.1, LLVM 22.1.8
and the task-local wasm-bindgen CLI 0.2.129. Exact commands are in the
[browser README](../../spikes/storage-browser/README.md).

The approved runtime uses SQLCipher 4.19.0 / SQLite 3.53.4 through
sqlite-wasm-rs 0.6.2 and SAH-pool VFS 0.3.0, with the reviewed rusqlite Git pin.
Each encrypted shared factory checks the adapter's actual protected capability.
The production constructor probes the actual connection's version,
AES-256-CBC, 4096-byte pages, HMAC-SHA512, v4 KDF/profile, zero plaintext header
and active cipher status. LibTomCrypt 1.18.2 is established from the locked
WASM build/source selection, **not a separate runtime provider-PRAGMA query**.
See the [dependency packet](phase-six-dependencies.md).

## Protected execution

Thirty-five shared groups execute **79 unchanged named assertions**: typed
values/query/batch, schema/model, reactive/projection, five large-operand and
retry regressions, 14 command cases and 13 entity/pull/reset cases. A single
scoped factory supplies an externally provided synthetic key. It owns a
zeroizing key buffer and clears it on every returned success/error. A WASM
panic requires worker termination. The command capacity-500 boundary passed
in **47,513ms** without reducing its configured capacity or assertions.
Oversized TEMP operands preserve the real SAH file inventory.

Five host groups additionally prove:

- Absent/malformed foreign keys create no database or physical-file changes.
- Actual encrypted rows, identities, command results, frozen retries and field
  clocks round-trip, while pooled main/sidecar bytes contain no tested domain
  canary, key bytes/hex or plain SQLite header.
- Four context mutations throw through an awaited handler; the actual outgoing
  envelope retains the original identity. Diagnostics contain only the exact
  ten numeric/Boolean contract fields. Explicit retry data remains complete.
- An independent unkeyed adapter, a wrong key and protected opening of a legacy
  plaintext store all reject, with complete physical file hashes unchanged.
- A changed byte inside the first authenticated encrypted SQLite page rejects
  without any file/checkpoint/recovery commit. Restoring only this closed owned
  test file permits correct-key reopening.
- A worker is terminated with an open owner after acknowledged committed reads.
  A later worker and actual page reload receive the key again and recover exact
  rows, checkpoint/progress and byte-identical frozen retry envelopes.

The raw pool scan covers its 4096-byte physical metadata header and all stored
bytes; the tamper offset is after that header. The namespace is unique and
owned by this run. Cleanup removed only its files, with no origin-wide purge.
The postcommit worker termination is distinct from a precommit interruption;
this browser run does not claim a precommit crash or native WAL test.

## Earlier-phase preservation and boundary tests

On the **same generated artifacts**, the explicit unprotected legacy flows
passed persistence **36/36 before + 12/12 after actual reload**, reactive
**17/17**, command **13/13** including large-message paging and three reload
stages, and entity **11/11** including actual mixed HTTP and two reloads.
These preserve the earlier assertions but do not count as encryption evidence.
The first persistence stage was recaptured once because the historical page
keeps only its namespace across reload; both stages are now retained together.

Meaningful worker routing RED preceded its implementation
(`target/phase-six-browser/keyed-routing-red.txt` → `keyed-routing-green.txt`).
The pre-upgrade real worker initialized successfully but rejected the first
keyed shared test control; its old WASM hash and cleaned namespace are retained
in `target/phase-six-browser/security-runtime-red*.json`. That older page's
profile label was a target intention, not evidence about its historical binary.
An intermediate debug host fixture correctly hit `PullConflict` when it tried
bootstrap after a live command. Moving bootstrap before that live command
obeys the existing recovery policy; no production behavior was weakened.
The intermediate output is retained under `target/phase-six-browser/`.

Current local checks passed: debug/release WASM builds, browser formatting,
WASM Clippy with warnings denied, **33 Node cases**, and syntax checks for all
17 browser JS/MJS files plus the shell gate. Node uses the real worker router
with stub exports for policy/error tests; those tests are not cipher proof.
Post-reload browser console capture was empty; it is not a capture of every
console event before navigation. Actual HTTP is exercised by the existing
caller-owned command/entity transports; the new local security fixture uses an
explicit caller-provided pull response.

## Limits

OPFS protection reports `backup_exclusion="unsupported"`; no persistence
request substitutes for an OS backup control. Application key entropy,
storage/rotation and foreign-runtime/process memory copies remain external.
The synthetic application uses session storage for expected reload comparison
values, never its key; explicit application data access is not covered by a
promise to encrypt every app-created copy. No export, telemetry, auth backend,
mobile, FIPS, rekey/conversion or new production JS SDK is introduced.

The phase-wide native/process/backup, container and final review gates are
reported separately. This report covers the executed browser lane only.
