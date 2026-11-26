#!/usr/bin/env python3
"""DEF-0028 permanent regressions for prepare_repo.py (TC-0036).

Offline: synthetic fake bytes in fresh temporary roots; no JAR, Gradle, compiler, network, ignored cache or
shared input. Every case runs the actual tool as a subprocess both normally (-I) and optimized (-I -O).
Run:  python3 -I packages/react-native/tools/android/test_prepare_repo.py
      python3 -I -O packages/react-native/tools/android/test_prepare_repo.py
"""
import ast
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
TOOL = HERE / 'prepare_repo.py'
V1 = HERE / 'testdata' / 'repository-assertion-control.py.txt'
V1_SHA256 = '7725f78c4ac91ae72a6111a253898a941ca9aa8ff9b065fa7738feb9e3f62ab2'
MODES = {'normal': ['-I'], 'optimized': ['-I', '-O']}


def sha(data, algorithm='sha256'):
    return hashlib.new(algorithm, data).hexdigest()


class Fixture:
    """A complete synthetic input plan: two binaries and two metadata files in two read-only roots."""

    def __init__(self, base):
        self.base = Path(base)
        self.roots = {'bin': self.base / 'inputs-bin', 'meta': self.base / 'inputs-meta'}
        for root in self.roots.values():
            root.mkdir(parents=True)
        self.files = {
            ('bin', 'a.jar'): b'fake-binary-a\x00\x01\n',
            ('bin', 'nested/b.aar'): b'fake-binary-b\n',
            ('meta', 'a.pom'): b'<project>fake a</project>\n',
            ('meta', 'b.module'): b'{"fake": "b"}\n',
        }
        for (root, rel), data in self.files.items():
            target = self.roots[root] / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        self.entries = [
            self.entry('com/example/a/1.0/a-1.0.jar', 'binary', 'bin', 'a.jar'),
            self.entry('com/example/b/1.0/b-1.0.aar', 'binary', 'bin', 'nested/b.aar'),
            self.entry('com/example/a/1.0/a-1.0.pom', 'metadata', 'meta', 'a.pom'),
            self.entry('com/example/b/1.0/b-1.0.module', 'metadata', 'meta', 'b.module'),
        ]

    def entry(self, path, kind, root, rel, identity=None):
        data = self.files[(root, rel)]
        return {'path': path, 'kind': kind, 'source': {'root': root, 'path': rel}, 'size': len(data),
                'sha256': sha(data), 'sha512': sha(data, 'sha512'),
                'provenance': {'url': 'https://repo.example.invalid/' + path, 'identity': identity or ('fixture:' + path)}}

    def write_plan(self, name='plan.json', entries=None):
        plan = {'format': 1, 'roots': {k: str(v) for k, v in self.roots.items()}, 'entries': entries or self.entries}
        path = self.base / name
        path.write_text(json.dumps(plan, indent=2) + '\n')
        return path

    def expected_inventory(self):
        return {e['path']: e['sha256'] for e in self.entries}


def run_tool(mode, command, plan, out, record):
    argv = [sys.executable, *MODES[mode], str(TOOL), command, '--inputs', str(plan), '--out', str(out), '--record', str(record)]
    p = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=120, cwd=tempfile.gettempdir())
    return p.returncode, p.stdout


def tree(root):
    root = Path(root)
    return {str(p.relative_to(root)): p.read_bytes() for p in sorted(root.rglob('*')) if p.is_file()}


class Base(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix='hoya-def28-'))
        self.addCleanup(shutil.rmtree, self.tmp, True)

    def fixture(self, name):
        return Fixture(self.tmp / name)

    def assert_rejected(self, mode, fx, plan, reason, command='assemble', out=None, record=None):
        out = out or fx.base / 'repo'
        record = record or fx.base / 'record.json'
        code, output = run_tool(mode, command, plan, out, record)
        self.assertNotEqual(code, 0, f'{mode}: expected rejection {reason}, got success: {output}')
        self.assertIn(f'REJECTED {reason}:', output, f'{mode}: wrong rejection: {output}')
        if command == 'assemble':
            self.assertFalse(record.exists(), f'{mode}: success record written despite rejection')
            self.assertFalse(out.exists(), f'{mode}: repository published despite rejection')
        return output


class R01CompleteAssembly(Base):
    def test_normal_and_optimized_produce_identical_complete_repositories(self):
        # One read-only input plan; each mode writes into its own fresh owned output root.
        fx = self.fixture('inputs')
        plan = fx.write_plan()
        results = {}
        for mode in MODES:
            out_root = self.tmp / ('out-' + mode)
            out_root.mkdir()
            code, output = run_tool(mode, 'assemble', plan, out_root / 'repo', out_root / 'record.json')
            self.assertEqual(code, 0, f'{mode}: {output}')
            inventory = tree(out_root / 'repo')
            self.assertEqual({k: sha(v) for k, v in inventory.items()}, fx.expected_inventory(), mode)
            self.assertEqual(sorted(p.name for p in out_root.iterdir()), ['record.json', 'repo'], f'{mode}: leftover partial output')
            record_bytes = (out_root / 'record.json').read_bytes()
            record = json.loads(record_bytes)
            self.assertEqual(record['inventory'], fx.expected_inventory(), mode)
            self.assertEqual(record['counts'], {'binary': 2, 'metadata': 2}, mode)
            self.assertEqual(record['plan_sha256'], sha(plan.read_bytes()), mode)
            code, output = run_tool(mode, 'verify', plan, out_root / 'repo', out_root / 'record.json')
            self.assertEqual(code, 0, f'{mode} verify: {output}')
            results[mode] = (inventory, record_bytes)
        self.assertEqual(results['normal'][0], results['optimized'][0], 'repository bytes differ between modes')
        self.assertEqual(results['normal'][1], results['optimized'][1], 'success records differ between modes')


class R02CorruptedInput(Base):
    def corrupt(self, root, rel):
        for mode in MODES:
            fx = self.fixture(mode + '-' + root)
            plan = fx.write_plan()
            (fx.roots[root] / rel).write_bytes(b'corrupted bytes\n')
            self.assert_rejected(mode, fx, plan, 'integrity')

    def test_corrupted_binary_rejected_before_success_record(self):
        self.corrupt('bin', 'a.jar')

    def test_corrupted_metadata_rejected_before_success_record(self):
        self.corrupt('meta', 'a.pom')


class R02bMissingInput(Base):
    def remove(self, root, rel):
        for mode in MODES:
            fx = self.fixture(mode + '-' + root)
            plan = fx.write_plan()
            (fx.roots[root] / rel).unlink()
            self.assert_rejected(mode, fx, plan, 'missing-input')

    def test_missing_binary_rejected_locally(self):
        self.remove('bin', 'nested/b.aar')

    def test_missing_metadata_rejected_locally(self):
        self.remove('meta', 'b.module')

    def test_tool_has_no_network_or_subprocess_capability(self):
        tree_ = ast.parse(TOOL.read_text())
        imported = {alias.name.split('.')[0] for node in ast.walk(tree_) if isinstance(node, (ast.Import, ast.ImportFrom))
                    for alias in node.names} | {node.module.split('.')[0] for node in ast.walk(tree_) if isinstance(node, ast.ImportFrom) and node.module}
        self.assertFalse(imported & {'subprocess', 'socket', 'urllib', 'http', 'ftplib', 'ssl', 'requests'}, imported)


class R03Conflicts(Base):
    def test_conflicting_provenance_identity_for_same_path_rejected(self):
        for mode in MODES:
            fx = self.fixture(mode)
            twin = dict(fx.entries[2], provenance={'url': fx.entries[2]['provenance']['url'], 'identity': 'other-approval-record'})
            plan = fx.write_plan(entries=fx.entries + [twin])
            self.assert_rejected(mode, fx, plan, 'provenance-conflict')

    def test_conflicting_bytes_for_same_destination_rejected(self):
        for mode in MODES:
            fx = self.fixture(mode)
            other = fx.entry(fx.entries[0]['path'], 'binary', 'bin', 'nested/b.aar', identity=fx.entries[0]['provenance']['identity'])
            other['provenance'] = dict(fx.entries[0]['provenance'])
            plan = fx.write_plan(entries=fx.entries + [other])
            self.assert_rejected(mode, fx, plan, 'byte-conflict')


class R04ConsumerVerification(Base):
    def assembled(self, mode, name=None):
        fx = self.fixture(name or mode)
        plan = fx.write_plan()
        code, output = run_tool(mode, 'assemble', plan, fx.base / 'repo', fx.base / 'record.json')
        self.assertEqual(code, 0, output)
        return fx, plan

    def test_verification_rejects_missing_stale_tampered_records_and_mutations(self):
        for mode in MODES:
            cases = {
                'missing-record': lambda fx, plan: (fx.base / 'record.json').unlink(),
                'stale-record': lambda fx, plan: plan.write_text(plan.read_text().replace('fixture:com/example/a/1.0/a-1.0.jar', 'fixture:changed')),
                'record-mismatch': lambda fx, plan: (fx.base / 'record.json').write_text(
                    (fx.base / 'record.json').read_text().replace(fx.entries[0]['sha256'], '0' * 64)),
                'unexpected-file': lambda fx, plan: (fx.base / 'repo/extra.jar').write_bytes(b'extra'),
                'missing-file': lambda fx, plan: (fx.base / 'repo' / fx.entries[1]['path']).unlink(),
                'integrity': lambda fx, plan: (fx.base / 'repo' / fx.entries[2]['path']).write_bytes(b'mutated\n'),
                'missing-repository': lambda fx, plan: shutil.rmtree(fx.base / 'repo'),
            }
            for reason, mutate in cases.items():
                with self.subTest(mode=mode, reason=reason):
                    fx, plan = self.assembled(mode, mode + '-' + reason)
                    mutate(fx, plan)
                    self.assert_rejected(mode, fx, plan, reason, command='verify')

    def test_partial_output_is_not_a_repository(self):
        for mode in MODES:
            fx, plan = self.assembled(mode)
            partial = fx.base / 'repo.partial-manual'
            shutil.copytree(fx.base / 'repo', partial)
            (partial / fx.entries[3]['path']).unlink()
            self.assert_rejected(mode, fx, plan, 'missing-file', command='verify', out=partial)


class PathSafety(Base):
    def test_unsafe_output_and_source_paths_rejected(self):
        for mode in MODES:
            for label, change in {
                'absolute-output': lambda e: e.__setitem__('path', '/tmp/escape.jar'),
                'escaping-output': lambda e: e.__setitem__('path', 'com/../../escape.jar'),
                'backslash-output': lambda e: e.__setitem__('path', 'com\\example\\a.jar'),
                'escaping-source': lambda e: e['source'].__setitem__('path', '../inputs-meta/a.pom'),
            }.items():
                with self.subTest(mode=mode, case=label):
                    fx = self.fixture(mode + '-' + label)
                    entries = json.loads(json.dumps(fx.entries))
                    change(entries[0])
                    self.assert_rejected(mode, fx, fx.write_plan(entries=entries), 'unsafe-path')

    def test_source_symlink_leaving_its_root_rejected(self):
        for mode in MODES:
            fx = self.fixture(mode)
            outside = fx.base / 'outside.jar'
            outside.write_bytes(fx.files[('bin', 'a.jar')])
            (fx.roots['bin'] / 'a.jar').unlink()
            os.symlink(outside, fx.roots['bin'] / 'a.jar')
            self.assert_rejected(mode, fx, fx.write_plan(), 'unsafe-path')


class StaticGuarantees(unittest.TestCase):
    def test_tool_uses_no_assert_statements(self):
        self.assertFalse([n.lineno for n in ast.walk(ast.parse(TOOL.read_text())) if isinstance(n, ast.Assert)])

    def test_import_has_no_side_effects(self):
        with tempfile.TemporaryDirectory() as cwd:
            code = ('import importlib.util, sys; s = importlib.util.spec_from_file_location("prepare_repo", sys.argv[1]); '
                    'm = importlib.util.module_from_spec(s); s.loader.exec_module(m)')
            p = subprocess.run([sys.executable, '-I', '-c', code, str(TOOL)], cwd=cwd, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, text=True, timeout=60)
            self.assertEqual((p.returncode, p.stdout, os.listdir(cwd)), (0, '', []))


class V1MechanismReplay(Base):
    """Unsafe assert/setdefault control, using synthetic input paths only.

    The path-only rewrite retains the assertion mechanism and expected normal/
    optimized behavior. It is not an exact historical incident replay.
    """

    def stage(self, name, corrupt=False):
        root = self.tmp / name
        android = root / 'target/phase-seven-rn/android'
        android.mkdir(parents=True)
        script_bytes = V1.read_bytes()
        self.assertEqual(sha(script_bytes), V1_SHA256, 'frozen v1 copy changed')
        (android / 'prepare-repo.py').write_bytes(script_bytes)
        binary = b'fake-v1-binary\n'
        metadata = b'<project>fake v1</project>\n'
        (root / 'target/inputs').mkdir(parents=True)
        (root / 'target/inputs/a-1.0.jar').write_bytes(b'corrupted\n' if corrupt else binary)
        index = {'artifacts': [{'coordinate': 'com.example:a:1.0', 'name': 'a-1.0.jar', 'configurations': ['androidDebug'],
                                'sha256': sha(binary), 'sha512': sha(binary, 'sha512'), 'qualified_url': 'https://repo.example.invalid/a-1.0.jar'}]}
        (root / 'tests/fixtures/repository-inputs/native').mkdir(parents=True)
        (root / 'tests/fixtures/repository-inputs/native/artifact-index.json').write_text(json.dumps(index))
        (root / 'target/phase-seven-rn/maven').mkdir(parents=True)
        (root / 'target/phase-seven-rn/maven/verified.json').write_text(json.dumps(
            {'verified': [{'coordinate': 'com.example:a:1.0', 'path': 'target/inputs/a-1.0.jar'}]}))
        review = root / 'target/phase-seven-dependencies/native-source-review/maven'
        (review / 'com/example/a/1.0').mkdir(parents=True)
        (review / 'com/example/a/1.0/a-1.0.pom').write_bytes(metadata)
        listing = [{'url': 'https://repo.example.invalid/a-1.0.pom', 'file': 'com/example/a/1.0/a-1.0.pom', 'sha256': sha(metadata)}]
        for name_ in ['metadata-provenance.json', 'selected-metadata-provenance.json', 'selected-patched-metadata-provenance.json']:
            (review / name_).write_text(json.dumps(listing))
        (root / 'tests/fixtures/repository-addendum').mkdir(parents=True)
        (root / 'tests/fixtures/repository-addendum/bundle.json').write_text(json.dumps({'additional_artifacts': []}))
        (root / 'tests/fixtures/repository-addendum/repository-responses.json').write_text('[]')
        (root / 'target/phase-seven-plugin-addendum/archive-review').mkdir(parents=True)
        return android

    def replay(self, mode, corrupt=False):
        android = self.stage(f'v1-{mode}-{corrupt}', corrupt)
        p = subprocess.run([sys.executable, *MODES[mode], 'prepare-repo.py'], cwd=android, stdout=subprocess.PIPE,
                           stderr=subprocess.STDOUT, text=True, timeout=120)
        counts = json.loads((android / 'prepare-repo.json').read_text())['counts'] if (android / 'prepare-repo.json').exists() else None
        return p.returncode, p.stdout, counts

    def test_v1_optimized_run_loses_metadata_collection(self):
        normal, optimized = self.replay('normal'), self.replay('optimized')
        print(f'\nV1 replay (sha256 {V1_SHA256[:12]}…): normal counts={normal[2]} optimized counts={optimized[2]}')
        self.assertEqual((normal[0], normal[2]['metadata']), (0, 1), normal[1])
        self.assertEqual((optimized[0], optimized[2]['metadata']), (0, 0), optimized[1])

    def test_v1_optimized_run_accepts_corrupted_binary(self):
        normal, optimized = self.replay('normal', corrupt=True), self.replay('optimized', corrupt=True)
        print(f'\nV1 corrupted replay: normal exit={normal[0]} optimized exit={optimized[0]} counts={optimized[2]}')
        self.assertNotEqual(normal[0], 0)
        self.assertIn('sha256 mismatch', normal[1])
        self.assertEqual((optimized[0], optimized[2]['binaries']), (0, 1), optimized[1])


if __name__ == '__main__':
    print(f'python optimize flag: {sys.flags.optimize}')
    unittest.main(verbosity=2)
