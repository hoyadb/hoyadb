#!/usr/bin/env python3
"""API-0011 / TC-0036: build.sh guards, exercised on a task-owned copy.

The script derives the repository root from its own location, so a copy inside
a temporary tree with stub `cargo`/`wasm-bindgen` tools never touches this
checkout's `target/` or generated `pkg/`. No compiler or network is used.

    python3 -m unittest discover -s crates/hoyadb-web/tests -p 'test_*.py'
"""
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "build.sh"

# Honours the same target-directory precedence as the real tool, then emits a
# wasm whose bytes name the features it was "built" with.
CARGO = """#!/bin/sh
target=${CARGO_TARGET_DIR:-}
manifest=
features=none
while [ $# -gt 0 ]; do
    case "$1" in
        --target-dir) target=$2; shift ;;
        --manifest-path) manifest=$2; shift ;;
        --features) features=$2; shift ;;
    esac
    shift
done
[ -n "$target" ] || target=$(dirname "$manifest")/target
mkdir -p "$target/wasm32-unknown-unknown/release"
printf 'fresh %s\\n' "$features" > "$target/wasm32-unknown-unknown/release/hoyadb_web.wasm"
"""

# Copies its input next to a JS shim that exports the test controls only when
# the input was built with them.
BINDGEN = """#!/bin/sh
if [ "$1" = --version ]; then echo 'wasm-bindgen 0.2.129'; exit 0; fi
input=$1
shift
while [ $# -gt 0 ]; do
    case "$1" in --out-dir) out=$2; shift ;; esac
    shift
done
mkdir -p "$out"
cp "$input" "$out/hoyadb_web_bg.wasm"
if grep -q test-controls "$input"; then
    echo 'export function testFile() {}' > "$out/hoyadb_web.js"
else
    echo 'export class Facade {}' > "$out/hoyadb_web.js"
fi
"""

# A minimal stand-in so the positive cases do not depend on an installed ripgrep.
RG = """#!/bin/sh
[ "$1" = -q ] && shift
exec grep -Eq "$1" "$2"
"""


class BuildScript(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="hoya-web-build-"))
        self.addCleanup(shutil.rmtree, self.root, True)
        self.crate = self.root / "repo/crates/hoyadb-web"
        self.crate.mkdir(parents=True)
        shutil.copy(SCRIPT, self.crate / "build.sh")
        self.tools = self.root / "tools"
        self.tools.mkdir()
        self.tool("cargo", CARGO)
        self.bindgen = self.tool("wasm-bindgen", BINDGEN)
        # Only the system utilities the script itself needs: no ambient ripgrep.
        for name in ["dirname", "uname", "mkdir", "cp", "grep"]:
            (self.tools / name).symlink_to(shutil.which(name))

    def tool(self, name, body):
        path = self.tools / name
        path.write_text(body)
        path.chmod(path.stat().st_mode | stat.S_IXUSR)
        return path

    def build(self, *arguments, **environment):
        return subprocess.run(
            ["/bin/sh", str(self.crate / "build.sh"), *arguments],
            env={"PATH": str(self.tools), "HOYADB_WASM_BINDGEN": str(self.bindgen), **environment},
            capture_output=True, text=True, check=False)

    def packaged(self, directory="pkg"):
        return (self.crate / directory / "hoyadb_web_bg.wasm").read_text()

    def test_ordinary_build_packages_the_wasm_it_just_built(self):
        self.tool("rg", RG)
        result = self.build()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.packaged(), "fresh none\n")

    def test_missing_ripgrep_fails_the_test_control_guard(self):
        result = self.build()
        self.assertNotEqual(result.returncode, 0, "an unchecked package was accepted")
        self.assertIn("could not be checked", result.stderr)

    def test_test_controls_in_ordinary_assets_are_still_rejected(self):
        self.tool("rg", RG)
        self.tool("wasm-bindgen", BINDGEN.replace("grep -q test-controls", "grep -q fresh"))
        result = self.build()
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("Test controls must not enter", result.stderr)

    def test_redirected_target_directory_never_packages_a_stale_wasm(self):
        self.tool("rg", RG)
        stale = self.crate / "target/wasm32-unknown-unknown/release"
        stale.mkdir(parents=True)
        (stale / "hoyadb_web.wasm").write_text("stale test-controls\n")
        result = self.build(CARGO_TARGET_DIR=str(self.root / "elsewhere"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.packaged(), "fresh none\n")

    def test_test_controls_build_stays_in_its_own_directory(self):
        result = self.build("--test-controls")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.packaged("pkg-test"), "fresh test-controls\n")
        self.assertFalse((self.crate / "pkg").exists())


if __name__ == "__main__":
    unittest.main()
