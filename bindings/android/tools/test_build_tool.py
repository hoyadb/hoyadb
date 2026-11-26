#!/usr/bin/env python3
"""TEST_TOOLING_CASES 8: complete source closure; fake compiler activations only."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
SPEC = importlib.util.spec_from_file_location('build_tool_under_test', HERE / 'build.py')
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class SourceClosure(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='hoyadb-build-guard-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        android = self.root / 'bindings/android'
        for relative in ['src/main/kotlin/dev/hoyadb/Values.kt', 'src/main/java/dev/hoyadb/NativeBridge.java', 'tools/build.py']:
            path = android / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b'synthetic source; never compiled')
        self.tool = self.root / 'kotlinc'
        self.sdk = self.root / 'sdk'
        self.java = self.root / 'jdk/bin'
        closure = {}
        for name in ['bin/kotlinc', 'lib/kotlin-compiler.jar']:
            path = self.tool / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b'synthetic tool ' + name.encode())
            closure[name] = sha(path)
        for path in [self.sdk / 'platforms/android-37.0/android.jar', self.sdk / 'build-tools/37.0.0/d8',
                     self.sdk / 'build-tools/37.0.0/lib/d8.jar', self.java / 'java', self.java / 'javac']:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b'synthetic never-activated ' + path.name.encode())
        runtime = []
        for name in ['kotlin-stdlib-2.1.20.jar', 'annotations-13.0.jar']:
            path = self.root / 'deps' / name
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(b'synthetic runtime ' + name.encode())
            runtime.append({'coordinate': name, 'path': str(path), 'sha256': sha(path)})
        (android / 'toolchain-review.json').write_text(json.dumps({'installed_tool_closure': closure, 'runtime_artifacts': runtime}))
        self.out = self.root / 'target/build'

    def build(self, during_compile):
        def execute(command, **kwargs):
            output = Path(command[command.index('-d') + 1])
            (output / 'dev/hoyadb').mkdir(parents=True, exist_ok=True)
            (output / 'dev/hoyadb' / (Path(command[0]).name + '.class')).write_bytes(b'synthetic class')
            if Path(command[0]).name == 'kotlinc':
                during_compile()
            return subprocess.CompletedProcess(command, 0, b'', b'')
        with patch.object(BUILD, 'ROOT', self.root), patch.object(BUILD, 'TOOL', self.tool), \
                patch.object(BUILD, 'SDK', self.sdk), patch.object(BUILD, 'JAVA', self.java), \
                patch.dict(os.environ, {}, clear=False), \
                patch.object(sys, 'argv', ['build.py', '--out', str(self.out)]), \
                patch.object(subprocess, 'run', side_effect=execute):
            for key in ['JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS', 'JDK_JAVA_OPTIONS']:
                os.environ.pop(key, None)
            BUILD.main()

    def test_unchanged_synthetic_sources_record_complete_closure(self):
        self.build(lambda: None)
        manifest = json.loads((self.out / 'manifest.json').read_text())
        self.assertEqual(manifest['sources_before'], manifest['sources_after'])
        self.assertIn('bindings/android/src/main/kotlin/dev/hoyadb/Values.kt', manifest['sources_before'])

    def test_case8_source_added_during_compile_is_drift(self):
        added = self.root / 'bindings/android/src/main/kotlin/dev/hoyadb/Added.kt'
        with self.assertRaises(SystemExit) as failure:
            self.build(lambda: added.write_bytes(b'synthetic added source'))
        self.assertEqual(str(failure.exception), 'SDK source drift during build')
        self.assertFalse((self.out / 'manifest.json').exists())

    def test_case8_tool_script_added_during_compile_is_drift(self):
        added = self.root / 'bindings/android/tools/added.py'
        with self.assertRaises(SystemExit) as failure:
            self.build(lambda: added.write_bytes(b'synthetic added tool'))
        self.assertEqual(str(failure.exception), 'SDK source drift during build')
        self.assertFalse((self.out / 'manifest.json').exists())

    def test_case8_source_removed_during_compile_is_drift(self):
        removed = self.root / 'bindings/android/src/main/kotlin/dev/hoyadb/Values.kt'
        with self.assertRaises(SystemExit) as failure:
            self.build(removed.unlink)
        self.assertEqual(str(failure.exception), 'SDK source drift during build')
        self.assertFalse((self.out / 'manifest.json').exists())


if __name__ == '__main__':
    unittest.main()
