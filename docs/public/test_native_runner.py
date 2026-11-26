"""SPEC-0043 / TC-0047: run_native.py only writes inside target/documentation-languages.

Pure path checks: no compiler, Swift, native build or JVM. Each case runs in a bounded
subprocess so a path check that never returns fails instead of hanging the suite.
"""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent

PROBE = '''
import json, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
import run_native
root = Path(sys.argv[2])
run_native.REPO = root
run_native.OUT = root / 'target/documentation-languages'
try:
    run_native.require_inside(Path(sys.argv[3]))
    print(json.dumps('accepted'))
except SystemExit as error:
    print(json.dumps(str(error)))
'''


class RequireInside(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.out = self.root / 'target/documentation-languages'
        (self.out / 'native').mkdir(parents=True)
        (self.root / 'elsewhere').mkdir()
        (self.out / 'link').symlink_to(self.root / 'elsewhere', target_is_directory=True)

    def tearDown(self):
        self.temporary.cleanup()

    def check(self, path):
        try:
            result = subprocess.run([sys.executable, '-I', '-c', PROBE, str(HERE), str(self.root), str(path)],
                                    capture_output=True, text=True, timeout=10)
        except subprocess.TimeoutExpired:
            self.fail(f'require_inside did not return for {path}')
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)

    def test_accepts_task_owned_paths(self):
        self.assertEqual(self.check(self.out / 'native'), 'accepted')
        self.assertEqual(self.check(self.out / 'swift-guide/Package.swift'), 'accepted')

    def test_rejects_paths_outside_the_task_directory(self):
        for path in [Path('/tmp/hoyadb-outside'), self.root / 'bindings/swift/Artifacts/HoyaNative.xcframework',
                     self.root / 'target/other', self.out / '../escape', Path('relative/path')]:
            with self.subTest(path=path):
                self.assertIn('escapes', self.check(path))

    def test_rejects_symlinks_on_the_way(self):
        for path in [self.out / 'link', self.out / 'link/child']:
            with self.subTest(path=path):
                self.assertIn('symlink', self.check(path))


class NativeProvenance(unittest.TestCase):
    def test_archived_native_path_uses_the_commits_layout(self):
        from unittest.mock import patch
        import run_native
        for present, expected in [
            ({"crates/hoyadb-native"}, "crates/hoyadb-native"),
            ({"bindings/native"}, "bindings/native"),
        ]:
            with self.subTest(layout=expected):
                def probe(args, **kwargs):
                    path = args[-1].split(":", 1)[1].removesuffix("/Cargo.toml")
                    return subprocess.CompletedProcess(args, 0 if path in present else 1)
                with patch.object(run_native.subprocess, "run", side_effect=probe):
                    self.assertEqual(run_native.archived_native_path(Path("/fixture"), "commit"), expected)
        with patch.object(run_native.subprocess, "run", return_value=subprocess.CompletedProcess([], 1)):
            with self.assertRaisesRegex(SystemExit, "native.*commit"):
                run_native.archived_native_path(Path("/fixture"), "commit")

    def test_swift_compile_inputs_cannot_drift_after_native_build(self):
        import hashlib
        from unittest.mock import patch
        import run_native
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            out = root/'target/documentation-languages'
            framework = out/'swift-package/Artifacts/HoyaNative.xcframework'
            framework.mkdir(parents=True)
            swift = out/'swift-package/Sources/HoyaDB/Types.swift'
            swift.parent.mkdir(parents=True)
            swift.write_text('public struct Row {}')
            lib = framework/'lib.a'
            lib.write_bytes(b'fixture native library')
            digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
            package = out/'swift-package/Package.swift'
            package.write_text('// fixture package')
            manifest = out/'native-build.json'
            manifest.write_text(json.dumps({'artifacts':{lib.relative_to(root).as_posix():digest(lib)},
                                           'swift_inputs':{swift.relative_to(root).as_posix():digest(swift),package.relative_to(root).as_posix():digest(package)}}))
            with patch.multiple(run_native, REPO=root, OUT=out, MANIFEST=manifest, XCFRAMEWORK=framework, PACKAGE=out/'swift-package'):
                run_native.built_artifacts()
                swift.write_text('public struct InventedRow {}')
                with self.assertRaisesRegex(SystemExit, 'differs'):
                    run_native.built_artifacts()
                swift.write_text('public struct Row {}')
                extra = swift.parent/'Extra.swift'
                extra.write_text('public struct Extra {}')
                with self.subTest(mutation='added file'):
                    with self.assertRaisesRegex(SystemExit, 'unrecorded|differs'):
                        run_native.built_artifacts()
                extra.unlink()
                report = json.loads(manifest.read_text())
                report.pop('swift_inputs')
                manifest.write_text(json.dumps(report))
                with self.subTest(mutation='old manifest lacks inputs'):
                    with self.assertRaisesRegex(SystemExit, 'swift_inputs'):
                        run_native.built_artifacts()


if __name__ == '__main__':
    unittest.main()
