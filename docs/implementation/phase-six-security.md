# Phase 6 security hardening — verification report

Phase 6 implementation and runtime verification are complete: supplied-key
native/browser storage, full protected and legacy matrices, the Rust regression
run and fresh pinned Linux gates pass. The final crypto review and the later
browser lifecycle review both returned **PASS**. The pending-open/close and
simultaneous-open follow-up is resolved with meaningful regressions and actual
worker disposal/reopen evidence. Final governance evidence is recorded at
closure; Phase 7 implementation has not started.

Reviewed preparation is `6f5ffb4`, based on Phase 5 closure `a4b35e7`. The operator
explicitly approved the frozen dependency graph; accepted ADR-0034 is committed
at `e318c4a`. [API-0010](../design/contracts/API-0010-secure-storage-and-diagnostics.md),
[SPEC-0032](../design/spec/SPEC-0032-security-hardening.md),
[TC-0035](../design/tests/cases/TC-0035-security-hardening.md) and
[THR-0019](../design/security/threats/THR-0019-protected-storage-and-diagnostic-boundary.md)
define acceptance. Earlier preparation/checkpoint evidence remains historical;
its pending-dependency and old-engine statements do not describe this snapshot.

## Implemented behavior

### Protected engines and key ownership

The same production SQLite adapter now uses SQLCipher 4.19.0 / SQLite 3.53.4:
AES-256-CBC with per-page HMAC-SHA512, the reviewed v4 profile, 4096-byte database
pages, zero plaintext header and memory-only temporary storage. The macOS
profile's actual provider probe returned `commoncrypto`, version
`61901.160.44.701.4`. The fresh Linux probe returned OpenSSL 3.5.7. WASM selects
upstream LibTomCrypt 1.18.2 and its WebCrypto entropy path through the reviewed
build; that provider attribution is source/feature evidence, not a separate
browser provider-PRAGMA result.

`EncryptionKey::from_bytes([u8;32])` and `SqliteOpenOptions` keep configuration
below the generic storage traits. The supplied-key constructor consumes an owned
zeroizing key and zeroizing key-SQL buffer. It sends only internally generated
hex digits, before any schema/catalog or pragma-list read. Direct fixed SQL
checks actual cipher/version/profile/status; unknown or ignored PRAGMAs cannot
establish protection. A subsequent catalog read authenticates an existing file
before initialization or DDL. `SqliteStorage::open(path)` fails for its absent
key before even evaluating the path. Explicit `open_unprotected` preserves the
historical synthetic controls, reporting unprotected status honestly.

Missing, malformed or wrong keys, plaintext/foreign files and unsupported crypto
reject without fallback, conversion, stamping or reset. Protected-open errors
are static and omit engine SQL, key and path. No application key generation,
passphrase interface, keychain, rotation or migration mechanism was introduced.
The core and memory adapter remain free of external runtime dependencies.

The [frozen dependency packet](phase-six-dependencies.md),
[complete graph](phase-six-dependencies.json) and
[ADR-0034](../design/decisions/ADR-0034-protected-engines-and-diagnostics.md) record
the exact approved pins: upstream rusqlite/libsqlite3-sys Git
`661107f6eb5d7477c61be1221744de3ad96c53e7`, sqlcipher-src 419.0.6,
sqlite-wasm-rs 0.6.2, SAH-pool VFS 0.3.0, wasm-bindgen/CLI 0.2.129,
js/web-sys 0.3.106, wasm-bindgen-futures 0.4.79 and zeroize 1.9.1. The Git
packages retain labels 0.40.1/0.38.1; this is an approved source replacement,
not a floating branch or a registry patch-version claim. The activation audit
found no package outside the approved graph in any of the three application
locks. Lock-only Emscripten Tokio remains inactive on the native and browser
verification targets. The original packet is intentionally not rewritten as an
activation/runtime record.

### Backup, diagnostics and foreign boundaries

The private Swift reference host validates the exact key length, store name and
pre-established canonical owned root before applying Foundation's backup flag.
It writes and reads back directory exclusion, default true, with explicit false
opt-in. Failed validation/policy prevents opening. Replacement revalidation is
closed-only, with ownership and policy checked again. A compiling regression
proved and fixed an ordering bug where a second valid-looking temp root could
receive attributes before the RPC rejected its unrelated path. The protected
constructor now checks exact root identity first. Application Support remains
deployment guidance; no arbitrary/default path resolver was added to this fixture.

Browser OPFS reports backup exclusion as `unsupported`. Origin isolation and
`persist()` do not substitute for an unavailable OS backup control. Protected
Swift/browser constructors require caller keys and use `open_secure`; explicit
unprotected constructors remain available for legacy controls. Neither silently
falls back. C/WASM frame bounds, checked handles and existing scalar encoding
remain unchanged.

`CommandDatabase::diagnostics()` reads only already-owned state and returns ten
numeric/Boolean fields: schema_version, pending, sending, awaiting_outcome, live,
blocked, ready, pages, changes and tombstones. Automatic Debug and foreign errors
omit payloads, keys, identities, results/reasons, names/IDs, paths, namespace,
scope and checkpoint. Explicit data access and complete durable retry bytes
remain unchanged. JavaScript captured context recursively rejects mutation,
including non-strict assignments and nested/descriptor operations through await.
Rust/Swift keep their owned value captures. No telemetry, automatic export or
library network path was added. See [diagnostics/context evidence](phase-six-diagnostics.md).

## Verification evidence

Counts below describe executed tests or report groups, not interchangeable
numbers of individual assertions. Failed intermediate logs remain alongside the
successful reruns; compilation/setup failures are not counted as behavioral RED.

| Gate | Observed result and evidence |
| --- | --- |
| Full host workspace | **347 passed, 0 failed**, 31 Cargo result records, 25 nonempty test binaries; includes 80 core and 53 encrypted-engine tests. `target/phase-six-security/workspace-encrypted.txt` |
| Native encrypted engine | **53 named tests passed**; direct security/process cases plus 60 unchanged generic shared functions. [Engine report](phase-six-security-engine.md) |
| Actual protected Swift | Backup **6/6**, secure foreign owner **3/3** plus independent reader; unchanged protected command **6/6** and entity **4/4** groups. [Native report](../../spikes/cross-platform/native/SECURITY_RED.md), [hashes/output map](../../spikes/cross-platform/native/SECURITY_EVIDENCE.json) |
| Explicit legacy Swift | Same command **6/6** and entity **4/4** groups; earlier main **5**, experiments **3**, reactive **4**, persistence writer/reader both pass on the cipher archive. Native evidence above |
| Actual protected OPFS | **40/40 report groups**, including 35 shared groups executing 79 named assertions and five host groups. [Browser report](phase-six-browser.md), [results](phase-six-browser-results.json) |
| Explicit legacy browser | Persistence **36 before + 12 after actual reload**, reactive **17**, command **13**, entity **11** report groups pass on the same generated artifacts. [Results](phase-six-browser-legacy-results.json) |
| JS boundary tests before lifecycle follow-up | **33 Node cases** pass; syntax checks for all 17 JS/MJS files and shell gate pass. These stub-export router checks are not cipher proof. [Browser output](phase-six-browser-test-output.txt) |
| Fresh pinned Linux | **347 workspace + 11 host-spike + 28 native-spike** tests pass, plus one repeated provider/profile unit: **387 invocations, 386 unique tests**, 0 failures. [Durable output](phase-six-ci-output.txt) ends `PHASE6_CONTAINER_GREEN`; process exited 0 |
| Linux build/lint matrix | All five Cargo format manifests, layout regression, warnings-denied Clippy, both memory examples and all scripted library/spike WASM builds pass. [Exact commands](phase-six-ci-commands.sh) |
| Behavior-only clean candidate | All five default-format checks plus workspace/native all-target compile checks pass; source-hash report matches and index unchanged. `target/phase-six-clean-check-result.json` |
| Final crypto review | Claude Opus 5 **PASS, no blocking issues**. [Exact final response](phase-six-review.md); reviewer distinguishes source inspection from runner-owned runtime results |
| Browser lifecycle follow-up | Five permanent regressions observed **5 RED → 5 GREEN**; all **38 Node tests** and 18 JS/MJS syntax checks pass. Actual worker **4/4**, protected host **5/5**, command **13/13**, entity **11/11** groups pass, including real reloads and cleanup. [Report](phase-six-lifecycle.md), [runtime results](phase-six-lifecycle-results.json) |
| Independent lifecycle review | Claude Opus 5 **PASS**, independently reran all **38 Node tests**. [Exact response](phase-six-lifecycle-review.md); actual browser run belongs to the separate runtime evidence |

The workspace count was recomputed from the 31 recorded Cargo summaries, not
estimated from a prior phase or by adding generic shared-function counts. The
53-test encrypted binary contains grouped callers and one child-driver entry;
its 60 shared functions are not another 60 tests to add to the workspace total.
The container's extra profile invocation repeats a unit already included in its
workspace run. Linux used Rust 1.98.1, Debian LLVM/Clang 19.1.7 and system
OpenSSL 3.5.7, with the image/tool commands recorded by its run.

Modified `commands.swift` passes strict swift-format and all Swift builds use
warnings-as-errors. The four earlier Swift fixtures are unchanged and compile/
run, but retain their existing strict-format findings. No whole-Swift-tree lint
pass is claimed. Plain Swift/C consumers of the CommonCrypto static archive
need explicit Security/CoreFoundation framework linkage; the historical link
failure and corrected successful commands are documented in the
[native README](../../spikes/cross-platform/native/README.md).

### What the protected cases demonstrate

Native tests inspect exact main/journal/WAL bytes and preserve every owned file
on wrong-key/format errors. A separate unkeyed connection cannot read schema.
Authenticated first-page corruption fails without repair or partial publication.
Real journal precommit and WAL postcommit writers reach named barriers before
termination; correct-key reopen produces the specified old/new state. Further
command/entity pre/postcommit processes preserve rows, original namespace,
counters, acknowledged work, checkpoint and byte-identical frozen retries.
A 33,006-operand TEMP query preserves the owned file inventory, and a private
connection probe separately verifies `temp_store=2`.

Swift additionally verifies actual Foundation policy/readback, the exact
controlled-root boundary, protected key/format refusals, closed encrypted-file
replacement and independent-process reopen. Its raw scan covers the main file
and any sidecars present, not an assertion that its simple host case created a
WAL. The physical journal/WAL assertions belong to the native engine cases.

The actual OPFS worker verifies physical pool bytes, including its 4096-byte
VFS metadata header, for domain/key/header canaries. Tampering occurs after that
header in an authenticated SQLite page. Wrong-key, unkeyed and protected legacy
opens preserve complete physical hashes. A terminated worker and subsequent
actual page reload receive the supplied key again and recover exact committed
rows, checkpoint and frozen bytes. This is postcommit worker/process recovery,
not browser precommit interruption or power-loss proof. The browser context
mutation, actual outgoing identity and diagnostic allowlist checks run through
the real worker. The local security pull response is caller-provided; existing
command/entity scenarios exercise the real caller-owned HTTP transport.

### Performance and provenance

One fresh protected Swift constructor sample was **25,580,020 ns**, including
path/key validation, Foundation policy, key encoding, C ABI and engine/catalog
open. Existing protected/unprotected H01 handler-to-drain samples were
13,214,228 ns and 13,696,483 ns. Protected/unprotected complete H/E runs took
45.07 s and 44.57 s. These are single debug-fixture observations on a shared
host, with no new SLO, percentile or isolated-engine performance claim. The
optimized browser's unchanged capacity-500 case passed in 47,513 ms; capacity
and assertions were not reduced.

Native [evidence](../../spikes/cross-platform/native/SECURITY_EVIDENCE.json)
records the full-suite source/runner and the later three-line S10 timing-only
change. The protected security cases reran after that instrumentation. Both
use archive SHA256
`cfdf3bf05d9b78224529ad2831eb9a3bca8ed0e4ac1de1d817f81ce9929b5e74`.
Browser [source/artifact hashes](phase-six-browser-source-sha256.json) record
123 source and two generated files, rechecked after runtime flows. Local detailed
RED/GREEN commands, failed attempts, measurements and cleanup records remain
under `target/phase-six-*`. The
[historical checkpoint output](phase-six-checkpoint-output.txt) and
[checkpoint evidence](phase-six-checkpoint-evidence.json) are preserved as their
earlier pre-encryption snapshot, not overwritten with later results.

### Browser lifecycle correction and final source cuts

Contract clarification `c3dfcf8` preceded the permanent regressions and minimal
JS-only fix. One synchronous private reservation rejects a second open. Close
cancels publication, waits for the original reply and disposes any resulting
owner exactly once; concurrent closes share cleanup. A failed disposal retains
its handle for close retry, without resending a key-bearing open request.
The attempt holds no key/configuration. The [lifecycle report](phase-six-lifecycle.md)
retains meaningful RED/GREEN, real suspended-worker-reply tests and affected
protected/command/entity runtime reruns. The separate [126-source/two-generated
manifest](phase-six-lifecycle-source-sha256.json) matches this later execution;
its generated WASM/bindings are identical to the original crypto run. The
original 40/40 matrix, 123-source manifest and crypto review remain their honest
earlier source cuts. Unchanged Rust/shared assertions were not rerun solely for
the JS lifecycle fix. All four fresh namespaces, task-owned tabs and the owned
HTTP fixture were cleaned, with [cleanup evidence](phase-six-lifecycle-cleanup.json).

The [durable verification record](phase-six-verification.json) preserves the
workspace, fresh Linux and clean-candidate results. The [activation audit](phase-six-dependency-activation-audit.json)
records the approved graph check. Root closure records final traceability and
RTM checks after the completed documentation set; those are governance checks,
not additional runtime cases. Final checks report **0 trace violations** and
an up-to-date RTM. REQ-0045, SPEC-0032 and API-0010 are accepted, TC-0035 active
and THR-0019 mitigated within this documented scope. Copied log presentation
normalizes line endings/trailing whitespace; the verification record retains
both raw and normalized digests and original logs remain under `target/`.

## Limits

An indefinitely suspended caller transport can keep close pending. A permanently
lost worker during failed disposal keeps that client Busy as the reviewed
fail-closed behavior; the wrapper does not promise a timeout or rollback of
filesystem effects.

Protection covers local engine files outside the running application. It does
not hide sizes/access patterns, validate backend authorization or protect an
already compromised process. Key entropy, storage/lifecycle and compatible-reader
availability remain the application’s responsibility. Vetted zeroization covers
owned Rust buffers; parser, allocator, stack and foreign-runtime copies cannot
be guaranteed erased. No FIPS, rekey, plaintext migration/restore, universal
backup suppression or mobile support is claimed. Explicit application copies
and logs of data returned by ordinary APIs remain application-owned.

All destructive corruption/interruption work used owned synthetic stores.
Successful hosts cleaned their namespaces/roots; failed native roots were
removed only after their processes exited. The shared loopback server was stopped
after both host lanes confirmed completion. No user stores were migrated or
reset, no push/merge/publication occurred, and pre-existing user formatting/CI
work remains preserved. The user authorized Phase 7 after Phase 6; read-only
planning does not count as implementation or packaged/mobile support.
