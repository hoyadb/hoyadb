# Phase 6 dependency approval packet (metadata only)

Resolved with observed Rust/cargo1.98.1. Project manifests and lockfiles are unchanged. No build script, compiler build, encryption runtime or downloaded executable was run. Source fetches were for review only.

**39 external locked packages:7 new,13 changed,19 unchanged** relative to the union of root/native/browser existing lockfiles. All37 registry archive SHA256 values match the resolved lock; the other two packages are the exact Git commit below. Actual native graph has13 external packages; wasm32-unknown-unknown has35.

`rusqlite` + `libsqlite3-sys` Git source: `661107f6eb5d7477c61be1221744de3ad96c53e7`; Cargo checkout matches and GitHub signature reports verified/valid. Feature/source files are hashed in phase-six-dependencies.json. Git package labels are0.40.1/0.38.1; this is an explicitly approved source replacement for registry0.40.2/0.38.2, not an automatic patch update.

Proposed engines: native bundled-sqlcipher and browser sqlite-wasm-rs0.6.2[wasm-bindgen,sqlcipher], both exact sqlcipher-src419.0.6 (SQLCipher4.19.0 /SQLite3.53.4 /LibTomCrypt1.18.2). VFS0.3.0[sahpool] uses rsqlite-vfs0.2.0. Bridge/CLI family0.2.129, js/web0.3.106, futures0.4.79. CLI was NOT installed. zeroize1.9.1 uses its default alloc feature.

**Inactive lock-only Tokio:** tokio1.54.1 is retained by Cargo for wasm-bindgen-futures cfg(all(target_os="emscripten",wasm_bindgen_unstable_tokio)). It is absent from both target feature trees. No serde, Tokio or networking client is active in the reviewed native/wasm targets. This distinction must remain in approval text.

## New / changed packages

| Package | Exact version | License metadata |
| --- | --- | --- |
| cc | 1.10.0 | MIT OR Apache-2.0 |
| futures-core | 0.3.34 | MIT OR Apache-2.0 |
| futures-task | 0.3.34 | MIT OR Apache-2.0 |
| futures-util | 0.3.34 | MIT OR Apache-2.0 |
| js-sys | 0.3.106 | MIT OR Apache-2.0 |
| libsqlite3-sys | 0.38.1 | MIT |
| rsqlite-vfs | 0.2.0 | MIT |
| rusqlite | 0.40.1 | MIT |
| slab | 0.4.12 | MIT |
| smallvec | 1.16.4 | MIT OR Apache-2.0 |
| sqlcipher-src | 419.0.6+sqlcipher-4.19.0-sqlite-3.53.4-libtomcrypt-1.18.2 | MIT AND BSD-3-Clause AND blessing AND WTFPL |
| sqlite-wasm-rs | 0.6.2 | MIT |
| sqlite-wasm-vfs | 0.3.0 | MIT |
| wasm-bindgen | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-futures | 0.4.79 | MIT OR Apache-2.0 |
| wasm-bindgen-macro | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-macro-support | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-shared | 0.2.129 | MIT OR Apache-2.0 |
| web-sys | 0.3.106 | MIT OR Apache-2.0 |
| zeroize | 1.9.1 | Apache-2.0 OR MIT |

Exact complete names/versions/checksums/licenses/target activity and prior versions: [phase-six-dependencies.json](phase-six-dependencies.json). Frozen candidate [phase-six-dependencies.toml](phase-six-dependencies.toml) and [phase-six-dependencies.lock](phase-six-dependencies.lock). Native/WASM [feature](phase-six-features-native.txt) [trees](phase-six-features-wasm.txt) and exact [commands](phase-six-dependency-commands.txt) are retained. Primary sources are package manifests plus crates.io registry version metadata; cryptographic archive verification used the actual downloaded .crate bytes.

SQLCipher Community attribution plus all constituent license grants must be retained in user-accessible licensing materials. SQLCipher source package includes a documented WASM-only finalizer adjustment; this is upstream reviewed source, not a claim of byte-identical unmodified SQLCipher.

Source proposal remains gated on operator dependency approval and post-approval compile/runtime/security regressions. No compatibility/build pass is implied by metadata success. zeroize1.9.1 replaces1.9.0 after primary patch review; MSRV/features/dependencies are unchanged. The separate CLI install build graph is not part of this runtime lock.

Patch review: zeroize1.9.1 makes Zeroizing Debug opaque, fixes potential uninitialized memory read in optimization_barrier fallback and avoids double-zeroizing Vec elements. It has the same MSRV1.85/edition2024 and features/dependencies as1.9.0; all other lockfile package identities/checksums are unchanged. See official [changelog](https://github.com/RustCrypto/utils/blob/master/zeroize/CHANGELOG.md).
