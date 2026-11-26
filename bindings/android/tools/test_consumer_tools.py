#!/usr/bin/env python3
"""Pure stdlib guard tests: no real compiler, adb, signing or foreign artifact activation."""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

HERE = Path(__file__).resolve().parent

def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module
BUILD = load('consumer_build_under_test', 'build-consumer.py')
RUN = load('consumer_run_under_test', 'run-consumer.py')

def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()

class ConsumerTools(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='hoyadb-tool-cases-')
        self.addCleanup(self.temp.cleanup); self.root = Path(self.temp.name).resolve()
        self.maven = self.root / 'maven'
        coordinate = self.maven / 'dev/hoyadb/hoyadb-android/0.1.0'; coordinate.mkdir(parents=True)
        self.aar = coordinate / 'hoyadb-android-0.1.0.aar'
        with zipfile.ZipFile(self.aar, 'w') as archive: archive.writestr('classes.jar', b'synthetic guard fixture; never executed')
        self.pom = self.aar.with_suffix('.pom'); self.pom.write_text('<project/>')
        self.aar.with_suffix('.aar.sha256').write_text(sha(self.aar)); self.pom.with_suffix('.pom.sha256').write_text(sha(self.pom))
        self.project = self.root / 'hoyadb-kotlin-consumer-tool-case'
        # Guard tests must reach their intended rejection without installed tools.
        self.checkout = self.root / 'checkout'
        android = self.checkout / 'bindings/android'; android.mkdir(parents=True)
        self.tool = self.root / 'synthetic-kotlinc'
        compiler = self.tool / 'bin/kotlinc'; compiler.parent.mkdir(parents=True)
        compiler.write_bytes(b'synthetic compiler; never executed')
        self.runtime = self.root / 'kotlin-stdlib-2.1.20.jar'
        self.runtime.write_bytes(b'synthetic runtime; never executed')
        (android / 'toolchain-review.json').write_text(json.dumps({
            'installed_tool_closure': {'bin/kotlinc': sha(compiler)},
            'runtime_artifacts': [{'path': str(self.runtime), 'sha256': sha(self.runtime)}]}))
    def build_guard(self, project=None, reason=None):
        with patch.object(BUILD, 'ROOT', self.checkout), patch.object(BUILD, 'TOOL', self.tool), \
                patch.object(sys, 'argv', ['build-consumer.py', '--maven-repo', str(self.maven), '--project', str(project or self.project)]), patch.object(subprocess, 'run') as execute:
            with self.assertRaises(SystemExit) as failure: BUILD.main()
            if reason is not None: self.assertEqual(str(failure.exception), reason)
            execute.assert_not_called()
    def test_c01_fresh_outside_only(self):
        self.build_guard(self.checkout / 'target/hoyadb-kotlin-consumer-invalid-inside')
        self.project.mkdir(); self.build_guard()
    def test_c02_bad_aar_stops_before_activation(self):
        self.aar.with_suffix('.aar.sha256').write_text('0' * 64); self.build_guard()
    def test_c02_bad_runtime_stops_before_activation(self):
        self.runtime.write_bytes(b'synthetic runtime drift')
        self.build_guard(reason='approved runtime changed')
    def test_c02_java_injection_stops_before_activation(self):
        with patch.dict(os.environ, {'JAVA_TOOL_OPTIONS': 'synthetic-unreviewed-injection'}): self.build_guard(reason='unreviewed Java injection')
    def test_c02_bad_compiler_stops_before_activation(self):
        (self.tool / 'bin/kotlinc').write_bytes(b'synthetic compiler drift')
        self.build_guard(reason='installed compiler closure changed')
    def runtime_fixture(self, app='dev.hoyadb.consumer.paaaaaaaaaaaaaaaa'):
        self.project.mkdir(); (self.project / 'consumer.apk').write_bytes(b'synthetic APK; mocked adb only')
        (self.project / 'expected-cases.json').write_bytes((HERE.parent / 'consumer/expected-cases.json').read_bytes())
        manifest = {'app_id': app, 'apk_sha256': sha(self.project / 'consumer.apk'), 'runtime_executed': False, 'installed': False,
            'templates_sha256': {'expected-cases.json': sha(self.project / 'expected-cases.json')}}
        (self.project / 'owned-consumer.json').write_text(json.dumps(manifest))
        return self.root / 'evidence'
    def run_guard(self, out, fail=None):
        commands = []
        def execute(command, **kwargs):
            commands.append(command)
            args = command[3:]
            stdout, code = b'', 0
            if args == ['shell', 'getprop', 'sys.boot_completed']: stdout = b'1\n'
            elif args == ['shell', 'getconf', 'PAGE_SIZE']: stdout = b'4096\n' if fail == 'page' else b'16384\n'
            elif args[:3] == ['shell', 'pm', 'path']: code = 1
            elif args[:1] == ['install']: code = 1 if fail == 'install' else 0
            elif args[:3] == ['shell', 'run-as', 'dev.hoyadb.consumer.paaaaaaaaaaaaaaaa']:
                mode = args[-1].split('results-')[-1].split('.')[0]
                partial = fail == 'partial'
                stdout = json.dumps({'mode': mode, 'failed': 0 if partial else 1,
                    'cases': [{'id': 'synthetic-shortened-case', 'ok': partial}],
                    'page_size': 16384, 'pid': 124 if mode == 'restart' else 123}).encode()
            elif args[:2] == ['shell', 'pidof']: code = 1
            return subprocess.CompletedProcess(command, code, stdout, b'')
        with patch.object(sys, 'argv', ['run-consumer.py', '--project', str(self.project), '--out', str(out), '--uninstall-after']), patch.object(subprocess, 'run', side_effect=execute):
            with self.assertRaises(SystemExit) as failure: RUN.main()
            if fail == 'partial': self.assertEqual(str(failure.exception), 'consumer result matrix mismatch')
        return commands
    def test_c03_wrong_page_persists_failure_before_install(self):
        out = self.runtime_fixture(); commands = self.run_guard(out, 'page')
        self.assertFalse(any(command[3:4] == ['install'] for command in commands))
        evidence = json.loads((out / 'runtime-manifest.json').read_text())
        self.assertFalse(evidence['artifact']['runtime_executed']); self.assertTrue(evidence['commands'])
    def test_c03_install_failure_persists_commands_and_setup_failure(self):
        out = self.runtime_fixture(); self.run_guard(out, 'install')
        evidence = json.loads((out / 'runtime-manifest.json').read_text())
        self.assertTrue(any(command[3:4] == ['install'] for command in evidence['commands']))
        self.assertFalse(evidence['artifact']['runtime_executed'])
    def test_c04_failed_fixture_preserves_owned_app_and_report(self):
        out = self.runtime_fixture(); commands = self.run_guard(out, 'behavior')
        self.assertFalse(any(command[3:4] == ['uninstall'] for command in commands))
        self.assertTrue((out / 'full.json').is_file())
        evidence = json.loads((out / 'runtime-manifest.json').read_text())
        self.assertTrue(evidence['artifact']['installed']); self.assertFalse(evidence['artifact']['runtime_executed'])
    def test_c03_foreign_app_identity_rejects_without_adb(self):
        out = self.runtime_fixture('com.synthetic.foreign')
        commands = self.run_guard(out)
        self.assertEqual(commands, [])
    def test_c05_shortened_all_pass_matrix_cannot_certify_or_uninstall(self):
        out = self.runtime_fixture(); commands = self.run_guard(out, 'partial')
        self.assertFalse(any(command[3:4] == ['uninstall'] for command in commands))
        evidence = json.loads((out / 'runtime-manifest.json').read_text())
        self.assertFalse(evidence['artifact']['runtime_executed'])

if __name__ == '__main__': unittest.main()
