# General browser WebAssembly boundary

This crate exports the schema-independent API-0011 facade. It uses the already
approved Phase 6 Rust dependency versions, not the fixed host spike.

Build ordinary optimized consumer assets with:

```sh
sh crates/hoyadb-web/build.sh
```

The script checks the reviewed wasm-bindgen0.2.129 CLI. Install that exact tool
into the default location from the repository root (this setup step needs network):

```sh
cargo install wasm-bindgen-cli --version 0.2.129 --locked --root target/tools/wasm-bindgen
```

The default location is `target/tools/wasm-bindgen/bin/wasm-bindgen`; set
`HOYADB_WASM_BINDGEN` to another existing CLI of that exact version when rebuilding
from a clean checkout. Fetch the pinned Cargo lock before using offline builds.
On macOS, the documented LLVM22 compiler/archiver provide the WASM backend;
`CC_wasm32_unknown_unknown` and `AR_wasm32_unknown_unknown` can select those tools
explicitly. Apple clang does not supply the required target in this environment.

`--debug` is an explicit development artifact, never a performance baseline.
`--test-controls` writes `pkg-test/` and enables closed task-owned logical-file
controls for the security harness. It never writes ordinary `pkg/` assets. The
ordinary build checks that test exports are absent and fails if that check cannot
run (it needs `rg`). The script always builds into `crates/hoyadb-web/target`,
overriding `CARGO_TARGET_DIR`, so it packages the WASM it just built. Generated
assets and target outputs are ignored; local consumer packages copy the ordinary
assets. `python3 -m unittest discover -s crates/hoyadb-web/tests -p 'test_*.py'`
checks these guards on a temporary copy with stub tools.

Load WASM explicitly before measuring hermetic memory open. Construct one Facade
per shared runtime/worker; `open` receives a separate binary key, `call` accepts
ordinary ABI1 frames, and pure `validateOpen` checks exact original requests
before protected VFS setup. Memory does not invoke `initStorage`. Protected OPFS
requires a dedicated worker and awaits `initStorage`, using the preserved
`hoyadb-v1` pool with24 initial slots. Only one active worker may own that pool
per origin, including when logical store names differ. Concurrent pool ownership
is unsupported; after an initialization failure, a later explicit open can retry
once the competing worker releases the pool. Browser backup policy is `unsupported`.

The real preload/retained-memory smoke test runs after a build:

```sh
node crates/hoyadb-web/tests/memory.mjs
```

Further actual SDK/browser coverage is indexed in the Phase 7 implementation
report. An import or build alone is not target support or phase completion.
