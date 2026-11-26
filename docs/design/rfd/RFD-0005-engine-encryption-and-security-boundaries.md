---
id: RFD-0005
title: Current engine encryption supplied keys and bounded security diagnostics
type: rfd
status: discussion
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [ARC-0001, ARC-0002, REQ-0030, REQ-0031, REQ-0032]
tags: [phase-six, security, encryption]
---

# RFD-0005: Current engine encryption supplied keys and bounded security diagnostics

The operator requested the full next roadmap phase with recommended policies,
limited Claude Opus feedback and at least a final Opus review. This authorizes
recommended architecture/behavior choices within Phase 6. New cryptographic
runtime dependency versions still need the separate explicit CHG-4 approval
listed below; no dependency is activated by this proposal.

## Verified starting point and need

Phase 5 closes at a4b35e7. The core and memory adapter have no external runtime
dependencies. SqliteStorage::open initializes plaintext native/OPFS SQLite;
current hosts expose no key or backup policy. Existing pinned SQLCipher 4.14.0
and SQLite3MultipleCiphers 2.2.6 sources predate upstream security fixes.
Native and browser research recommends a current SQLCipher source shared by
both targets, avoiding a private cipher implementation or stale security code.

## Recommended policies

### P1 — established authenticated engine encryption

Use SQLCipher 4.19.0 / SQLite 3.53.4, AES-256-CBC with per-page HMAC-SHA512,
4096-byte pages and the v4 profile. Enable the maintained upstream SQLCipher
backends on native and browser; never implement page crypto, a VFS cipher,
a key derivation scheme or an encryption substitute in hoyadb. Native uses
CommonCrypto on macOS and the maintained system libcrypto in the pinned Linux
verification image. WASM uses the upstream LibTomCrypt provider and its WebCrypto
entropy shim. No weakened authentication/header/RNG fallback is accepted.
Verify runtime cipher/profile, memory-only temp storage and encrypted journals.

### P2 — supplied raw keys and secure defaults

Require an application-supplied exact 32-byte raw key for protected opens. No
passphrase/KDF/keychain/rotation/key generation is owned by hoyadb. Missing or
malformed keys fail before touching a file. Wrong keys, plaintext/foreign files,
unknown formats and unsupported crypto fail closed without initialization,
plaintext fallback, migration or reset. Authenticate before any catalog/DDL.
Existing explicit unprotected regression fixtures remain available under a
clearly named unprotected entry, never claimed to meet the security floor.
The default public adapter open has no implicit plaintext path. Read-only
configuration/Debug/errors never include key bytes. Zeroize owned temporary key
buffers using a vetted library; do not claim full process/foreign-copy erasure,
protection against a compromised running app or FIPS certification.

### P3 — bounded compatibility and recovery

Encryption is below the generic storage traits. Preserve query/index/schema,
command/entity recovery, metadata/checkpoint atomicity and reset semantics.
No automatic encryption conversion, rekey or backup restore API is introduced.
An existing plaintext store requires a separately designed explicit migration;
this phase operates only on owned synthetic verification stores. Reverting
source requires a compatible encrypted reader and the caller's original key.
Do not strip metadata or mutate user stores to satisfy an older implementation.

### P4 — honest platform backup controls

The Swift desktop reference host owns a store directory and sets/verifies
Foundation's backup-exclusion resource value before protected store creation,
default true, explicit opt-in false. Reapply/verify after its own replacement
operations; include sidecars via directory policy. Failure prevents protected
open/data creation. This is an OS backup hint, not a promise that no external
backup/copy can occur. Browser OPFS/Storage API has no backup-exclusion control:
report Unsupported explicitly. Origin isolation or persist() is not exclusion.
No library-owned export or automatic selective purge is added.

### P5 — allowlisted diagnostics and conservative formatting

Provide a bounded owned diagnostics snapshot containing only schema version,
queue counters and redacted sync progress/capabilities. Exclude domain payloads,
identity, reasons/results, keys, table/record names/IDs, namespaces, paths,
scopes and checkpoints. No automatic export, logging, telemetry or network.
Redact Debug on context/recovery/transport/event owner boundaries and sanitize
engine/foreign errors. Keep complete retry state locally, unchanged. Explicit
row/value/JSON access remains application-owned; no claim can prevent an app
from deliberately logging data it reads. Local caller-owned transport remains
the sole network seam.

### P6 — immutable foreign context and verification

Foreign handler context is an immutable owned snapshot through awaits. Reject
JS reassignment including nested identity changes; Rust/Swift retain their
existing immutable views. No local auth or membership validation is added.
Test supplied-key preservation, main/sidecar canaries, wrong-key nonmutation,
page authentication failure, encrypted behavioral parity and process/reload
recovery, platform backup attributes, diagnostic absence and actual suspended
hosts. Run prior regressions and a final bounded Claude Opus review for real
reachable issues, without a finding quota. No Phase 7 implementation or release.

## Proposed exact dependency approval

Subject to review and explicit approval:

- rusqlite and libsqlite3-sys from upstream Git revision
  661107f6eb5d7477c61be1221744de3ad96c53e7, native bundled-sqlcipher;
- sqlcipher-src =419.0.6 (SQLCipher 4.19.0 / SQLite 3.53.4 / LibTomCrypt 1.18.2);
- sqlite-wasm-rs =0.6.2 with wasm-bindgen/sqlcipher;
- sqlite-wasm-vfs =0.3.0 with sahpool;
- wasm-bindgen =0.2.129, js-sys/web-sys =0.3.106,
  wasm-bindgen-futures =0.4.79 and matching task-local wasm-bindgen CLI;
- zeroize =1.9.1 inside the SQLite adapter, not core/memory;
- reviewed locked transitive graph, to be recorded before activation.

The Git packages label themselves 0.40.1/0.38.1 despite their newer source.
This is an unreleased upstream integration pin with substantial changes,
not a two-line feature switch. Pin its full commit and checksums, review actual
used APIs and licensing, then execute complete compatibility gates. Never use
a floating branch, silent engine downgrade or a private cipher fork.

## Alternatives and sources

Reject the older bundled ciphers after upstream security fixes; a smaller diff
cannot establish the accepted security floor. SQLite SEE needs a commercial
source/distribution license; wa-sqlite alone supplies no encryption. A custom
WebCrypto VFS would add cryptographic design/atomicity risk and violates the
engine-owned boundary. Native-only encryption would leave the current browser
claim unfinished. Native vendored OpenSSL adds build/runtime dependencies that
are unnecessary with the existing platform providers.

Primary evidence checked 2026-10-24:
[SQLCipher design](https://www.zetetic.net/sqlcipher/design/),
[upstream integration pin](https://github.com/rusqlite/rusqlite/tree/661107f6eb5d7477c61be1221744de3ad96c53e7),
[WASM backend](https://docs.rs/crate/sqlite-wasm-rs/0.6.2),
[SQLite3MC fixes](https://github.com/utelle/SQLite3MultipleCiphers/blob/main/CHANGELOG.md),
[Apple backup resource](https://developer.apple.com/documentation/foundation/urlresourcekey/isexcludedfrombackupkey),
[File System Standard](https://fs.spec.whatwg.org/),
[Storage Standard](https://storage.spec.whatwg.org/).
