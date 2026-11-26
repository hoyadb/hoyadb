#!/usr/bin/env python3
"""Reviewed split-runtime guards, no compiler/DEX/device activation."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('test_runtime_cache_under_test', HERE / 'runtime_cache.py')
CACHE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CACHE)


class RuntimePlanTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='hoyadb-runtime-plan-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.inputs = {}
        for name in ['kotlin-stdlib-2.1.20.jar', 'annotations-13.0.jar', 'android.jar', 'd8', 'd8.jar', 'java']:
            path = self.root / name
            path.write_bytes(('synthetic never-activated ' + name).encode())
            self.inputs[name] = path

    def plan(self):
        return CACHE.runtime_plan(self.inputs, min_api=24, profile='test-runtime-v1')

    def test_reviewed_exact_runtime_only_plan_and_changed_input_identity(self):
        first = self.plan()
        self.assertEqual(first['program_inputs'], ['kotlin-stdlib-2.1.20.jar', 'annotations-13.0.jar'])
        self.assertEqual(first['min_api'], 24)
        self.assertEqual(first['profile'], 'test-runtime-v1')
        self.assertEqual(first['d8_flags'], ['--lib', 'android.jar', '--min-api', '24'])
        for name, path in self.inputs.items():
            original = path.read_bytes()
            path.write_bytes(original + b'changed')
            self.assertNotEqual(first['identity'], self.plan()['identity'], name)
            path.write_bytes(original)

    def test_unsupported_minimum_profile_or_incomplete_input_rejects(self):
        for minimum, profile in [(23, 'test-runtime-v1'), (24, 'unsupported')]:
            with self.assertRaises(SystemExit):
                CACHE.runtime_plan(self.inputs, min_api=minimum, profile=profile)
        missing = dict(self.inputs); del missing['java']
        with self.assertRaises(SystemExit):
            CACHE.runtime_plan(missing, min_api=24, profile='test-runtime-v1')

    def valid_cache(self):
        plan = self.plan()
        directory = self.root / 'cache' / plan['identity']
        directory.mkdir(parents=True)
        dex = directory / 'classes.dex'; dex.write_bytes(b'synthetic unexecuted DEX')
        manifest = {'plan': plan, 'dex_sha256': hashlib.sha256(dex.read_bytes()).hexdigest()}
        (directory / 'manifest.json').write_text(json.dumps(manifest))
        return plan, directory, manifest

    def test_valid_cache_checks_hash_and_exact_metadata(self):
        plan, directory, manifest = self.valid_cache()
        self.assertEqual(CACHE.validate_cache(directory, plan), directory / 'classes.dex')
        (directory / 'classes.dex').write_bytes(b'changed')
        with self.assertRaises(SystemExit): CACHE.validate_cache(directory, plan)
        (directory / 'classes.dex').write_bytes(b'synthetic unexecuted DEX')
        manifest['extra'] = True
        (directory / 'manifest.json').write_text(json.dumps(manifest))
        with self.assertRaises(SystemExit): CACHE.validate_cache(directory, plan)

    def test_incomplete_symlink_and_wrong_identity_cache_reject(self):
        plan, directory, manifest = self.valid_cache()
        (directory / 'manifest.json').unlink()
        with self.assertRaises(SystemExit): CACHE.validate_cache(directory, plan)
        (directory / 'manifest.json').write_text(json.dumps(manifest))
        alias = self.root / 'alias'; alias.symlink_to(directory, target_is_directory=True)
        with self.assertRaises(SystemExit): CACHE.validate_cache(alias, plan)
        manifest['plan'] = dict(plan, identity='0' * 64)
        (directory / 'manifest.json').write_text(json.dumps(manifest))
        with self.assertRaises(SystemExit): CACHE.validate_cache(directory, plan)

    def test_fresh_outputs_never_reuse_classes(self):
        output = self.root / 'fresh'
        CACHE.fresh_output(output)
        self.assertTrue(output.is_dir())
        with self.assertRaises(SystemExit): CACHE.fresh_output(output)


if __name__ == '__main__':
    unittest.main()
