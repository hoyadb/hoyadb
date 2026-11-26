"""SPEC-0037 R11 / TC-0041 SI-X04: conversion is explicit native tooling only.

Static: no facade, SDK, WASM, native-bridge or CommandDatabase source names the
core re-encoder or the SQLite tooling entry. Graph: ordinary native and WASM
dependency graphs never enable `ledger-convert-tool`; the explicit feature does.
"""

import json
import re
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOKENS = ("command::convert", "convert::run", "convert::testing", "reencode(", "hoyadb-convert-ledger")
FORBIDDEN = [
    "crates/hoyadb-bindings/src",
    "crates/hoyadb-web/src",
    "crates/hoyadb-native/src",
    "crates/hoyadb-storage-memory/src",
    "packages",
    "bindings",
]
ALLOWED = {
    "crates/hoyadb-core/src/command/convert.rs",
    "crates/hoyadb-core/src/command/convert/tests.rs",
    "crates/hoyadb-core/src/command/mod.rs",
    "crates/hoyadb-storage-sqlite/src/convert.rs",
    "crates/hoyadb-storage-sqlite/src/lib.rs",
    "crates/hoyadb-storage-sqlite/src/bin/hoyadb-convert-ledger.rs",
    "crates/hoyadb-storage-sqlite/tests/convert_process.rs",
    "crates/hoyadb-storage-sqlite/Cargo.toml",
}


def cargo(args):
    result = subprocess.run(["cargo", *args, "--offline", "--locked"], cwd=ROOT, capture_output=True, text=True, check=False)
    return result.returncode, result.stdout + result.stderr


def dependency_features(manifest):
    """Features any manifest enables on its hoyadb-storage-sqlite dependency entries."""
    text = manifest.read_text()
    features = []
    for match in re.finditer(r"hoyadb-storage-sqlite\s*=\s*\{([^}]*)\}", text):
        body = match.group(1)
        found = re.search(r"features\s*=\s*\[([^\]]*)\]", body)
        if found:
            features.extend(item.strip().strip('"') for item in found.group(1).split(",") if item.strip())
    return features


class ConversionBoundaryTests(unittest.TestCase):
    def test_no_runtime_source_names_the_conversion_entries(self):
        offenders = []
        for base in FORBIDDEN + ["crates/hoyadb-core/src", "crates/hoyadb-storage-sqlite/src"]:
            for path in (ROOT / base).rglob("*"):
                if path.suffix not in {".rs", ".js", ".mjs", ".ts", ".kt", ".swift", ".c", ".h"} or not path.is_file():
                    continue
                relative = path.relative_to(ROOT).as_posix()
                if relative in ALLOWED or "/.build/" in relative or "/node_modules/" in relative:
                    continue
                text = path.read_text(errors="replace")
                for token in TOKENS:
                    if token in text:
                        offenders.append((relative, token))
        self.assertEqual(offenders, [])

    def test_sqlite_binary_requires_the_explicit_feature(self):
        manifest = (ROOT / "crates/hoyadb-storage-sqlite/Cargo.toml").read_text()
        self.assertIn('required-features = ["ledger-convert-tool"]', manifest)
        self.assertRegex(manifest, re.compile(r"^ledger-convert-tool = \[\]$", re.M))
        lib = (ROOT / "crates/hoyadb-storage-sqlite/src/lib.rs").read_text()
        self.assertIn('#[cfg(all(feature = "ledger-convert-tool", not(target_arch = "wasm32")))]\n#[doc(hidden)]\npub mod convert;', lib)
        convert = (ROOT / "crates/hoyadb-storage-sqlite/src/convert.rs").read_text()
        self.assertIn('#[cfg(feature = "test-hooks")]\n#[doc(hidden)]\npub mod testing', convert)

    def test_no_manifest_enables_the_feature_on_the_sqlite_dependency(self):
        manifests = [path for path in ROOT.rglob("Cargo.toml") if "target" not in path.parts and "node_modules" not in path.parts and ".build" not in path.parts]
        self.assertTrue(manifests)
        for manifest in manifests:
            self.assertNotIn("ledger-convert-tool", dependency_features(manifest), manifest)

    def test_binary_is_absent_without_the_feature_and_declared_with_it(self):
        code, output = cargo(["build", "-p", "hoyadb-storage-sqlite", "--bin", "hoyadb-convert-ledger"])
        self.assertNotEqual(code, 0)
        self.assertIn("requires the features: `ledger-convert-tool`", output)
        code, output = cargo(["metadata", "--format-version", "1", "--no-deps"])
        self.assertEqual(code, 0, output)
        package = next(p for p in json.loads(output)["packages"] if p["name"] == "hoyadb-storage-sqlite")
        self.assertIn("ledger-convert-tool", package["features"])
        self.assertEqual(package["features"]["ledger-convert-tool"], [])
        binary = next(t for t in package["targets"] if t["name"] == "hoyadb-convert-ledger")
        self.assertEqual(binary["required-features"], ["ledger-convert-tool"])
        self.assertIn("bin", binary["kind"])
