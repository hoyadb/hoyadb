# Android SQLCipher source-reuse candidate — metadata review only

Checked2026-10-24. **Feasible build candidate, not compiled, linked, installed or
runtime-verified by this task.** No source/manifests/global environment changed.
Phase 6 native/WASM acceptance does not establish this new Android provider path.

## Conclusion

Reuse the already-approved SQLCipher419.0.6 / SQLCipher4.19.0 / SQLite3.53.4 /
LibTomCrypt1.18.2 source package to build a target-specific static
`libsqlcipher.a`, then use libsqlite3-sys's existing externally-linked-library
route. This can avoid a rusqlite fork, new Rust runtime dependency and new
OpenSSL library graph. It still introduces reviewed native build configuration
and an OS-entropy adapter; approve/commit exact contract/cases before TDD.

The significant caveats are fail-closed entropy, a single shared LTC configuration
with real threading, explicit Android ABI/minSDK/16KiB linking, and proving the
correct static archive was selected instead of a host/system library.

## Verified available source and toolchain facts

All source below is already in the local approved Cargo cache; no fetch/install
was done. The source package's Rust API exports:

- `SOURCE_FILE = "sqlcipher.c"`, `HEADER_FILE = "sqlite3.h"`;
- `LIBTOMCRYPT_INCLUDE_DIR = "libtomcrypt/headers"`;
- `LIBTOMCRYPT_SOURCES`, an explicit list of individual crypto C translation units;
- `source_dir()` (source package src/lib.rs:14-35).

Local base is
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sqlcipher-src-419.0.6+sqlcipher-4.19.0-sqlite-3.53.4-libtomcrypt-1.18.2/`.
Use Cargo metadata/a checked source manifest to resolve this path in the eventual
build, not a permanently hardcoded developer home/cache location.

The installed NDK27.1.12297006 sysroot has public Android libc/pthread headers and
clang triples for x86_64 and arm64 API24. No public libcrypto was found by the
independent inventory. Installed Rust mobile targets are absent. No compiler
invocation was performed here. NDK27.1 matches RN0.86.3's catalog but requires
explicit16KiB final-link flags; a reviewed r28+ choice is an alternative, not a
silent toolchain upgrade.

## Existing no-fork link route: exact checked behavior

Approved rusqlite revision:
`661107f6eb5d7477c61be1221744de3ad96c53e7`.

1. `libsqlite3-sys/build.rs:80-86`: nonzero
   `LIBSQLITE3_SYS_USE_PKG_CONFIG` chooses `build_linked` **before** the bundled
   branch, even with the current bundled-sqlcipher feature.
2. `env_prefix()/lib_name():376-389`: sqlcipher or bundled-sqlcipher selects
   `SQLCIPHER_*` variables and library name `sqlcipher`.
3. `build_linked:429-450`: current bundled features reuse their checked SQLCipher
   bindings unless buildtime_bindgen is activated. No bindgen dependency needed.
4. `find_sqlite:471-505`: `SQLCIPHER_LIB_DIR` points at the external archive directory;
   `SQLCIPHER_INCLUDE_DIR` is the matching header location;
   `SQLCIPHER_STATIC=1` selects static fallback. It first probes
   `<libdir>/pkgconfig/sqlcipher.pc`, otherwise emits static link/search directly.
5. Existing pkg-config0.3.34 `src/lib.rs:774-790` also recognizes
   `SQLCIPHER_STATIC` for a successful `.pc` probe. Declare only the target-local
   package; do not let a host SQLCipher package satisfy this cross-build.

Candidate eventual build invocation (illustrative until real facade manifest
exists, so no fabricated cargo package command is supplied):

```text
env LIBSQLITE3_SYS_USE_PKG_CONFIG=1 \
    SQLCIPHER_LIB_DIR=<task-owned-target-abi-stage>/lib \
    SQLCIPHER_INCLUDE_DIR=<matching-stage>/include \
    SQLCIPHER_STATIC=1 \
    PKG_CONFIG_ALLOW_CROSS=1 \
    PKG_CONFIG_LIBDIR=<matching-stage>/lib/pkgconfig \
    CARGO_TARGET_<RUST_TRIPLE_UPPER_UNDERSCORE>_LINKER=<matching NDK clang wrapper> \
    <reviewed Cargo build command for exactly one Android target>
```

Keep these command-local, with a separate Cargo target output directory per
ABI/profile/provider fingerprint. Do not export globally or store developer
absolute paths in consumer packages. With a deliberately generated target-local
`.pc` file, report the truthful SQLCipher package version4.19.0, include
`-L<stage>/lib -lsqlcipher` and the verified required Android system libs. The
dependency's numeric `>=3.45.3` check does not establish the embedded SQLite API
version for a differently-versioned SQLCipher package; independently verify
SQLite3.53.4 at build/runtime and never rewrite metadata just to satisfy a check.
Do not assume the dependency's forced pkg-config path alone excludes all host
defaults: set a target-only `PKG_CONFIG_LIBDIR`, inspect actual `-vv` build output
and final ELF dependencies. This is normal upstream linking configuration, not
changing a tool/check to conceal failure.

## Candidate engine archive build

Use the NDK clang directly with target e.g. `--target=x86_64-linux-android24` or
`--target=aarch64-linux-android24`, `-O2 -fPIC`, and its `llvm-ar` to archive:

1. SQLCipher amalgamation object, matching approved native feature flags.
2. Every explicit `LIBTOMCRYPT_SOURCES` object **except** upstream
   `prngs/rng_get_bytes.c` if the recommended fail-closed replacement below is
   selected. Validate expected source count/list/hash rather than recursively
   compiling every C file, which could duplicate implementation tables/tests.
3. One small Android OS-entropy adapter providing that excluded function's exact
   public ABI; no AES/HMAC/KDF algorithm changes.

One combined archive avoids ordering/dependency ambiguity among separate static
SQLCipher/LTC archives. Matching header and archive/source hashes travel into the
package evidence. Final JNI `.so` build links it once alongside Rust facade;
avoid shipping a second independently initialized engine/core in the same app.

SQLCipher candidate flags:

```text
SQLITE_HAS_CODEC=1
SQLCIPHER_CRYPTO_LIBTOMCRYPT=1
SQLITE_EXTRA_INIT=sqlcipher_extra_init
SQLITE_EXTRA_SHUTDOWN=sqlcipher_extra_shutdown
SQLITE_THREADSAFE=1
SQLITE_TEMP_STORE=3
SQLITE_DEFAULT_FOREIGN_KEYS=1
SQLITE_ENABLE_API_ARMOR
SQLCIPHER_OMIT_LOG=1
SQLCIPHER_OMIT_LOG_DEVICE=1
SQLCIPHER_OMIT_DEFAULT_LOGGING=1
```

Preserve the current native build's other enabled SQLite features (column
metadata/dbstat/FTS/JSON/rtree/stat4/etc.) explicitly if the public adapter or
existing gates depend on them; compare `PRAGMA compile_options` against the
accepted allowlisted profile. These flags are a candidate minimum, not a claim
that feature parity has been compiled. Android bundled native currently sets
SQLITE_TEMP_STORE=3 already (libsqlite3-sys/build.rs:275-279).

All SQLCipher and LTC units must share one `-include <reviewed-ltc-config.h>`:
the approved minimal AES/CBC/SHA1/SHA256/SHA512/HMAC/Fortuna/PKCS5/hash helpers,
NO_MATH/NO_ASM/NO_TEST/CLEAN_STACK and ARGTYPE4 configuration, plus
**LTC_PTHREAD**, with LTC_SOURCE for library implementation units. Exact layout
matters: `prng_state` contains a mutex when LTC_PTHREAD is enabled. Never compile
the amalgamation and LTC against different macros. SQLCipher already uses its
own SQLite mutexes for provider initialization/random access; keep real native
SQLite mutexes and LTC registry/PRNG locks. Do not copy wasm's
SQLITE_MUTEX_NOOP, fake libc/stdio/atexit/rename definitions or OMIT_MEMLOCK.

## Entropy: the concrete trap and recommended native adapter

The approved upstream `libtomcrypt/prngs/rng_get_bytes.c` does this:
hook if nonzero → OS read if nonzero → ANSI clock-jitter generator. A hook that
returns0 on failure **does not fail closed**. LTC_NO_FILE alone makes the OS read
return0 and still permits clock fallback. The approved wasm shim explicitly
aborts if its WebCrypto entropy hook fails; transplanting it would bring wasm
assumptions/abort semantics into the native library.

Recommended native option: exclude only that upstream entropy-dispatch unit and
provide the same `rng_get_bytes(out,len,callback)` symbol using **public Android
OS cryptographic entropy only**, returning exactly len on success or0 on error,
zeroing partial output. SQLCipher's actual LTC provider checks exact32 bytes and
returns SQLITE_ERROR on short/error (sqlcipher.c:115120-115128), so no fallback
or invented salts/keys is necessary. This is a platform entropy adapter, not a
custom cryptographic primitive. It needs security review and fault tests.

For minSDK24 compatibility, a small checked `/dev/urandom` open/read loop with
O_CLOEXEC, EINTR/short-read handling and closed descriptor avoids referencing API28
symbols. If minSDK28 is explicitly adopted, `getrandom(...,0)` with checked loops
is a simpler public libc option. Installed NDK headers mark getrandom/getentropy
as API28, so do not silently link those functions into a claimed API24 artifact.
`arc4random_buf` is documented by the NDK as available on all API levels, but it
returns void; any process-abort behavior on provider failure must be understood
before choosing it over a recoverable checked adapter.

Do not use rand/time/clock/SQLite PRNG as entropy, Android private BoringSSL,
fallback plaintext or a constant seed. Do not log entropy, keys or domain data.
Do not implement separate RNG state in wrappers. On provider failure admit no
usable owner or committed ciphertext; test that an existing-file open attempt
preserves prior bytes. If failure leaves a newly created empty artifact, specify
and test its cleanup policy rather than claiming pre-creation failure without
checking the adapter's actual SQLite open/key ordering.

## Meaningful acceptance before claiming support

1. Contract/case prep committed first. A compiling placeholder provider or
   deliberately unavailable archive gives a genuine supplied-key open RED;
   compiler/header failures are setup failures, not behavioral RED.
2. Build x86_64 emulator and arm64-v8a archives from the exact approved source
   hashes; inspect ELF machine, exported/imported symbols and final NEEDED. No
   `libcrypto.so`, host Mach-O/object or stock SQLite may be selected by accident.
3. Runtime provider reports SQLCipher4.19.0 + `libtomcrypt`/1.18.2, AES256CBC,
   HMAC-SHA512,4096-byte DB pages, plaintext_header_size0, temp_store memory and
   thread safety. Missing/wrong keys fail closed; correct key survives process
   restart; no automatic migration/plaintext fallback.
4. Reuse unchanged typed/model/reactive/command/entity/pull/reset cases with the
   protected factory, then actual Kotlin/JNI and RN callers. Check custom caller
   schema, awaited handler re-entry and callbacks without native locks held.
5. Inject OS entropy failure/short reads/EINTR: exact entropy count or a static
   error, no clock fallback/hang, zeroed partial output, no mutation of existing
   store. Test the adapter's real error path, not only hook stubs returning0 into
   the unmodified weak dispatcher.
6. Multiple encrypted owner lifetimes on concurrent managed threads exercise
   provider init/deinit/random locks; no data races, ABI layout mismatch or stale
   JNI references. Match one library/core instance per application process.
7. Existing native security cases: wrong/plaintext/foreign/corrupt files preserve
   bytes, actual sidecars contain no synthetic canaries, real pre/postcommit
   killed-process recovery, secure diagnostic/error boundaries and key lifetime.
8. Cross-provider fixture: read a task-owned CommonCrypto-generated encrypted DB
   under Android LTC, write/reopen through the other provider with the same
   synthetic key/profile and exact typed data/recovery state. Never user stores.
9. Final shared object/APK/AAR has16KiB ELF/ZIP alignment and runs on a task-owned
   ps16k emulator if claiming that runtime. SQLCipher4096 DB pages stay unchanged.

## Source/provenance checks

- Local approved rusqlite build.rs and pkg-config0.3.34 source lines cited above.
- Local approved source-package `src/lib.rs`, `sqlcipher.c:115037-115170`,
  `libtomcrypt/prngs/rng_get_bytes.c`, `headers/tomcrypt_custom.h:549-570`.
- Local NDK27.1 sysroot `usr/include/sys/random.h:45-57`,
  `bits/getentropy.h:42-53`, `stdlib.h:128-130`.
- [Official NDK cross-build guidance](https://developer.android.com/ndk/guides/other_build_systems): target API suffix, clang and archive tools.
- [Cargo native link/build-script contract](https://doc.rust-lang.org/cargo/reference/build-scripts.html): link ordering and target/host distinctions.
- [Android native page-size guidance](https://developer.android.com/guide/practices/page-sizes): final native alignment/runtime checks.

Web access to the Bionic Gitiles header URLs failed in this session; the installed
versioned NDK headers above were checked directly instead. No conclusion depends
on an unread page. No engine compile/runtime claim is made by this packet.

## Reviewed adapter implementation boundary

The `HOYA_ENTROPY_TEST` preprocessor switch is strictly for the standalone
fault-injection test binary. Production archive builds omit it; its injected
OS function declarations have no definitions in the production build and
would fail linking rather than silently replacing OS entropy.

## Minimal package-approval packet follow-up

Use existing Node/npm, Pods, JDK/Gradle/NDK and Swift tooling for dependency-only
scratch manifests after authorization; no global installs or floating latest
initializers. Pin the candidate RN0.86.3/React19.2.3/CLI20.1.0 and matching native
catalog, resolve complete npm lock, Maven dependency locks/checksums and Pod lock
without consumer implementation, review licenses/source/optional postinstall
scripts and artifacts, then ask CHG4 approval of that exact graph before build
activation. npm package-lock-only/ignore-scripts is useful metadata isolation but
does not resolve CocoaPods/Maven/native/Hermes artifacts. Swift Foundation/C ABI
and Kotlin stdlib/JNI add no generator; their packaging/toolchain fingerprints
still belong in the packet. Reuse approved crypto sources does not exempt the
new Android entropy/build path from formal review and actual security tests.
