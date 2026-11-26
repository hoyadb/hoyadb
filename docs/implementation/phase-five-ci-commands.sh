set -eu
# SPEC-0031 / TC-0034: run from the frozen repository copy in a fresh container.
apt-get update
apt-get install --no-install-recommends -y clang llvm
rustup component add --toolchain 1.98.1 rustfmt clippy
rustup target add --toolchain 1.98.1 wasm32-unknown-unknown
rustc --version
clang --version
llvm-ar --version
cargo fetch --locked
cargo fetch --manifest-path spikes/storage-browser/Cargo.toml --locked
cargo fetch --manifest-path spikes/cross-platform/native/Cargo.toml --locked
cargo fetch --manifest-path spikes/cross-platform/web/Cargo.toml --locked
cargo fmt --all --check
sh tests/tooling/formatting.sh
cargo test --workspace --offline --locked
cargo run -p hoyadb-storage-memory --example data_model --offline --locked
cargo run -p hoyadb-storage-memory --example reactive --offline --locked
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo build -p hoyadb-core -p hoyadb-storage-memory --target wasm32-unknown-unknown --offline --locked
cargo fmt --manifest-path spikes/storage-browser/Cargo.toml --check
cargo clippy -p hoyadb-storage-sqlite --lib --target wasm32-unknown-unknown --offline --locked -- -D warnings
cargo build -p hoyadb-storage-sqlite --lib --target wasm32-unknown-unknown --offline --locked
cargo clippy --manifest-path spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --offline --locked -- -D warnings
cargo clippy --manifest-path spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --release --offline --locked -- -D warnings
cargo build --manifest-path spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --release --offline --locked
cargo test --manifest-path spikes/cross-platform/Cargo.toml --offline --locked
cargo fmt --manifest-path spikes/cross-platform/Cargo.toml --check
cargo clippy --manifest-path spikes/cross-platform/Cargo.toml --all-targets --offline --locked -- -D warnings
cargo build --manifest-path spikes/cross-platform/Cargo.toml --target wasm32-unknown-unknown --offline --locked
cargo test --manifest-path spikes/cross-platform/native/Cargo.toml --offline --locked
cargo fmt --manifest-path spikes/cross-platform/native/Cargo.toml --check
cargo clippy --manifest-path spikes/cross-platform/native/Cargo.toml --all-targets --offline --locked -- -D warnings
cargo fmt --manifest-path spikes/cross-platform/web/Cargo.toml --check
cargo clippy --manifest-path spikes/cross-platform/web/Cargo.toml --all-targets --offline --locked -- -D warnings
cargo build --manifest-path spikes/cross-platform/web/Cargo.toml --target wasm32-unknown-unknown --offline --locked
printf 'PHASE5_CONTAINER_GREEN\n'
