---
id: ADR-0034
title: Supplied-key SQLCipher storage and bounded security diagnostics
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0030, REQ-0031, REQ-0032, REQ-0045]
depends_on: [RFD-0005, SPEC-0032, API-0010, THR-0019]
tags: [security, encryption, security]
---

# ADR-0034: Supplied-key SQLCipher storage and bounded diagnostics

## Context and considered options

The operator requested security with recommended policies, then explicitly
approved the exact dependency proposal in preparation commit 6f5ffb4, answering
"Approve exact reviewed dependencies" on 2026-10-24. The older bundled cipher
sources predate upstream security fixes. A private VFS cipher, silent plaintext
fallback or native-only protection would not establish the accepted floor.

## Decision

Adopt RFD-0005 P1–P6 and API-0010: supplied 32-byte raw keys, engine-owned
SQLCipher 4.19 authenticated page encryption, secure default opens, explicit
unprotected historical controls, fail-closed key/format checks, native owned-
directory backup hints, truthful browser Unsupported capability, bounded
allowlisted diagnostics and immutable foreign context. Generic core/storage
traits remain key-free; core and memory keep no runtime dependencies.

The active source graph is pinned by platform Cargo locks and the native notice
inventory. It includes the exact rusqlite/libsqlite3-sys Git revision, SQLCipher,
WASM VFS and bridge toolchain selected by those locks. Inactive lock entries are
not claims of target activation. Dependencies and provider changes require their
own source, license and artifact review.

## Consequences and confirmation

No key generation, storage, passphrase KDF, rekey, plaintext conversion, user
migration, new platform, network/telemetry, publication or platform SDK is authorized.
Caller keys must remain available for compatible readers after a source revert.
OS backup values are hints; foreign/process copies cannot be guaranteed erased.
No FIPS or compromised-process protection is claimed. Explicit application data
access remains distinct from automatic owner diagnostics.

TC-0035 requires compiling meaningful RED/GREEN, actual native and worker engines,
wrong-key/format nonmutation, authenticated corruption, main/sidecar inspection,
protected functional parity and process/reload recovery, Foundation backup checks,
all regressions/build/lint/trace/container gates and final bounded Opus review.
Approval permits implementation; it does not assert any cipher gate already passed.
