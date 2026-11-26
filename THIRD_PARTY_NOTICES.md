# Third-party encryption notices

This product uses SQLCipher Community Edition, Copyright (c) 2025 ZETETIC LLC.
The approved source is sqlcipher-src419.0.6 (SQLCipher4.19.0 / SQLite3.53.4 /
LibTomCrypt1.18.2), through the exact upstream pins recorded in
[the dependency packet](docs/implementation/phase-six-dependencies.md).

License text whitespace is normalized for this repository; the original grants are retained in [licenses/sqlcipher](licenses/sqlcipher/).
The upstream Rust source package is MIT, SQLCipher is BSD-3-Clause, SQLite is
public domain with its blessing, and LibTomCrypt provides its original dual
public-domain/WTFPL license text. Neither upstream names nor contributors imply
endorsement. The source package documents an upstream WASM finalizer adjustment;
hoyadb does not supply a private cryptographic implementation.

Native macOS uses the system CommonCrypto provider. Linux verification links
system OpenSSL/libcrypto. WASM includes the upstream LibTomCrypt provider and
WebCrypto entropy shim. These are platform-specific provider choices, not a
FIPS certification. Applications distributing platform libraries must preserve
their applicable license materials as well.

Complete package versions, sources, license metadata and archive checksums are
in [phase-six-dependencies.json](docs/implementation/phase-six-dependencies.json).
The matching task-local wasm-bindgen CLI is a build tool, not a runtime library
bundled in the database. Consumer packaging and complete distribution notices
are finalized in Phase 7; no package is published by this phase.
