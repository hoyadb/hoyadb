"""Synthetic installer/closure tests; no browser or performance measurements."""
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from browser_run import accepted_samples, arguments, control_loop, package_footprint, prepare_application, snapshot, verify_snapshot, release_reference


class BrowserInstallerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='hoya-browser-installer-test-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.repo, self.app, self.output = [self.root / value for value in ('repo', 'app', 'output')]
        for path in (self.repo, self.app, self.output):
            path.mkdir()
        for name in ('client', 'testing'):
            path = self.repo / 'tools/javascript/dist' / name
            path.mkdir(parents=True)
            (path / 'package.json').write_text(json.dumps({'name': '@hoyadb/' + name, 'version': '0.1.0'}))
        (self.repo / 'tools/javascript/benchmark').mkdir(parents=True)
        (self.repo / 'tools/javascript/benchmark/browser.html').write_text('application')
        (self.repo / 'tools/javascript/benchmark/browser.mjs').write_text("import '@hoyadb/client';")
        controls = self.repo / 'tests/packaging/javascript'
        controls.mkdir(parents=True)
        (controls / 'control-client.mjs').write_text("import '../../../packages/client/src/index.js';")
        (controls / 'control-worker.mjs').write_text("import '../../../crates/hoyadb-web/pkg-test/hoyadb_web.js'; import '../../../packages/client/src/worker-host.js';")
        (self.repo / 'crates/hoyadb-web/pkg-test').mkdir(parents=True)
        (self.repo / 'crates/hoyadb-web/pkg-test/hoyadb_web_bg.wasm').write_bytes(b'instrumented')
        self.calls = []

    def command(self, args, cwd):
        self.calls.append((args, Path(cwd)))
        if args[1] == 'pack':
            name = Path(cwd).name
            filename = 'hoyadb-' + name + '-0.1.0.tgz'
            archive = Path(args[args.index('--pack-destination') + 1]) / filename
            archive.write_bytes(('archive:' + name).encode())
            return json.dumps([{'filename': filename}])
        self.assertEqual(args[:2], ['npm', 'install'])
        self.assertIn('--offline', args)
        self.assertIn('--ignore-scripts', args)
        packages = {'': {}, 'node_modules/@hoyadb/client': {}, 'node_modules/@hoyadb/testing': {}}
        (Path(cwd) / 'package-lock.json').write_text(json.dumps({'packages': packages}))
        return 'installed'

    def test_installs_only_local_archives_and_copies_isolated_actual_application(self):
        result = prepare_application(self.repo, self.app, self.output, self.command)
        self.assertEqual(len(self.calls), 3)
        self.assertTrue(all('--ignore-scripts' in args for args, _ in self.calls))
        self.assertEqual(self.calls[-1][1], self.app.resolve())
        self.assertEqual((self.app / 'browser.html').read_text(), 'application')
        self.assertIn("'@hoyadb/client/host'", (self.app / 'control-client.mjs').read_text())
        self.assertIn("'./node_modules/@hoyadb/client/src/worker-host.js'", (self.app / 'control-worker.mjs').read_text())
        self.assertEqual((self.app / 'test-wasm/hoyadb_web_bg.wasm').read_bytes(), b'instrumented')
        self.assertEqual(len(result['archives']), 2)
        for record in result['archives']:
            self.assertEqual(hashlib.sha256(Path(record['path']).read_bytes()).hexdigest(), record['sha256'])

    def test_reused_application_root_rejects_before_install_or_overwrite(self):
        (self.app / 'prior').write_text('preserve')
        with self.assertRaises(ValueError):
            prepare_application(self.repo, self.app, self.output, self.command)
        self.assertEqual(self.calls, [])
        self.assertEqual((self.app / 'prior').read_text(), 'preserve')

    def test_unexpected_transitive_package_is_rejected_even_after_offline_install(self):
        def extra(args, cwd):
            result = self.command(args, cwd)
            if args[1] == 'install':
                path = Path(cwd) / 'package-lock.json'
                value = json.loads(path.read_text())
                value['packages']['node_modules/unreviewed'] = {}
                path.write_text(json.dumps(value))
            return result
        with self.assertRaises(ValueError):
            prepare_application(self.repo, self.app, self.output, extra)

    def test_source_snapshot_is_ordered_and_changed_or_missing_files_fail_closed(self):
        (self.repo / 'a').write_bytes(b'first')
        (self.repo / 'b').write_bytes(b'second')
        before = snapshot(self.repo, ['b', 'a'])
        self.assertEqual(list(before['files']), ['a', 'b'])
        self.assertEqual(verify_snapshot(self.repo, before), before)
        (self.repo / 'a').write_bytes(b'changed')
        with self.assertRaises(ValueError):
            verify_snapshot(self.repo, before)
        (self.repo / 'a').unlink()
        with self.assertRaises(ValueError):
            verify_snapshot(self.repo, before)

    def test_symlinked_inputs_do_not_escape_the_frozen_source_inventory(self):
        (self.repo / 'link').symlink_to(self.root / 'output', target_is_directory=True)
        with self.assertRaises(ValueError):
            snapshot(self.repo, ['link'])

    def test_verified_release_reference_and_installed_assets_must_match_exact_bytes(self):
        artifact = self.repo / 'crates/hoyadb-web/pkg/hoyadb_web_bg.wasm'
        artifact.parent.mkdir(parents=True)
        artifact.write_bytes(b'known release')
        installed = self.app / 'node_modules/@hoyadb/client/src/wasm'
        installed.mkdir(parents=True)
        (installed / artifact.name).write_bytes(artifact.read_bytes())
        reference = self.repo / 'reference.json'
        record = {'path': 'crates/hoyadb-web/pkg/' + artifact.name, 'bytes': len(artifact.read_bytes()), 'sha256': hashlib.sha256(artifact.read_bytes()).hexdigest()}
        reference.write_text(json.dumps({'status': 'passed', 'source_closure': {'artifacts': [record]}}))
        checked = release_reference(self.repo, self.app, 'reference.json')
        self.assertEqual(checked['sha256'], hashlib.sha256(reference.read_bytes()).hexdigest())
        (installed / artifact.name).write_bytes(b'different')
        with self.assertRaises(ValueError):
            release_reference(self.repo, self.app, 'reference.json')


class BrowserControlLoopTests(unittest.TestCase):
    """Parent stdin admission commands; synthetic protocol only, no browser or timing."""
    def collector(self, directory):
        from browser_server import BrowserCollector
        root = Path(__file__).resolve().parents[1]
        metadata = json.loads((root / 'benchmarks/browser-workloads.json').read_text())
        parent = json.loads((root / 'benchmarks/workloads.json').read_text())
        return BrowserCollector(Path(directory), metadata, parent, 'a' * 64, 'b' * 64, allow_measurements=True,
                                recheck=lambda: ('a' * 64, 'b' * 64))

    def test_admit_malformed_recover_and_cancel_are_explicit_parent_commands(self):
        import io
        coordinates = {'kind': 'warm', 'engine': 'memory', 'cohort': 0, 'run': 1, 'worker_id': 'worker:1'}
        lines = ['ADMIT ' + json.dumps({'coordinates': coordinates, 'environment': {'visibility': 'visible'}}),
                 'ADMIT ' + json.dumps({'coordinates': coordinates}), 'PAUSE now', 'ADMIT {broken',
                 'RECOVER ' + json.dumps({'run_id': None, 'reason': 'parent acknowledged malformed input'}),
                 'CANCEL ' + json.dumps({'reason': 'synthetic end'}), 'ADMIT ' + json.dumps({'coordinates': coordinates})]
        with tempfile.TemporaryDirectory() as directory:
            collector = self.collector(directory)
            emitted = []
            self.assertEqual(control_loop(io.StringIO('\n'.join(lines) + '\n'), collector, emitted.append), 'cancelled')
            kinds = [line.split(' ', 1)[0] for line in emitted]
            self.assertEqual(kinds, ['ADMITTED', 'REJECTED', 'REJECTED', 'REJECTED', 'RECOVERED', 'CANCELLED'])
            self.assertEqual(json.loads(emitted[0].split(' ', 1)[1])['coordinates'], coordinates)
            self.assertEqual(collector.state, 'cancelled')
            with self.assertRaises(ValueError):
                collector.start('memory', 0, 1, 'worker:1')

    def test_N06_parent_revoke_withdraws_unconsumed_permit_and_blocks_until_recover(self):
        import io
        coordinates = {'kind': 'warm', 'engine': 'memory', 'cohort': 0, 'run': 1, 'worker_id': 'worker:1'}
        lines = ['ADMIT ' + json.dumps({'coordinates': coordinates}), 'REVOKE ' + json.dumps({'reason': 'page lost response'}),
                 'REVOKE ' + json.dumps({'reason': 'nothing left'}), 'ADMIT ' + json.dumps({'coordinates': coordinates}),
                 'RECOVER ' + json.dumps({'run_id': None, 'reason': 'revocation acknowledged'}), 'ADMIT ' + json.dumps({'coordinates': coordinates}),
                 'CANCEL ' + json.dumps({'reason': 'end'})]
        with tempfile.TemporaryDirectory() as directory:
            collector = self.collector(directory)
            emitted = []
            control_loop(io.StringIO('\n'.join(lines) + '\n'), collector, emitted.append)
            self.assertEqual([line.split(' ', 1)[0] for line in emitted], ['ADMITTED', 'REVOKED', 'REJECTED', 'REJECTED', 'RECOVERED', 'ADMITTED', 'CANCELLED'])

    def test_closed_control_input_cancels_without_admitting(self):
        import io
        with tempfile.TemporaryDirectory() as directory:
            collector = self.collector(directory)
            emitted = []
            self.assertEqual(control_loop(io.StringIO(''), collector, emitted.append), 'closed')
            self.assertEqual(collector.state, 'cancelled')
            self.assertEqual([line.split(' ', 1)[0] for line in emitted], ['CANCELLED'])


class BrowserArgumentTests(unittest.TestCase):
    def test_A07_ok_run_status_and_prepare_only_cannot_bypass_stdin_admission(self):
        import contextlib, io
        for argv in (['--output', 'x', '--run-status', 'ok'], ['--output', 'x', '--prepare-only', '--admission', 'stdin'], ['--output', 'x', '--port', '70000']):
            with self.subTest(argv=argv), contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                arguments(argv)
        self.assertEqual(arguments(['--output', 'x', '--release-reference', 'reference.json']).run_status, 'contaminated')
        self.assertEqual(arguments(['--output', 'x', '--release-reference', 'reference.json', '--admission', 'stdin', '--run-status', 'ok']).run_status, 'ok')

    def test_release_reference_must_be_explicit_before_preparation(self):
        with self.assertRaises(SystemExit):
            arguments(['--output', 'x'])


class PackageFootprintTests(unittest.TestCase):
    def test_actual_archive_and_installed_asset_bytes_with_fixed_mtime_gzip(self):
        import gzip
        with tempfile.TemporaryDirectory() as directory:
            app = Path(directory)
            client = app / 'node_modules/@hoyadb/client'
            (client / 'src/wasm').mkdir(parents=True)
            (client / 'src/wasm/hoyadb_web_bg.wasm').write_bytes(b'\0asm' + bytes(range(256)) * 8)
            (client / 'src/index.js').write_text('export const value = 1;\n' * 20)
            (client / 'LICENSES.md').write_text('notice\n')
            (client / 'package.json').write_text('{}')
            testing = app / 'node_modules/@hoyadb/testing/src'
            testing.mkdir(parents=True)
            (testing / 'index.js').write_text('export {};\n')
            archives = [{'path': str(app / 'hoyadb-client-0.1.0.tgz'), 'bytes': 1234, 'sha256': 'a' * 64}]
            result = package_footprint(app, archives)
            self.assertEqual(result['archives'], archives)
            wasm = result['packages']['@hoyadb/client']['files']['src/wasm/hoyadb_web_bg.wasm']
            data = (client / 'src/wasm/hoyadb_web_bg.wasm').read_bytes()
            self.assertEqual(wasm, {'kind': 'wasm', 'bytes': len(data), 'gzip_bytes': len(gzip.compress(data, compresslevel=9, mtime=0))})
            self.assertEqual(result['packages']['@hoyadb/client']['files']['LICENSES.md']['kind'], 'notice')
            self.assertEqual(result['packages']['@hoyadb/client']['files']['src/index.js']['kind'], 'javascript')
            self.assertEqual(result['packages']['@hoyadb/client']['total_bytes'], sum(path.stat().st_size for path in client.rglob('*') if path.is_file()))
            self.assertEqual(result['packages']['@hoyadb/testing']['total_bytes'], (testing / 'index.js').stat().st_size)
            self.assertEqual(result['gzip_command'], "python3 gzip.compress(data, compresslevel=9, mtime=0) per file")
            self.assertEqual(package_footprint(app, archives), result)
            (client / 'src/link.js').symlink_to(client / 'src/index.js')
            with self.assertRaises(ValueError):
                package_footprint(app, archives)



class AcceptedSampleCountTests(unittest.TestCase):
    def test_untimed_resource_runs_contribute_no_latency_samples(self):
        self.assertEqual(accepted_samples([{'kind': 'cold'}, {}, {'kind': 'resources'}]), 31)


if __name__ == '__main__':
    unittest.main()
