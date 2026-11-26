# HoyaDB Android package third-party notices

Unpublished `dev.hoyadb:hoyadb-android:0.1.0` artifact. First-party HoyaDB code
uses MIT; its license text is included as `third-party/LICENSE`. This inventory
is not a legal compliance certification. The machine-checked inventory is
[third-party/index.json](third-party/index.json); every listed file is copied
byte-for-byte from a verified source recorded there, and the final packager
rejects a missing, changed or unclassified entry.

## Linked into `libhoyadb_jni.so`

Rust crates on the Android target's normal dependency graph (both Cargo.lock
files, phase-six `active_native` set):

| Crate | Version | License | Texts |
| --- | --- | --- | --- |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | third-party/rust/bitflags-2.13.2/ |
| fallible-iterator | 0.3.0 | MIT/Apache-2.0 | third-party/rust/fallible-iterator-0.3.0/ |
| fallible-streaming-iterator | 0.1.9 | MIT/Apache-2.0 | third-party/rust/fallible-streaming-iterator-0.1.9/ |
| libsqlite3-sys | 0.38.1 (git 661107f) | MIT | third-party/rust/libsqlite3-sys-0.38.1/ |
| rusqlite | 0.40.1 (git 661107f) | MIT | third-party/rust/rusqlite-0.40.1/ |
| smallvec | 1.16.4 | MIT OR Apache-2.0 | third-party/rust/smallvec-1.16.4/ |
| zeroize | 1.9.1 | Apache-2.0 OR MIT | third-party/rust/zeroize-1.9.1/ |

C code compiled from the sqlcipher-src source package (the Android profile uses
the LibTomCrypt provider, not Apple CommonCrypto):

| Component | Version | License | Text |
| --- | --- | --- | --- |
| SQLCipher | 4.19.0 | BSD-3-Clause (Zetetic LLC) | third-party/native/sqlcipher-4.19.0/LICENSE-sqlcipher |
| SQLite | 3.53.4 | public-domain blessing | third-party/native/sqlite-3.53.4/LICENSE-sqlite |
| LibTomCrypt | 1.18.2 | public domain OR WTFPL | third-party/native/libtomcrypt-1.18.2/LICENSE-libtomcrypt |

## Build-only (no code in the shipped library)

sqlcipher-src 419.0.6+sqlcipher-4.19.0-sqlite-3.53.4-libtomcrypt-1.18.2 (MIT;
build role, supplies the C sources above), cc 1.10.0, find-msvc-tools 0.1.15,
pkg-config 0.3.34, shlex 2.0.1 and vcpkg 0.2.15 (each MIT OR Apache-2.0). Their
texts are under third-party/rust/.

## Kotlin runtime (POM dependency graph, not bundled in classes.jar)

| Coordinate | License | Texts |
| --- | --- | --- |
| org.jetbrains.kotlin:kotlin-stdlib:2.1.20 | Apache-2.0 | third-party/kotlin/kotlin-stdlib-2.1.20/ (LICENSE, NOTICE and the GWT, ThreeTen-Backport, Guava and Boost texts the v2.1.20 license README attributes to stdlib sources) |
| org.jetbrains:annotations:13.0 | Apache-2.0 | third-party/kotlin/annotations-13.0/LICENSE-2.0.txt (publisher POM-declared license URL) |

## Toolchain aggregate notices (supplemental)

The pinned toolchains statically link runtime code into `libhoyadb_jni.so`. The
verified link inputs (exact paths and SHA256 in the index) are the Rust 1.98.1
Android sysroot crates found as members of the hash-bound staticlib (std, core,
alloc, compiler_builtins, unwind, panic_unwind, std_detect, hashbrown, libc,
memchr, gimli, object, miniz_oxide, rustc_demangle, cfg_if and
rustc_std_workspace_alloc/core) and the NDK r27b clang driver's
compiler-rt builtins, libunwind and bionic crtbegin_so/crtend_so objects.

Byte-exact copies of the locally shipped, version-bound aggregate texts:
third-party/toolchain/rust-1.98.1/COPYRIGHT-library.html and
third-party/toolchain/ndk-27.1.12297006/{NOTICE.toolchain,NOTICE}. These are
aggregate notices: not every item they list is linked, and this is neither a
compiler SBOM nor a legal certification. The Rust text does not name
compiler_builtins, miniz_oxide, panic_unwind, std_detect or
rustc_std_workspace_alloc/core individually (recorded in the index); the NDK
texts do not name the crt objects individually. HoyaDB workspace crates are
first-party.
