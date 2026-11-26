#!/usr/bin/env python3
"""Synthetic packaging guards; never activate a synthetic native artifact."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from package_fixture import Fixture  # noqa: E402
SPEC = importlib.util.spec_from_file_location('package_aar_under_test', HERE / 'package-aar.py')
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class NativeArtifactBinding(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='hoyadb-aar-guard-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.classes = self.root / 'build/classes.jar'
        self.classes.parent.mkdir()
        self.classes.write_bytes(b'synthetic class artifact; no execution')
        (self.classes.parent / 'manifest.json').write_text(json.dumps({
            'test_classes_included': False, 'classes_sha256': sha(self.classes)}))
        self.inputs = self.root / 'native-input.c'
        self.inputs.write_bytes(b'synthetic source; no compilation')
        for abi in ('x86_64', 'arm64-v8a'):
            library = self.root / 'target/phase-seven-native/jni-current' / abi / 'libhoyadb_jni.so'
            library.parent.mkdir(parents=True)
            library.write_bytes(b'synthetic native artifact; no activation')
            library.with_name('evidence.json').write_text(json.dumps({
                'abi': abi, 'min_sdk': 24, 'artifact': str(library),
                'artifact_sha256': sha(library), 'inputs_sha256': {str(self.inputs): sha(self.inputs)}}))
        self.out = self.root / 'final/hoyadb-android-0.1.0.aar'

    def rejects_before_final_artifact(self, expected_reason):
        with patch.object(PACKAGE, 'ROOT', self.root), patch.object(sys, 'argv', [
                'package-aar.py', '--classes', str(self.classes), '--out', str(self.out)]):
            with self.assertRaises(SystemExit) as failure:
                PACKAGE.main()
            self.assertEqual(str(failure.exception), expected_reason)
        self.assertFalse(self.out.exists(), 'a rejected input cannot produce a final AAR')
        self.assertFalse(self.out.with_suffix('.json').exists())

    def test_p01_current_so_must_match_native_evidence(self):
        library = self.root / 'target/phase-seven-native/jni-current/x86_64/libhoyadb_jni.so'
        library.write_bytes(library.read_bytes() + b'changed after evidence')
        self.rejects_before_final_artifact('native artifact digest mismatch')

    def test_p01_native_source_must_match_evidence(self):
        self.inputs.write_bytes(b'changed source after evidence')
        self.rejects_before_final_artifact('native input digest mismatch')

    def test_p01_missing_native_input_must_not_certify(self):
        self.inputs.unlink()
        self.rejects_before_final_artifact('native input missing')

    def test_p01_missing_native_evidence_rejects_statically(self):
        (self.root / 'target/phase-seven-native/jni-current/x86_64/evidence.json').unlink()
        self.rejects_before_final_artifact('native artifact missing')

    def test_p01_malformed_native_evidence_rejects_statically(self):
        (self.root / 'target/phase-seven-native/jni-current/x86_64/evidence.json').write_text('not json')
        self.rejects_before_final_artifact('native evidence profile mismatch')

    def test_p01_evidence_without_rust_source_closure_must_not_certify(self):
        # The archive digest alone does not show which Rust sources it was compiled from.
        self.rejects_before_final_artifact('native Rust source closure missing')


class FinalCertificate(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='hoyadb-aar-final-')
        self.addCleanup(self.temp.cleanup)
        self.fixture = Fixture(Path(self.temp.name).resolve())

    def package(self, abis=None):
        fixture = self.fixture
        argv = ['package-aar.py', '--classes', str(fixture.classes), '--out', str(fixture.out)]
        if abis is not None:
            argv += ['--abis', *abis]
        with patch.object(PACKAGE, 'ROOT', fixture.root), patch.object(PACKAGE, 'READELF', fixture.readelf), \
                patch.object(PACKAGE, 'CRATES', fixture.crates), \
                patch.object(subprocess, 'run', side_effect=fixture.execute), \
                patch.object(sys, 'argv', argv):
            PACKAGE.main()

    def test_p05_p08_complete_local_coordinate_and_honest_certificate(self):
        self.package()
        out = self.fixture.out
        coordinate = out / 'dev/hoyadb/hoyadb-android/0.1.0'
        names = ['hoyadb-android-0.1.0.aar', 'hoyadb-android-0.1.0.pom', 'hoyadb-android-0.1.0-sources.jar', 'hoyadb-android-0.1.0-notices.zip']
        self.assertEqual(sorted(p.name for p in coordinate.iterdir()), sorted(names + [name + '.sha256' for name in names]))
        for name in names:
            self.assertEqual((coordinate / (name + '.sha256')).read_text().strip(), sha(coordinate / name))
        index = self.fixture.load_index()
        notices = ['NOTICE.md', 'third-party/index.json', 'third-party/LICENSE'] + ['third-party/' + f['file'] for p in index['packages'] for f in p['files']]
        with zipfile.ZipFile(coordinate / names[0]) as aar:
            self.assertEqual(sorted(aar.namelist()), sorted(['AndroidManifest.xml', 'classes.jar', 'consumer-rules.pro',
                'jni/x86_64/libhoyadb_jni.so', 'jni/arm64-v8a/libhoyadb_jni.so'] + ['META-INF/hoyadb-android/' + n for n in notices]))
            self.assertEqual(aar.read('classes.jar'), self.fixture.classes.read_bytes())
            for abi in ['x86_64', 'arm64-v8a']:
                self.assertEqual(aar.read('jni/' + abi + '/libhoyadb_jni.so'),
                    (self.fixture.root / 'target/phase-seven-native/jni-current' / abi / 'libhoyadb_jni.so').read_bytes())
        with zipfile.ZipFile(coordinate / names[3]) as archive:
            self.assertEqual(sorted(archive.namelist()), sorted(notices))
        with zipfile.ZipFile(coordinate / names[2]) as archive:
            self.assertEqual(sorted(archive.namelist()), ['dev/hoyadb/NativeBridge.java', 'dev/hoyadb/Values.kt'])
        pom = (coordinate / names[1]).read_text()
        for fragment in ['<groupId>dev.hoyadb</groupId>', '<artifactId>hoyadb-android</artifactId>', '<version>0.1.0</version>',
                         '<packaging>aar</packaging>', '<artifactId>kotlin-stdlib</artifactId>', '<version>2.1.20</version>']:
            self.assertIn(fragment, pom)
        self.assertEqual(pom.count('<dependency>'), 1)
        self.assertNotIn('<licenses>', pom)
        certificate = json.loads((out / 'certificate.json').read_text())
        self.assertFalse(certificate['development_only'])
        self.assertEqual(certificate['compiler'], 'installed2.2.20 language/API/metadata2.1')
        self.assertEqual(certificate['abis'], {'x86_64': 'runtime pending independent consumer', 'arm64-v8a': 'build-only'})
        self.assertEqual(certificate['files'], {name: sha(coordinate / name) for name in names})
        self.assertEqual(set(certificate['elf']), {'x86_64', 'arm64-v8a'})
        for abi, record in certificate['elf'].items():
            self.assertEqual([r['command'][1:-1] for r in record['inspections']], [['-h'], ['-lW'], ['--dyn-syms', '-W'], ['-dW']])
            self.assertEqual(record['exports'], sorted(self.fixture.elf[abi]['exports']))
        self.assertEqual(certificate['sources'], self.fixture.production_sources())
        self.assertFalse(list(out.parent.glob('final-repo.pending-*')))


class FinalGuards(unittest.TestCase):
    """Each guard must reject without a final repository or certificate."""
    setUp = FinalCertificate.setUp
    package = FinalCertificate.package

    def rejects(self, reason, pending=True):
        with self.assertRaises(SystemExit) as failure:
            self.package()
        self.assertEqual(str(failure.exception), reason)
        out = self.fixture.out
        self.assertFalse(out.exists() or out.is_symlink())
        self.assertFalse(list(self.fixture.root.rglob('certificate.json')))
        attempts = list(out.parent.glob(out.name + '.pending-*'))
        if pending:
            self.assertEqual(len(attempts), 1)
            self.assertEqual(json.loads((attempts[0] / 'failure.json').read_text())['reason'], reason)
        else:
            self.assertEqual(attempts, [])

    # P02 actual ELF
    def test_p02_load_alignment_below_16k(self):
        self.fixture.elf['arm64-v8a']['align'] = '0x1000'
        self.rejects('native ELF LOAD alignment below 16KiB')

    def test_p02_unexpected_export(self):
        self.fixture.elf['x86_64']['exports'].append('leaked_internal_symbol')
        self.rejects('native ELF exports differ from exports.map')

    def test_p02_missing_export(self):
        self.fixture.elf['x86_64']['exports'].pop()
        self.rejects('native ELF exports differ from exports.map')

    def test_p02_unexpected_needed(self):
        self.fixture.elf['x86_64']['needed'].append('libssl.so')
        self.rejects('native ELF unexpected NEEDED dependency')

    def test_p02_wrong_machine(self):
        self.fixture.elf['arm64-v8a']['machine'] = 'Advanced Micro Devices X86-64'
        self.rejects('native ELF machine mismatch')

    def test_p02_failed_tool_exit(self):
        self.fixture.elf['x86_64']['exit'] = 1
        self.rejects('native ELF inspection failed')

    # P03 classes and source closure
    def test_p03_test_class_in_production_classes(self):
        self.fixture.classes = self.fixture.build(('dev/hoyadb/Values.class', 'dev/hoyadb/SdkTests.class'))
        self.rejects('test class in production classes')

    def test_p03_source_changed_after_build(self):
        (self.fixture.android / 'src/main/kotlin/dev/hoyadb/Values.kt').write_text('changed after build')
        self.rejects('source closure mismatch')

    def test_p03_source_added_after_build(self):
        (self.fixture.android / 'src/main/kotlin/dev/hoyadb/Added.kt').write_text('added after build')
        self.rejects('source closure mismatch')

    def test_p03_recorded_build_drift(self):
        manifest_path = self.fixture.classes.parent / 'manifest.json'
        manifest = json.loads(manifest_path.read_text())
        manifest['sources_before'] = dict(manifest['sources_before'], extra='0' * 64)
        manifest_path.write_text(json.dumps(manifest))
        self.rejects('source closure mismatch')

    def test_p03_split_runtime_artifact(self):
        manifest_path = self.fixture.classes.parent / 'manifest.json'
        manifest = json.loads(manifest_path.read_text())
        manifest['split_runtime'] = True
        manifest_path.write_text(json.dumps(manifest))
        self.rejects('not a verified production-only class artifact', pending=False)

    # P04 notice closure
    def test_p04_missing_license_text(self):
        (self.fixture.android / 'third-party/rust/alpha-1.0.0/LICENSE').unlink()
        self.rejects('notice file missing')

    def test_p04_corrupted_license_text(self):
        (self.fixture.android / 'third-party/rust/alpha-1.0.0/LICENSE').write_text('corrupted text')
        self.rejects('notice file digest mismatch')

    def test_p04_wrong_license_text_with_consistent_index(self):
        wrong = (self.fixture.android / 'third-party/rust/beta-tool-2.0.0/LICENSE').read_bytes()
        (self.fixture.android / 'third-party/rust/alpha-1.0.0/LICENSE').write_bytes(wrong)
        index = self.fixture.load_index()
        index['packages'][0]['files'][0]['sha256'] = hashlib.sha256(wrong).hexdigest()
        self.fixture.write_index(index)
        self.rejects('notice text differs from verified source')

    def test_p04_wrong_provider_text_with_consistent_index(self):
        path = self.fixture.android / 'third-party/native/SQLCipher/LICENSE'
        path.write_bytes(b'not the crate sqlcipher license\n')
        index = self.fixture.load_index()
        entry = next(p for p in index['packages'] if p['name'] == 'SQLCipher')
        entry['files'][0]['sha256'] = sha(path)
        self.fixture.write_index(index)
        self.rejects('notice text differs from verified source')

    def test_p04_unclassified_active_package(self):
        index = self.fixture.load_index()
        index['packages'] = [p for p in index['packages'] if p['name'] != 'beta-tool']
        self.fixture.write_index(index)
        (self.fixture.android / 'third-party/rust/beta-tool-2.0.0/LICENSE').unlink()
        self.rejects('notice set differs from active native set')

    def test_p04_unknown_role(self):
        index = self.fixture.load_index()
        index['packages'][0]['role'] = 'runtime'
        self.fixture.write_index(index)
        self.rejects('unclassified notice package')

    def test_p04_lock_tuple_changed(self):
        lock = self.fixture.root / 'crates/hoyadb-native/Cargo.lock'
        lock.write_text(lock.read_text().replace('version = "1.0.0"', 'version = "1.0.1"', 1))
        # The native library is relinked from the changed lock, so only the notice tuples are stale.
        self.fixture.link_native()
        self.rejects('notice tuple differs from Cargo.lock')

    def test_p01_rust_source_changed_after_native_build(self):
        source = self.fixture.root / 'crates/hoyadb-core/src/lib.rs'
        source.write_text(source.read_text() + '// changed after the native archive was linked\n')
        self.rejects('native Rust source closure mismatch', pending=False)

    def test_p01_rust_source_added_after_native_build(self):
        (self.fixture.root / 'crates/hoyadb-bindings/src/added.rs').write_text('// new module\n')
        self.rejects('native Rust source closure mismatch', pending=False)

    def test_p01_rust_source_removed_after_native_build(self):
        (self.fixture.root / 'crates/hoyadb-storage-sqlite/build.rs').unlink()
        self.rejects('native Rust source closure mismatch', pending=False)

    def test_p08_certificate_records_rust_source_closure(self):
        self.package()
        certificate = json.loads((self.fixture.out / 'certificate.json').read_text())
        self.assertEqual(certificate['native_rust_sources_sha256'], self.fixture.rust_sources)
        self.assertIn('crates/hoyadb-core/src/lib.rs', certificate['native_rust_sources_sha256'])

    def test_p04_index_bound_to_other_lock_bytes(self):
        # Same package tuples, but the index was generated from different lock bytes.
        index = self.fixture.load_index()
        index['lock_sha256']['Cargo.lock'] = '0' * 64
        self.fixture.write_index(index)
        self.rejects('notice index lock digest mismatch')

    def test_p04_lock_path_not_a_current_lock(self):
        index = self.fixture.load_index()
        index['lock_sha256']['bindings/native/Cargo.lock'] = index['lock_sha256'].pop('crates/hoyadb-native/Cargo.lock')
        self.fixture.write_index(index)
        self.rejects('notice index lock path mismatch')

    def test_p04_lock_binding_missing(self):
        index = self.fixture.load_index()
        del index['lock_sha256']
        self.fixture.write_index(index)
        self.rejects('notice index lock path mismatch')

    def test_p04_kotlin_pom_changed(self):
        pom = self.fixture.root / 'poms/kotlin-stdlib.pom'
        pom.write_bytes(pom.read_bytes() + b'<!-- changed -->')
        self.rejects('Kotlin runtime notice provenance mismatch')

    def test_p04_kotlin_runtime_not_approved_jar(self):
        index = self.fixture.load_index()
        next(p for p in index['packages'] if p['role'] == 'kotlin_runtime')['jar_sha256'] = '0' * 64
        self.fixture.write_index(index)
        self.rejects('Kotlin runtime notice provenance mismatch')

    def test_p04_unindexed_notice_file(self):
        (self.fixture.android / 'third-party/rust/stray-LICENSE').write_text('unindexed text')
        self.rejects('unindexed notice file')

    def test_p04_missing_notice_summary(self):
        (self.fixture.android / 'NOTICE.md').unlink()
        self.rejects('notice file missing')

    def test_p04_invented_project_license(self):
        index = self.fixture.load_index()
        index['project_distribution_license'] = 'Apache-2.0'
        self.fixture.write_index(index)
        self.rejects('project distribution license must match MIT')

    def test_p04_changed_project_license_text(self):
        (self.fixture.android / 'third-party/LICENSE').write_text('changed license')
        self.rejects('project license digest mismatch')

    # P04 supplemental toolchain aggregate attribution
    def test_p04_missing_toolchain_notice(self):
        (self.fixture.android / 'third-party/toolchain/rust-1.98.1/COPYRIGHT-library.html').unlink()
        self.rejects('notice file missing')

    def test_p04_corrupted_toolchain_notice(self):
        (self.fixture.android / 'third-party/toolchain/rust-1.98.1/COPYRIGHT-library.html').write_text('corrupted')
        self.rejects('notice file digest mismatch')

    def test_p04_toolchain_notice_not_shipped_text(self):
        path = self.fixture.android / 'third-party/toolchain/rust-1.98.1/COPYRIGHT-library.html'
        path.write_bytes(b'<p>not the shipped toolchain text</p>\n')
        index = self.fixture.load_index()
        next(p for p in index['packages'] if p['role'] == 'toolchain_aggregate')['files'][0]['sha256'] = sha(path)
        self.fixture.write_index(index)
        self.rejects('notice text differs from verified source')

    def test_p04_toolchain_link_input_changed(self):
        self.fixture.link_input.write_bytes(b'different rlib')
        self.rejects('toolchain link input changed')

    # P06 fresh output, no partial certificate, re-verification
    def test_p06_existing_destination(self):
        self.fixture.out.mkdir()
        with self.assertRaises(SystemExit) as failure:
            self.package()
        self.assertEqual(str(failure.exception), 'final output must be fresh')
        self.assertEqual(list(self.fixture.out.iterdir()), [])

    def test_p06_symlinked_destination(self):
        target = self.fixture.root / 'elsewhere'
        target.mkdir()
        self.fixture.out.symlink_to(target)
        with self.assertRaises(SystemExit) as failure:
            self.package()
        self.assertEqual(str(failure.exception), 'final output must be fresh')
        self.assertEqual(list(target.iterdir()), [])

    def test_p06_source_drift_during_packaging_is_reverified(self):
        original = self.fixture.execute
        source = self.fixture.android / 'src/main/kotlin/dev/hoyadb/Values.kt'
        def drifting(command, **kwargs):
            if command[1:-1] == ['-dW'] and Path(command[-1]).parent.name == 'arm64-v8a':
                source.write_text('changed during packaging')
            return original(command, **kwargs)
        self.fixture.execute = drifting
        self.rejects('source closure mismatch')


class ExplicitAndroidAbis(unittest.TestCase):
    abis = ['x86_64', 'arm64-v8a', 'armeabi-v7a', 'x86']

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='hoyadb-aar-four-abis-')
        self.addCleanup(self.temp.cleanup)
        self.fixture = Fixture(Path(self.temp.name).resolve(), self.abis)

    def package(self):
        return FinalCertificate.package(self, self.abis)

    rejects = FinalGuards.rejects

    def test_explicit_four_abi_archive_and_build_only_certificate(self):
        self.package()
        coordinate = self.fixture.out / 'dev/hoyadb/hoyadb-android/0.1.0'
        with zipfile.ZipFile(coordinate / 'hoyadb-android-0.1.0.aar') as archive:
            self.assertEqual(sorted(n for n in archive.namelist() if n.startswith('jni/')),
                             sorted('jni/' + abi + '/libhoyadb_jni.so' for abi in self.abis))
            for abi in self.abis:
                source = self.fixture.root / 'target/phase-seven-native/jni-current' / abi / 'libhoyadb_jni.so'
                self.assertEqual(archive.read('jni/' + abi + '/libhoyadb_jni.so'), source.read_bytes())
        certificate = json.loads((self.fixture.out / 'certificate.json').read_text())
        self.assertEqual(certificate['abis'], {'x86_64': 'runtime pending independent consumer',
                         'arm64-v8a': 'build-only', 'armeabi-v7a': 'build-only', 'x86': 'build-only'})
        self.assertEqual(set(certificate['elf']), set(self.abis))
        for abi in self.abis:
            self.assertEqual(len(certificate['elf'][abi]['inspections']), 4)

    def test_selected_32bit_slice_missing_rejects_before_output(self):
        (self.fixture.root / 'target/phase-seven-native/jni-current/x86/libhoyadb_jni.so').unlink()
        self.rejects('native artifact missing', pending=False)

    def test_32bit_machine_mismatch_rejects(self):
        self.fixture.elf['armeabi-v7a']['machine'] = 'Intel 80386'
        self.rejects('native ELF machine mismatch')

    def test_64bit_elf_in_32bit_slice_rejects(self):
        self.fixture.elf['x86']['class'] = 'ELF64'
        self.rejects('native ELF machine mismatch')

    def test_32bit_elf_in_64bit_slice_rejects(self):
        self.fixture.elf['x86_64']['class'] = 'ELF32'
        self.rejects('native ELF machine mismatch')

    def test_32bit_load_alignment_guard_retained(self):
        self.fixture.elf['armeabi-v7a']['align'] = '0x1000'
        self.rejects('native ELF LOAD alignment below 16KiB')

    def test_32bit_export_guard_retained(self):
        self.fixture.elf['x86']['exports'].append('leaked_internal_symbol')
        self.rejects('native ELF exports differ from exports.map')

    def test_32bit_needed_guard_retained(self):
        self.fixture.elf['armeabi-v7a']['needed'].append('libssl.so')
        self.rejects('native ELF unexpected NEEDED dependency')

    def test_duplicate_abi_selection_rejects(self):
        self.abis = ['x86', 'x86']
        self.rejects('duplicate ABI selection', pending=False)


class CommittedNoticeIndex(unittest.TestCase):
    """The committed index names the current lock files and their current bytes."""

    def test_committed_index_binds_current_lock_paths_and_bytes(self):
        root = HERE.parents[2]
        index = json.loads((root / 'bindings/android/third-party/index.json').read_text())
        self.assertEqual(sorted(index['lock_sha256']), sorted(PACKAGE.LOCKS))
        for lock in PACKAGE.LOCKS:
            self.assertEqual(index['lock_sha256'][lock], sha(root / lock), lock)


if __name__ == '__main__':
    unittest.main()
