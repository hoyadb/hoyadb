# Phase 6 independent review

Reviewer: Claude Opus 5, agent `e6f3b33e-7787-43bb-904f-646b096c9154`.
Scope: final protected-storage implementation; read-only review on 2026-10-24.
The report below distinguishes source review from runner-owned execution evidence.

## Final Phase 6 crypto review — PASS, no blocking issues

### Cipher configuration (`crates/hoyadb-storage-sqlite/src/encryption.rs`)

- **Key-first ordering holds.** `lib.rs:95-110` opens the connection, then `configure` issues `PRAGMA key` as the first SQL. `PRAGMA cipher_compatibility=4` follows the key, which is SQLCipher's documented order. The catalog read (`lib.rs:115`) is strictly after. No `pragma_pragma_list` helper is reachable.
- **No silent-plaintext path.** On a non-SQLCipher build `PRAGMA key` is an ignored no-op that returns `Ok`, but `read("PRAGMA cipher_version")` then returns no rows → `cipher capability unavailable`. I verified `cipher_status` is a real pragma in the vendored source (`sqlcipher-src-419.0.6/sqlcipher/sqlcipher.c:113678-113683`): it emits `"1"` only when the codec context exists with `error == SQLITE_OK`, so the eight-way profile check cannot be satisfied by an unkeyed or wrong-build connection. This directly answers API-0010's "never infer protection from a PRAGMA that could be silently ignored."
- **Key-SQL buffer is leak-free by construction.** `String::with_capacity(84)` against exactly 83 bytes emitted (16 prefix + 64 hex + 3 suffix) — no reallocation, so no un-zeroized copy is orphaned. `Zeroizing` on both the SQL and the key, with explicit `drop(sql); drop(key)` and wiping on every error return.
- **Injection impossible** — hex is generated from `[u8;32]`, never from caller text.
- **Wrong key fails at the catalog read, not at keying** (SQLCipher sets any key successfully). `lib.rs:115-121` is a `SELECT`, and the `CREATE TABLE` branch is gated behind `empty == true`, so a wrong key, plaintext file or foreign format returns an error before any write. Matches the byte-preservation cases in `full-first.txt`.

### Entry points (`lib.rs`)

`open_with_options` consumes `options.encryption_key` and returns before `path.as_ref()` is ever called — the `Uninspected` panic-on-deref fixtures in both `encryption.rs:83-91` and `tests/security_cases.rs` pin this. All inner errors collapse to the static `"protected storage open failed"`, so no path, engine text or SQL escapes a protected open. `open()` fail-closed, `open_unprotected` explicit and honestly labelled; `storage_protection()` reports `encrypted: false, cipher: "SQLite", version: "unprotected"` for it.

### Key ownership at the boundaries

- `EncryptionKey` — `pub(super)` field, no `Clone`, no serialization, no accessor, redacted `Debug` asserted literally.
- `command_fixture.rs:975-992` — strict 64-char lowercase-hex check and `Zeroizing` decode run before `fixture_path`, before the duplicate-owner check, and before any storage call. `Owner` stores only `protection` (a `Copy` struct of `bool` + two `&'static str`); no key, no hex. The `protection` RPC returns no path or key.
- `security_conformance.rs` — `KeyScope` is installed after the only fallible step, so the nested-control rejection path cannot leave a key set; every normal and error return clears it. Hex decode is bounds-safe given the prior validation.
- `commands.swift` — key length and location validated, then the `HOYADB_PHASE4_ROOT` identity check, then backup attributes, then the keyed RPC. No `print` or `HostFailure` carries args or key material.

### Graph

`dependency-activation-audit.json` reports `outside_approved_graph: []` across all three locks. Manifest pins are exact (`=` or git rev) for rusqlite `661107f6`, `sqlcipher-src 419.0.6+sqlcipher-4.19.0`, `sqlite-wasm-rs 0.6.2`, `wasm-bindgen 0.2.129`, `zeroize 1.9.1`; `sqlite-wasm-vfs 0.3.0` present in the browser lock. ADR-0034 is `accepted` and records the approval. Native profile run shows `commoncrypto`, and `s01_s10_secure_rpc` moved RED → GREEN.

### Residuals worth stating, not defects

`{byte:02x}` formatting uses a transient stack buffer that is not wiped, and the JSON/hex representations in the fixture, JS and Swift layers cannot be erased. API-0010 and THR-0019 already disclaim allocator, parser, binding and process copies — consistent, no change needed.

### Not verified by me

Root full-workspace run, fresh pinned Linux container, and the ongoing browser legacy flows. I read the recorded evidence (53/53 engine, 40/40 browser, Swift backup 6/security 3) but did not re-execute any of it; treat those as the runners' results, not as confirmed by this review. Phase 7 out of scope.
