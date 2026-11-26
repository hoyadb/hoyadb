#!/usr/bin/env python3
"""TC-0036 G28 / DEF-0027: preserve upstream bytes through real Git filters.

Uses an isolated index and checkout directory; never changes the actual index,
worktree files or Git configuration. Run directly from any working directory.
"""

import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CLEANUP = "docs/implementation/phase-seven-kotlin-final/package/cleanup.txt"
CONTROL = "bindings/android/src/main/kotlin/dev/hoyadb/Database.kt"
RECORD = "docs/implementation/phase-seven-portable-declarations/record-before.json"


class GitProvenance(unittest.TestCase):
    def test_g28_upstream_bytes_survive_clean_and_checkout(self):
        paths = sorted(
            str(p.relative_to(ROOT))
            for corpus in [*(ROOT / "bindings").glob("*/third-party"), ROOT / "crates/hoyadb-native/third-party", ROOT / "licenses/javascript"]
            for p in corpus.rglob("*")
            if p.is_file()
        )
        self.assertTrue(any(p.startswith("bindings/android/") for p in paths))
        self.assertTrue(any(p.startswith("licenses/javascript/") for p in paths))
        license_paths = sorted(
            str(p.relative_to(ROOT))
            for corpus in (ROOT / "docs/implementation").glob("**/licenses")
            for p in corpus.rglob("*")
            if p.is_file()
        )
        self.assertIn(
            "docs/implementation/phase-seven-security-remediation-followup-3/licenses/jdom-2.0.6.1-LICENSE.txt",
            license_paths,
        )
        self.assertTrue(any(b"\r\n" in (ROOT / p).read_bytes() for p in license_paths))
        paths.extend(license_paths)
        paths.append(CLEANUP)
        original = {p: (ROOT / p).read_bytes() for p in paths}
        control = (ROOT / CONTROL).read_bytes()
        self.assertIn(b"\n", control)
        self.assertNotIn(b"\r", control)
        # Pin the explicit restoration to evidence already committed before the
        # correction, rather than silently accepting a recaptured LF digest.
        record = json.loads(subprocess.check_output(
            ["git", "cat-file", "blob", "HEAD:" + RECORD], cwd=ROOT
        ))["mirror_sources"]
        for name in ("LICENSE-APACHE", "LICENSE-MIT"):
            path = "licenses/javascript/vcpkg-0.2.15/" + name
            self.assertEqual(hashlib.sha256(original[path]).hexdigest(), record[path.replace('licenses/javascript/', 'bindings/javascript/third-party/')])

        with tempfile.TemporaryDirectory(prefix="hoyadb-git-provenance-") as scratch:
            for mode in ("input", "true", "false"):
                stage = Path(scratch) / mode
                stage.mkdir()
                env = os.environ.copy()
                env["GIT_INDEX_FILE"] = str(stage / "index")

                def git(*args):
                    return subprocess.check_output(
                        ["git", "-c", "core.autocrlf=" + mode, *args],
                        cwd=ROOT, env=env, stderr=subprocess.PIPE,
                    )

                git("read-tree", "HEAD")
                added = paths + [CONTROL]
                if (ROOT / ".gitattributes").is_file():
                    added.append(".gitattributes")
                git("add", "--", *added)
                destination = stage / "checkout"
                destination.mkdir()
                git("checkout-index", "--force", "--prefix=" + str(destination) + "/", "--", *paths, CONTROL)
                for path, expected in original.items():
                    with self.subTest(mode=mode, path=path, filter="clean"):
                        actual = git("cat-file", "blob", ":" + path)
                        self.assertEqual(hashlib.sha256(actual).hexdigest(), hashlib.sha256(expected).hexdigest())
                    with self.subTest(mode=mode, path=path, filter="checkout"):
                        actual = (destination / path).read_bytes()
                        self.assertEqual(hashlib.sha256(actual).hexdigest(), hashlib.sha256(expected).hexdigest())
                # Ordinary source must still use normal text conversion. A
                # global -text workaround would fail this negative control.
                self.assertEqual(git("cat-file", "blob", ":" + CONTROL), control)
                expected = control.replace(b"\n", b"\r\n") if mode == "true" else control
                self.assertEqual((destination / CONTROL).read_bytes(), expected)


if __name__ == "__main__":
    unittest.main(verbosity=2)
