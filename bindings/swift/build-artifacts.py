#!/usr/bin/env python3
"""Build local approved-source Apple artifacts. No package fetching/publication.

After xcodebuild creates the native framework and native-build.json records its native members, the
shared DEF-0030 notice finalizer always adds and validates the notice closure; a failure fails the build.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
stage = root / "target/phase-seven-native"
manifest = root / "crates/hoyadb-native/Cargo.toml"
headers = root / "crates/hoyadb-native/include"
artifacts = root / "bindings/swift/Artifacts"
artifacts.mkdir(exist_ok=True)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    # DEF-0060: enumerate again after compilation so additions/removals count as
    # drift too. Source resources may be include_bytes!/include_str! inputs.
    paths = {root / "Cargo.toml", root / "Cargo.lock", root / "rust-toolchain.toml",
             Path(__file__).resolve(), root / "crates/hoyadb-native/Cargo.lock"}
    for name in ["hoyadb-core", "hoyadb-storage-memory", "hoyadb-storage-sqlite", "hoyadb-bindings", "hoyadb-native"]:
        crate = root / "crates" / name
        paths.add(crate / "Cargo.toml")
        paths.update(p for p in (crate / "src").rglob("*") if p.is_file())
        if (crate / "build.rs").exists():
            paths.add(crate / "build.rs")
        # Only build configuration, never Cargo credentials or generated files.
        for directory in (root, root / "crates", crate):
            paths.update(p for p in (directory / ".cargo/config", directory / ".cargo/config.toml") if p.exists())
    paths.update(p for p in headers.rglob("*") if p.is_file())
    return {str(p.relative_to(root)): digest(p) for p in sorted(paths)}


before = sources()
commands = []

def run(command, env=None):
    commands.append({"argv": command, "overrides": env or {}})
    subprocess.run(command, cwd=root, env=dict(os.environ, **(env or {})), check=True)


run(["cargo", "build", "--manifest-path", str(manifest), "--release", "--offline", "--locked"], {"MACOSX_DEPLOYMENT_TARGET": "13.0"})
mac = root / "crates/hoyadb-native/target/release/libhoyadb_native.a"
libs = {}
for name, triple, sdk in [
    ("ios-simulator", "x86_64-apple-ios", "iphonesimulator"),
    ("ios-arm64-simulator", "aarch64-apple-ios-sim", "iphonesimulator"),
    ("ios-device", "aarch64-apple-ios", "iphoneos"),
]:
    sdkroot = subprocess.check_output(["xcrun", "--sdk", sdk, "--show-sdk-path"], text=True).strip()
    run(["cargo", "build", "--manifest-path", str(manifest), "--target", triple, "--release", "--offline", "--locked"],
        {"CARGO_TARGET_DIR": str(stage / name), "SDKROOT": sdkroot, "IPHONEOS_DEPLOYMENT_TARGET": "15.1"})
    libs[triple] = stage / name / triple / "release/libhoyadb_native.a"

# Every XCFramework slice keeps one library basename: CocoaPods rejects differing
# static binary names. A separate staging directory avoids colliding with inputs.
universal = stage / "xcframework-simulator" / "libhoyadb_native.a"
universal.parent.mkdir(exist_ok=True)
run(["xcrun", "lipo", "-create", str(libs["x86_64-apple-ios"]), str(libs["aarch64-apple-ios-sim"]), "-output", str(universal)])
output = artifacts / "HoyaNative.xcframework"
if output.exists():
    shutil.rmtree(output)  # Only this generated, ignored task artifact.
run(["xcodebuild", "-create-xcframework", "-library", str(mac), "-headers", str(headers),
     "-library", str(libs["aarch64-apple-ios"]), "-headers", str(headers),
     "-library", str(universal), "-headers", str(headers), "-output", str(output)])
after = sources()
if before != after:
    raise RuntimeError("Source changed during build; artifact certification refused")
evidence = {"profile": "optimized approved-source SQLCipher4.19.0; no LTO/size threshold claim",
            "runtime_verified": False, "macos_architectures": ["x86_64"], "ios_device_architectures": ["arm64"],
            "ios_simulator_architectures": ["x86_64", "arm64"], "source_sha256": after, "commands": commands,
            "artifact_files_sha256": {str(p.relative_to(artifacts)): digest(p) for p in sorted(output.rglob("*")) if p.is_file()}}
(artifacts / "native-build.json").write_text(json.dumps(evidence, indent=2) + "\n")

spec = importlib.util.spec_from_file_location("apple_notices", root / "crates/hoyadb-native/tools/apple_notices.py")
notices = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notices)
try:
    packaged = notices.finalize_framework(output, artifacts / "native-build.json", root / "crates/hoyadb-native/third-party",
                                          root / "crates/hoyadb-native/Cargo.lock")
except notices.NoticeError as error:
    sys.exit(f"Notice packaging rejected; the framework is not releasable: {error}")
print("HOYA_APPLE_ARTIFACT_BUILD_GREEN source_files=" + str(len(after)) + " " + json.dumps(packaged, sort_keys=True))
