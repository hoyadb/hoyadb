#!/bin/sh
# API-0011 / TC-0036: ordinary release assets by default; harness controls opt in.
set -eu
repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd -P)
profile=release
feature=
output=pkg
for argument in "$@"; do
    case "$argument" in
        --debug) profile=debug ;;
        --test-controls) feature=test-controls; output=pkg-test ;;
        *) printf 'Unknown build option\n' >&2; exit 2 ;;
    esac
done
bindgen=${HOYADB_WASM_BINDGEN:-"$repo/target/tools/wasm-bindgen/bin/wasm-bindgen"}
if [ ! -x "$bindgen" ] || [ "$("$bindgen" --version)" != 'wasm-bindgen 0.2.129' ]; then
    printf 'Set HOYADB_WASM_BINDGEN to the reviewed wasm-bindgen 0.2.129 CLI\n' >&2
    exit 2
fi
if [ "$(uname -s)" = Darwin ]; then
    : "${CC_wasm32_unknown_unknown:=/usr/local/opt/llvm@22/bin/clang}"
    : "${AR_wasm32_unknown_unknown:=/usr/local/opt/llvm@22/bin/llvm-ar}"
    export CC_wasm32_unknown_unknown AR_wasm32_unknown_unknown
fi
# Pin the output directory: CARGO_TARGET_DIR or a build.target-dir setting would
# otherwise send the build elsewhere and leave an older crate-local wasm to package.
target="$repo/crates/hoyadb-web/target"
set -- build --manifest-path "$repo/crates/hoyadb-web/Cargo.toml" --target-dir "$target" --target wasm32-unknown-unknown --offline --locked
if [ "$profile" = release ]; then set -- "$@" --release; fi
if [ -n "$feature" ]; then set -- "$@" --features "$feature"; fi
cargo "$@"
"$bindgen" "$target/wasm32-unknown-unknown/$profile/hoyadb_web.wasm" --target web --out-dir "$repo/crates/hoyadb-web/$output"
if [ -z "$feature" ]; then
    # rg: 0 = found, 1 = absent. A missing rg or unreadable asset must not pass.
    found=0
    rg -q 'testFile|testPoolStats' "$repo/crates/hoyadb-web/$output/hoyadb_web.js" || found=$?
    case "$found" in
        0) printf 'Test controls must not enter ordinary package assets\n' >&2; exit 1 ;;
        1) ;;
        *) printf 'Ordinary package assets could not be checked for test controls (ripgrep required)\n' >&2; exit 2 ;;
    esac
fi
