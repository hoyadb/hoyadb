"""SPEC-0050 / TC-0054: delivery gates fail closed before external writes."""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load_gate():
    spec = importlib.util.spec_from_file_location("delivery", ROOT / "tools/ci/delivery.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class DeliveryTests(unittest.TestCase):
    def setUp(self):
        self.gate = load_gate()

    def test_aggregate_rejects_every_nonsuccess_and_empty_or_malformed_input(self):
        self.gate.check_results({"rust": {"result": "success"}, "js": {"result": "success"}})
        for value in ({}, None, [], {"rust": {}}, {"rust": "success"},
                      *({"rust": {"result": result}} for result in
                        ["failure", "cancelled", "skipped", None])):
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.gate.check_results(value)

    def fixture(self, root):
        (root / "LICENSE").write_text("MIT License\n\nPermission is hereby granted, free of charge\n")
        for name in ("client", "testing", "react-native"):
            path = root / "packages" / name
            path.mkdir(parents=True)
            manifest = {"name": "@hoyadb/" + name, "version": "0.1.0", "license": "MIT",
                        "private": False, "repository": {"type": "git", "url": "https://github.com/example/hoyadb.git"},
                        "publishConfig": {"access": "public"}}
            if name == "testing":
                manifest["dependencies"] = {"@hoyadb/client": "0.1.0"}
            if name == "react-native":
                manifest["peerDependencies"] = {"@hoyadb/client": "0.1.0"}
            (path / "package.json").write_text(json.dumps(manifest))

    def test_release_tag_and_all_package_metadata_must_agree(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            self.assertEqual(self.gate.check_release(root, "v0.1.0", "example/hoyadb"), "0.1.0")
            for tag in ("main", "v0.1.0-beta.1", "v00.1.0", "v0.1.1", "v0.1.0;echo bad"):
                with self.subTest(tag=tag), self.assertRaises(ValueError):
                    self.gate.check_release(root, tag, "example/hoyadb")
            path = root / "packages/react-native/package.json"
            original = json.loads(path.read_text())
            for field, value in (("private", True), ("license", "UNLICENSED"),
                                 ("version", "0.2.0"), ("repository", {}),
                                 ("peerDependencies", {"@hoyadb/client": "0.2.0"}),
                                 ("publishConfig", {"access": "restricted"})):
                changed = copy.deepcopy(original)
                changed[field] = value
                path.write_text(json.dumps(changed))
                with self.subTest(field=field), self.assertRaises(ValueError):
                    self.gate.check_release(root, "v0.1.0", "example/hoyadb")
            path.write_text(json.dumps(original))
            (root / "LICENSE").unlink()
            with self.assertRaises(ValueError):
                self.gate.check_release(root, "v0.1.0", "example/hoyadb")

    def test_cli_missing_results_does_not_succeed(self):
        result = subprocess.run([sys.executable, str(ROOT / "tools/ci/delivery.py"), "results"],
                                capture_output=True, text=True, env={})
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
