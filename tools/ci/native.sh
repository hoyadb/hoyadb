#!/bin/bash
# SPEC-0050: run the existing reviewed native recipes without changing their gates.
set -euo pipefail
cd "$(dirname "$0")/../.."
test "$(uname -s)" = Darwin
test "$(uname -m)" = x86_64
test ! -e target/github-release
mkdir -p target/github-release/native target/github-release/npm

# The Kotlin/Android recipes deliberately use the locally reviewed tool graph.
# Keep these paths aligned with toolchain-review.json and the existing builders.
ndk=/Users/boristacyniak/Library/Android/sdk/ndk/27.1.12297006
ndk_bin="$ndk/toolchains/llvm/prebuilt/darwin-x86_64/bin"
test -x "$ndk_bin/clang"
test -x /Users/boristacyniak/.sdkman/candidates/java/17.0.1-tem/bin/javac
test -x /Applications/Android\ Studio.app/Contents/plugins/Kotlin/kotlinc/bin/kotlinc

cargo fetch --manifest-path crates/hoyadb-native/Cargo.toml --locked
rustup target add x86_64-apple-ios aarch64-apple-ios-sim aarch64-apple-ios x86_64-linux-android aarch64-linux-android
python3 bindings/swift/build-artifacts.py
parent_sha=$(shasum -a 256 bindings/swift/Artifacts/native-build.json | cut -d ' ' -f 1)
python3 bindings/swift/check_artifacts.py --expected-parent-sha256 "$parent_sha"
MACOSX_DEPLOYMENT_TARGET=13.0 cargo test --manifest-path crates/hoyadb-native/Cargo.toml --offline --locked
cargo fmt --manifest-path crates/hoyadb-native/Cargo.toml --check
MACOSX_DEPLOYMENT_TARGET=13.0 cargo clippy --manifest-path crates/hoyadb-native/Cargo.toml --all-targets --offline --locked -- -D warnings
swift test --package-path bindings/swift -Xswiftc -warnings-as-errors
xcrun swift-format lint --strict --recursive bindings/swift/Sources bindings/swift/Tests bindings/swift/Examples
python3 tools/release/compatibility.py swift --output target/github-release/swift-api

for abi in x86_64 arm64-v8a; do
    case "$abi" in
        x86_64) triple=x86_64-linux-android ;;
        arm64-v8a) triple=aarch64-linux-android ;;
    esac
    provider="$PWD/target/github-release/provider-$abi"
    build_target="$PWD/target/github-release/rust-$abi"
    python3 crates/hoyadb-native/android/build-provider.py --ndk "$ndk" --abi "$abi" --output "$provider"
    upper=$(printf '%s' "$triple" | tr '[:lower:]-' '[:upper:]_')
    lower=$(printf '%s' "$triple" | tr '-' '_')
    env LIBSQLITE3_SYS_USE_PKG_CONFIG=1 SQLCIPHER_LIB_DIR="$provider/lib" \
        SQLCIPHER_INCLUDE_DIR="$provider/include" SQLCIPHER_STATIC=1 \
        PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_LIBDIR="$provider/lib/pkgconfig" \
        "CARGO_TARGET_${upper}_LINKER=$ndk_bin/${triple}24-clang" \
        "CARGO_TARGET_${upper}_RUSTFLAGS=-Clink-arg=-Wl,-z,max-page-size=16384" \
        "CC_${lower}=$ndk_bin/${triple}24-clang" "AR_${lower}=$ndk_bin/llvm-ar" \
        CARGO_TARGET_DIR="$build_target" cargo build --manifest-path crates/hoyadb-native/Cargo.toml \
        --target "$triple" --release --offline --locked
    jni="target/phase-seven-native/jni-current/$abi"
    test ! -e "$jni"
    python3 crates/hoyadb-native/android/build-jni.py --abi "$abi" --ndk "$ndk" \
        --archive "$build_target/$triple/release/libhoyadb_native.a" --out "$jni"
    python3 tools/release/compatibility.py exports --kind android --slice "$abi" \
        --library "$jni/libhoyadb_jni.so" --nm "$ndk_bin/llvm-nm"
done
python3 bindings/android/tools/build.py --out target/github-release/kotlin
python3 bindings/android/tools/package-aar.py --classes target/github-release/kotlin/classes.jar \
    --out target/github-release/maven --abis x86_64 arm64-v8a
python3 packages/react-native/tools/pack.py \
    --source-framework bindings/swift/Artifacts/HoyaNative.xcframework \
    --expected-parent-sha256 "$parent_sha" --pack-destination target/github-release/npm \
    --extract-to target/github-release/rn-consumer --record target/github-release/native/rn-pack.json
cp bindings/swift/Artifacts/native-build.json target/github-release/native/
printf '%s\n' "$parent_sha" > target/github-release/native/APPLE_PARENT_SHA256
ditto -c -k --keepParent bindings/swift/Artifacts/HoyaNative.xcframework \
    target/github-release/native/HoyaNative.xcframework.zip
git rev-parse HEAD > target/github-release/SOURCE_REVISION
python3 tools/ci/central.py --maven target/github-release/maven \
    --out target/github-release/central --repository "$GITHUB_REPOSITORY"
