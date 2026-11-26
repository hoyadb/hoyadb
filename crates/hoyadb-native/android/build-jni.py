#!/usr/bin/env python3
"""Build the reviewed byte-array JNI boundary using an existing Rust staticlib."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
targets = {"x86_64": "x86_64-linux-android", "arm64-v8a": "aarch64-linux-android",
           "armeabi-v7a": "armv7a-linux-androideabi", "x86": "i686-linux-android"}
parser.add_argument("--abi", choices=targets, required=True)
parser.add_argument("--ndk", type=Path, required=True)
parser.add_argument("--archive", type=Path, required=True)
parser.add_argument("--out", type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[3]
triple = targets[args.abi]
compiler = args.ndk / "toolchains/llvm/prebuilt/darwin-x86_64/bin" / (triple + "24-clang")
archive = args.archive.resolve(strict=True)
args.out.mkdir(parents=True, exist_ok=True)
out = args.out.resolve() / "libhoyadb_jni.so"
source = root / "crates/hoyadb-native/android/hoya_jni.c"
exports = root / "crates/hoyadb-native/android/exports.map"
# Bind the Rust source closure as bindings/swift/build-artifacts.py does. This script does not
# compile the archive, so an archive older than any bound source is refused instead of certified.
spec = importlib.util.spec_from_file_location("hoyadb_rust_sources", Path(__file__).resolve().parent / "rust_sources.py")
closure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(closure)
rust_before = closure.rust_sources(root)
if any(path.stat().st_mtime > archive.stat().st_mtime for path in closure.rust_source_paths(root)):
    raise SystemExit("Rust archive predates a bound Rust source; rebuild it first")
command = [str(compiler), "-std=c11", "-O2", "-fPIC", "-shared", "-Wall", "-Wextra", "-Werror",
           "-I" + str(root / "crates/hoyadb-native/include"), str(source), str(archive),
           "-Wl,--version-script," + str(exports), "-Wl,--no-undefined", "-Wl,-z,max-page-size=16384",
           "-pthread", "-ldl", "-lm", "-o", str(out)]
subprocess.run(command, check=True)
if closure.rust_sources(root) != rust_before:
    raise SystemExit("Rust source changed during the JNI link; evidence refused")
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
evidence = {"abi": args.abi, "min_sdk": 24, "runtime_verified": False, "command": command,
            "inputs_sha256": {str(p.relative_to(root)) if p.is_relative_to(root) else str(p): sha(p)
                              for p in [source, exports, root / "crates/hoyadb-native/include/hoyadb.h", archive]},
            "rust_sources_sha256": rust_before, "artifact": str(out), "artifact_sha256": sha(out)}
(args.out / "evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")
print("HOYA_JNI_BUILD_GREEN", args.abi, evidence["artifact_sha256"])
