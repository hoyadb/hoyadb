set -eu
apt-get update
apt-get install --no-install-recommends -y clang llvm
rustup component add --toolchain 1.98.1 rustfmt clippy
rustup target add --toolchain 1.98.1 wasm32-unknown-unknown
rustc --version
clang --version
llvm-ar --version
cargo fetch --locked
cargo fetch --manifest-path spikes/storage-browser/Cargo.toml --locked
cargo fmt --all --check
cargo test --workspace --offline --locked
cargo run -p hoyadb-storage-memory --example data_model --offline --locked
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo build -p hoyadb-core -p hoyadb-storage-memory --target wasm32-unknown-unknown --offline --locked
cargo fmt --manifest-path spikes/storage-browser/Cargo.toml --check
cargo clippy -p hoyadb-storage-sqlite --lib --target wasm32-unknown-unknown --offline --locked -- -D warnings
cargo build -p hoyadb-storage-sqlite --lib --target wasm32-unknown-unknown --offline --locked
cargo clippy --manifest-path spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --offline --locked -- -D warnings
cargo build --manifest-path spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --offline --locked
printf 'PHASE2_CONTAINER_GREEN\n'
