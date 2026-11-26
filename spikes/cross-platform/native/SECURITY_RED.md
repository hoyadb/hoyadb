# security native supplied-key and backup proof

API-0010 / TC-0035 S08/S10; reviewed preparation commit `6f5ffb4` and approved dependency ADR-0034
(`e318c4a`) precede the protected host tests. The fixed 32-byte key is a synthetic
external test input. This reference host does not generate/manage application
keys or expose arbitrary filesystem/SQL/import capabilities.

## RED and fixes

- Dependency-free backup stub compiled against the previous native archive;
  five cases failed behaviorally (default/opt-in hint, malformed key, setter
  failure and open replacement). The implemented Foundation helper passes.
- A sixth compiling regression proved a second canonical pattern-valid task
  root could receive backup attributes before the common RPC rejected its path.
  The supplied-key constructor now requires the exact pre-established root
  before applying any policy. Six backup cases pass.
- After the approved dependency ADR, the three actual protected-host cases
  compiled/linked against the old archive and all failed at missing `open_secure`
  with typed `InvalidRequest`. They now pass against the cipher archive.
- Two first-run new test setup mistakes triggered existing `Protocol` / `PullConflict`
  rules. Initial entity binding and initial pull now precede local live work;
  no production code, guard or assertion was weakened. Failed logs remain.

## Actual results

| Executed suite | Result |
| --- | --- |
| Actual Foundation policy/input/fault/replacement helper cases | 6 passed |
| Protected foreign owner/cipher/reopen/refusal/replacement cases | 3 passed, plus independent reader |
| Existing H01–H06 through protected factory | 6 passed |
| Existing E-N01–E-N04 through protected factory | 4 passed |
| Same H/E scenarios through explicit unprotected factory | 6 + 4 passed |
| Earlier native Swift main / experiments / reactive cases | 5 / 3 / 4 passed |
| Earlier persistence independent writer and reader | both passed |

Protected H/E ran in 45.07 s, unprotected in 44.57 s on the same debug archive.
Existing assertions exercise suspended handlers, FIFO/abort/cancellation,
callback liveness, bounded HTTP arrays of 3 + 1, exact tagged scalars, partial bootstrap
and 360,264-byte chunked pages, authoritative correction clocks, guarded inverse,
reset and independent SIGKILL writer/readers. The protected factory is explicitly
selected before the child process starts and inherited by writer/readers. Neither
owner constructor silently falls back.

The three protected cases verify SQLCipher 4.19.0 capability, actual keyed data,
command/entity/checkpoint recovery, exact frozen retry bytes across reopen,
wrong/missing/plaintext rejection with identical main/sidecar snapshots, no
application canaries/key hex/plain SQLite header in raw main/any present sidecars,
real closed encrypted-file replacement and actual Foundation default/opt-in
readback. Sidecar/WAL/corruption physical-fault coverage beyond sidecars present
in this host case belongs to the parent's adapter tests; do not infer a WAL file
existed from a directory scan. Backup setter failure/readback mismatch includes
a private injected setter failure; positive cases use real Foundation.

One fresh protected-owner constructor sample measured 25,580,020 ns, including
input/path checks, Foundation policy, key encoding, C ABI and engine/catalog open.
Existing protected/unprotected H01 handler-to-drain samples were 13,214,228 ns and
13,696,483 ns. These are one-run debug fixture observations on a shared host, without
new budgets, percentile claims or isolated engine benchmarks.

## Build and provenance

See README security for current commands and SECURITY_EVIDENCE.json for exact
source/archive/runner hashes, logs and sample provenance. The complete H/E suite
source and runner are retained in ignored target before the later three-line
S10 timing instrumentation; that exact instrumentation diff is recorded, and
the three protected security cases reran successfully after warnings-as-errors rebuild with
explicit framework flags. The native cipher archive remained identical.

All builds use warnings-as-errors. Modified commands.swift passes strict
swift-format and diff whitespace checks. The four earlier Swift fixture files
are unchanged and compile/run, but their existing strict format findings are
recorded honestly; no whole-Swift-tree lint pass is claimed.

The current CommonCrypto static archive needs Apple Security/CoreFoundation
frameworks for plain Swift/C clients. The historical no-framework link command
failed as expected; adding explicit OS-framework flags passed all earlier Swift
builds and is now documented. Checked-in shell/CI link scripts were searched;
none call the plain native linker, and Rust jobs use Cargo's framework metadata.

Only task-created synthetic files were used. Successful runs clean their own
root; exact failed roots were removed after completion, with inventories/hashes
recorded under target. The shared local HTTP fixture was stopped after the
browser agent confirmed its tests completed.
Foreign/runtime copies cannot be guaranteed erased, an OS hint cannot stop every
external backup, and no new mobile/hostile-runtime/FIPS/platform SDK claim is made.
