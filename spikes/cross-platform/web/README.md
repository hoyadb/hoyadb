# Isolated browser validation — SPEC-0017

This unpublished bridge uses the same `hoyadb-cross-platform-spike` core as the
Swift experiment. It proves an insertion-only fixture, not a production API.
The operator approved wasm-bindgen 0.2.104 and reviewed locked transitives.
Core remains dependency-free. No npm dependency or bundler is required.

From repository root, with the pinned Rust toolchain and wasm-bindgen CLI 0.2.104:

```sh
cargo fetch --manifest-path spikes/cross-platform/web/Cargo.toml --locked
cargo fmt --manifest-path spikes/cross-platform/web/Cargo.toml --check
cargo clippy --manifest-path spikes/cross-platform/web/Cargo.toml --offline --locked --all-targets -- -D warnings
cargo build --manifest-path spikes/cross-platform/web/Cargo.toml --target wasm32-unknown-unknown --offline --locked
wasm-bindgen --target web --out-dir spikes/cross-platform/web/pkg spikes/cross-platform/web/target/wasm32-unknown-unknown/debug/hoyadb_browser_spike.wasm
python3 -m http.server 8765 --bind 127.0.0.1 --directory spikes/cross-platform/web
```

Open `http://127.0.0.1:8765/` in a browser supporting WebAssembly, ES modules and
BigInt. All B01–B05 entries must pass; actual rows, runtime, and measured
write-to-callback milliseconds appear on the page and in the console. Generated
`pkg` and Cargo `target` are ignored. Browser automation may inspect
`window.hoyaResults.every(result => result.passed)` after module initialization.

The JS fixture wrapper validates BigInt and signed range, then sends decimal
text into Rust for checked i64 parsing. Reads return owned row snapshots; the
harness copies their values and explicitly frees row/snapshot/store objects.
Notification token/count snapshots are drained first; only after the mutable
WASM call returns does JavaScript invoke callbacks. B05 callbacks re-enter for
reads only; mutation, recursive drain and disposal inside a callback are not
part of this slice. Unsubscribe is tested between drains. This mechanism needs
host scheduling and is not an unsolicited Rust push callback.

A reload sentinel is retained in a memory store until page navigation; inspect
`window.hoyaReloadRowsBefore` after reload: it remains empty. No IndexedDB,
OPFS, localStorage or persistent browser engine is used. Reloading reruns all
fixtures and reports fresh state. Stop the loopback server with Ctrl-C.

All five browser cases first failed for missing insert/query/observe, then
passed in Paseo 0.12.0 / Electron 44.2.0 / Chrome 152.0.7977.76 on macOS. One
write-to-callback sample was 3.5 ms, including immediate host drain and event-snapshot copying; this is a sample, not a latency budget or benchmark. Existing native scope
handling remains the separate async-host proof.
