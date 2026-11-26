"""Consumer guidance must survive the real JS builder and npm file allowlist.

Fixture WASM bytes are deliberately inert: this tests archive contents, not
runtime behavior. Everything is built and packed outside the working checkout.
"""
import hashlib
import json
import os
from pathlib import Path
import posixpath
import re
import shutil
import subprocess
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class AIGuidancePackagingTests(unittest.TestCase):
    def test_installed_guidance_is_complete_without_a_repository_checkout(self):
        with tempfile.TemporaryDirectory(prefix="hoyadb-ai-package-") as scratch:
            root = Path(scratch)
            scripts = root / "tools/javascript/scripts"
            scripts.mkdir(parents=True)
            shutil.copy2(ROOT / "tools/javascript/scripts/build.mjs", scripts)
            shutil.copy2(ROOT / "tools/javascript/scripts/freshness.mjs", scripts)
            shutil.copy2(ROOT / "tools/javascript/README.md", scripts.parent)
            shutil.copy2(ROOT / "LICENSE", root / "LICENSE")
            for name in ("client", "testing"):
                shutil.copytree(ROOT / "packages" / name, root / "packages" / name,
                                ignore=shutil.ignore_patterns("node_modules"))
            for name in ("hoyadb-web", "hoyadb-bindings", "hoyadb-core",
                         "hoyadb-storage-memory", "hoyadb-storage-sqlite"):
                (root / "crates" / name / "src").mkdir(parents=True)
            wasm = root / "crates/hoyadb-web/pkg"
            wasm.mkdir()
            # The lock predates the generated assets, as in a real build (packaging freshness guard).
            (wasm.parent / "Cargo.lock").write_text("# fixture\n")
            for name in ("hoyadb_web.js", "hoyadb_web_bg.wasm", "hoyadb_web.d.ts",
                         "hoyadb_web_bg.wasm.d.ts"):
                (wasm / name).write_bytes(b"inert packaging fixture\n")
            for name in ("sqlcipher", "javascript"):
                (root / "licenses" / name).mkdir(parents=True)

            assets = {relative: (ROOT / relative).read_text() for relative in (
                "AI.md", "skills/hoyadb-integration/SKILL.md",
            )}
            for relative, content in assets.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            env = dict(os.environ, npm_config_cache=str(root / "npm-cache"))
            built = subprocess.run(["node", str(scripts / "build.mjs")], cwd=root,
                                   env=env, text=True, capture_output=True, timeout=60)
            self.assertEqual(built.returncode, 0, built.stdout + built.stderr)
            for name in ("client", "testing"):
                with self.subTest(package=name):
                    package = scripts.parent / "dist" / name
                    packed = subprocess.run(
                        ["npm", "pack", "--offline", "--ignore-scripts", "--json",
                         "--pack-destination", str(root)], cwd=package, env=env,
                        text=True, capture_output=True, timeout=60,
                    )
                    self.assertEqual(packed.returncode, 0, packed.stdout + packed.stderr)
                    archive = root / json.loads(packed.stdout)[0]["filename"]
                    with tarfile.open(archive, "r:gz") as tar:
                        names = tar.getnames()
                        self.assertEqual(tar.extractfile("package/LICENSE").read(),
                                         (ROOT / "LICENSE").read_bytes())
                        provenance = json.load(tar.extractfile("package/SOURCE-MANIFEST.json"))
                        hashes = {item["path"]: item["sha256"] for item in provenance["sources"]}
                        for relative, content in assets.items():
                            member = "package/" + relative
                            self.assertIn(member, names, f"{name}: missing portable {relative}")
                            self.assertEqual(tar.extractfile(member).read(), content.encode())
                            self.assertEqual(hashes.get(relative), hashlib.sha256(content.encode()).hexdigest())
                            for target in re.findall(r"\[[^\]]*\]\(([^)]+)\)", content):
                                if target.startswith(("https://", "http://", "#")):
                                    continue
                                resolved = posixpath.normpath(posixpath.join(
                                    posixpath.dirname(member), target.split("#", 1)[0]))
                                self.assertTrue(resolved.startswith("package/"), target)
                                self.assertIn(resolved, names, f"broken packaged link: {target}")
                        manifest = json.load(tar.extractfile("package/package.json"))
                        original = json.loads((ROOT / "packages" / name / "package.json").read_text())
                        for field in ("exports", "dependencies", "private"):
                            self.assertEqual(manifest.get(field), original.get(field))


if __name__ == "__main__":
    unittest.main()
