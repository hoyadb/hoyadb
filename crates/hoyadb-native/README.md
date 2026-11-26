# HoyaDB native boundary (ABI1, local and unpublished)

This standalone crate forwards the general `hoyadb-bindings::Facade`. It does
not implement schema, query, queue or entity algorithms. Link one facade runtime
per application process. The public C header is `include/hoyadb.h`; Swift uses
its `HoyaNative` module map. Android JNI owns byte-array copies and keeps each
JNIEnv/local reference on its managed thread.

The four C symbols are `hoya_v1_abi_version`, `hoya_v1_open`, `hoya_v1_call` and
`hoya_v1_validate_open`. The last is an additive readonly preflight using the
existing strict parser without a registry, owner, key bytes, filesystem or cipher
initialization. All operations return exactly one copied ABI1 JSON reply on
status0. Keys enter only the separate binary open span; a protected key is32
bytes and memory uses0. Invalid nonempty spans, excessive lengths and malformed
UTF8 reject before dereference. Readable accessibility and callback lifetime are
caller obligations; arbitrary admitted-size invalid addresses are not made safe.

Rust panic containment needs unwinding and returns a static boundary status.
It does not replace the application's panic hook, catch process aborts or contain
foreign exceptions/signals. No app handler or event callback executes in the C
collector. Its borrowed reply must be copied before returning, after the Rust
registry guard has been released.

```sh
MACOSX_DEPLOYMENT_TARGET=13.0 cargo test --manifest-path crates/hoyadb-native/Cargo.toml --offline --locked
cargo fmt --manifest-path crates/hoyadb-native/Cargo.toml --check
MACOSX_DEPLOYMENT_TARGET=13.0 cargo clippy --manifest-path crates/hoyadb-native/Cargo.toml --all-targets --offline --locked -- -D warnings
```

Apple consumers link existing Security and CoreFoundation frameworks. The Swift
package's `build-artifacts.py` records the exact optimized archive/XCFramework
source and output hashes. Android's `android/build-provider.py` verifies the
approved SQLCipher source archive and extracted files, replaces only its reviewed
OS-entropy dispatcher and emits target-local archives/configuration evidence.
Its production builds exclude the `HOYA_ENTROPY_TEST` fault-injection switch.
`android/build-jni.py` links an existing Rust archive with the exact eight-symbol
C/JNI export allowlist and16KiB ELF alignment. It records the Rust source closure
(`android/rust_sources.py`: manifests, lockfiles, toolchain pin, the five native
crates' sources and the C headers) in its evidence for the AAR packager, and
refuses an archive older than any of those files or a closure that changes
during the link. `tests/exports.py` checks exports;
compile/link success does not prove target runtime or cipher compatibility.

Both Android builders accept `x86_64`, `arm64-v8a`, `armeabi-v7a` and `x86`.
The ARM32 Rust target is `armv7-linux-androideabi`; its NDK compiler target is
`armv7a-linux-androideabi24`. The x86 Rust target is `i686-linux-android`, using
`i686-linux-android24-clang`. Existing 64-bit targets and API24 are unchanged.
Build each provider into a fresh target-owned directory, select its exact
SQLCipher include/library/pkgconfig paths for the matching Rust target, then
pass that archive to `build-jni.py`. Never mix a host or another ABI's provider.
`python3 -m unittest discover -s crates/hoyadb-native/android -p 'test_*.py'`
checks compiler mappings and retained boundary flags without native compilation.

The narrow [Android32 runtime probe](tests/android32/run.py) consumes a local
four-ABI AAR and an already installed API30 Play Store x86_64 image. Set
`JAVA_HOME` to the installed JDK, then run from the repository root:

```sh
python3 crates/hoyadb-native/tests/android32/run.py build --aar <local-AAR> --out <fresh-probe-directory>
python3 crates/hoyadb-native/tests/android32/run.py run --out <same-probe-directory>
```

It uses a unique disposable app per ABI and an owned AVD, requires the device
slot to be empty, and retains logs while deleting only its own app/AVD. The
[instrumentation](tests/android32/Native32Instrumentation.java) tests protected
restart, pending dependency order, partial patch and atomic marker rollback,
64-bit integer values, canonical mappings and aliases after receipt forgetting.
`--monstera-module <storage-key-module>` on the build command additionally uses
that checkout's existing plaintext SQLite coexistence fixture read-only. The
probe records actual process width and page size; ARM32 through the installed
native bridge is translation evidence, not physical-device evidence. It does
not replace the independent 16KiB consumer gate.

The native provider policy and supply-chain approval are recorded in
[the reviewed Android recipe](../../docs/implementation/phase-seven-android-crypto.md),
[ADR-0034](../../docs/design/decisions/ADR-0034-protected-engines-and-diagnostics.md)
and [ADR-0035](../../docs/design/decisions/ADR-0035-general-sdk-and-restricted-mobile-builds.md).
Existing protected-engine source/license obligations and the exact approved
mobile dependency packet remain applicable; this crate does not relicense them.
No runtime dependency was added by this C/JNI boundary.

Platform wrappers also cooperate with the payload-free admission-release broker
specified in API-0011. Direct C/wire callers sharing managed owners must notify
the platform broker after terminal attempts and must not consume an SDK-owned
observation stream. The C functions do not run host callbacks or own a second
admission queue. Final runtime/packaging support is recorded by the Phase7 report,
not inferred from the presence of these sources.
