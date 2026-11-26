"""DEF-0060 / TC-0051: Apple build evidence observes local build inputs."""
import ast
import hashlib
from pathlib import Path
import tempfile
import unittest


BUILDER = Path(__file__).resolve().parents[2] / "bindings/swift/build-artifacts.py"
CRATES = ("hoyadb-core", "hoyadb-storage-memory", "hoyadb-storage-sqlite",
          "hoyadb-bindings", "hoyadb-native")


class AppleSourceTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="hoyadb-apple-sources-")
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name).resolve()
        for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
                     "crates/hoyadb-native/Cargo.lock",
                     "crates/hoyadb-native/include/hoyadb.h",
                     "bindings/swift/build-artifacts.py"):
            self.write(name, name)
        for name in CRATES:
            self.write(f"crates/{name}/Cargo.toml", "[package]\nname='fixture'\n")
            self.write(f"crates/{name}/src/lib.rs", "pub fn fixture() {}\n")
        # The builder intentionally runs its CLI at module scope. Compile the
        # real inventory function definitions without executing platform builds.
        source = ast.parse(BUILDER.read_text(), filename=str(BUILDER))
        definitions = [node for node in source.body if isinstance(node, ast.FunctionDef)]
        namespace = {"hashlib": hashlib, "Path": Path, "root": self.root,
                     "manifest": self.root / "crates/hoyadb-native/Cargo.toml",
                     "headers": self.root / "crates/hoyadb-native/include",
                     "__file__": str(self.root / "bindings/swift/build-artifacts.py")}
        exec(compile(ast.Module(body=definitions, type_ignores=[]), str(BUILDER), "exec"), namespace)
        self.sources = namespace["sources"]

    def write(self, name, data):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(data)

    def test_root_manifest_and_builder_changes_are_observed(self):
        for name in ("Cargo.toml", "bindings/swift/build-artifacts.py"):
            with self.subTest(name=name):
                before = self.sources()
                self.write(name, "changed")
                after = self.sources()
                self.assertNotEqual(before, after, f"unobserved build input: {name}")
                self.assertEqual(after[name], hashlib.sha256(b"changed").hexdigest())

    def test_build_scripts_and_source_resources_track_add_change_remove(self):
        for name in ("crates/hoyadb-core/build.rs",
                     "crates/hoyadb-native/src/fixture.bin"):
            with self.subTest(name=name):
                before = self.sources()
                self.write(name, "first")
                added = self.sources()
                self.assertNotEqual(before, added, f"unobserved new input: {name}")
                self.write(name, "second")
                changed = self.sources()
                self.assertNotEqual(added, changed)
                (self.root / name).unlink()
                self.assertEqual(self.sources(), before)

    def test_repository_cargo_config_is_observed(self):
        for name in (".cargo/config", ".cargo/config.toml",
                     "crates/hoyadb-native/.cargo/config.toml"):
            with self.subTest(name=name):
                before = self.sources()
                self.write(name, "[build]\njobs=2\n")
                self.assertNotEqual(before, self.sources(), f"unobserved config: {name}")

    def test_generated_outputs_and_credentials_are_excluded(self):
        before = self.sources()
        for name in ("target/debug/fixture", "crates/hoyadb-native/target/lib.a",
                     "bindings/swift/Artifacts/native-build.json", ".cargo/credentials.toml"):
            self.write(name, "generated-or-private")
        self.assertEqual(self.sources(), before)

    def test_missing_required_root_manifest_refuses_capture(self):
        (self.root / "Cargo.toml").unlink()
        with self.assertRaises(FileNotFoundError):
            self.sources()


if __name__ == "__main__":
    unittest.main()
