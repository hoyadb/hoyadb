#!/bin/sh
# SPEC-0023 / TC-0023 W09 and SPEC-0025 / TC-0025 P13: deterministic compile gates, not browser runtime proof.
set -eu
cd "$(dirname "$0")/../.."
: "${CC_wasm32_unknown_unknown:?Select a Clang with the wasm32 backend}"
: "${AR_wasm32_unknown_unknown:?Select its llvm-ar}"
export CC_wasm32_unknown_unknown AR_wasm32_unknown_unknown
test "$(wasm-bindgen --version)" = 'wasm-bindgen 0.2.129'
cargo fetch --locked
cargo fetch --manifest-path spikes/storage-browser/Cargo.toml --locked
cargo fmt --manifest-path spikes/storage-browser/Cargo.toml --check
cargo clippy -p hoyadb-storage-sqlite --lib --target wasm32-unknown-unknown --offline --locked -- -D warnings
cargo build -p hoyadb-storage-sqlite --lib --target wasm32-unknown-unknown --offline --locked
cargo clippy --manifest-path spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --offline --locked -- -D warnings
cargo build --manifest-path spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --offline --locked
spike_target_dir=${CARGO_TARGET_DIR:-spikes/storage-browser/target}
wasm-bindgen --target web --out-dir spikes/storage-browser/pkg "$spike_target_dir/wasm32-unknown-unknown/debug/hoyadb_storage_browser_spike.wasm"
