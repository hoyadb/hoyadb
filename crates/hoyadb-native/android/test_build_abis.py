"""Exercise Android compiler selection with owned fixtures, never real compilation."""
import hashlib
import io
import json
import os
from pathlib import Path
import runpy
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
TARGETS = {'x86_64': 'x86_64-linux-android', 'arm64-v8a': 'aarch64-linux-android',
           'armeabi-v7a': 'armv7a-linux-androideabi', 'x86': 'i686-linux-android'}


def write(path, content):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content if isinstance(content, bytes) else content.encode())
    return path


class AndroidCompilerTargets(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='hoyadb-android-abi-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.native = self.root / 'crates/hoyadb-native'
        self.android = self.native / 'android'
        for name in ['build-provider.py', 'build-jni.py', 'rust_sources.py', 'hoya_entropy.c', 'ltc_config.h', 'hoya_jni.c', 'exports.map']:
            write(self.android / name, (HERE / name).read_bytes())
        write(self.native / 'include/hoyadb.h', (HERE.parent / 'include/hoyadb.h').read_bytes())
        write(self.native / 'Cargo.toml', '[package]\nname = "synthetic"\n')
        write(self.native / 'src/lib.rs', '// synthetic facade\n')
        self.ndk = self.root / 'ndk'
        write(self.ndk / 'source.properties', 'synthetic NDK; not executable\n')
        self.calls = []

    def compile(self, argv, **kwargs):
        argv = [str(x) for x in argv]
        self.calls.append(argv)
        if '-o' in argv:
            write(Path(argv[argv.index('-o') + 1]), b'synthetic object; never linked')
        elif Path(argv[0]).name == 'llvm-ar':
            write(Path(argv[2]), b'synthetic archive; never linked')
        return subprocess.CompletedProcess(argv, 0)

    def provider_source(self):
        version = '419.0.6+sqlcipher-4.19.0-sqlite-3.53.4-libtomcrypt-1.18.2'
        package = 'sqlcipher-src-' + version
        source = self.root / 'registry/src/index' / package
        entries = {'Cargo.toml': '[package]\n', 'sqlcipher/sqlcipher.c': 'synthetic SQLCipher C',
                   'sqlcipher/sqlite3.h': 'synthetic sqlite header',
                   'src/libtomcrypt_sources.rs': '"libtomcrypt/prngs/rng_get_bytes.c"\n"libtomcrypt/hash.c"\n',
                   'sqlcipher/libtomcrypt/prngs/rng_get_bytes.c': 'excluded entropy',
                   'sqlcipher/libtomcrypt/hash.c': 'synthetic LTC C'}
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode='w:gz') as archive:
            for relative, content in entries.items():
                write(source / relative, content)
                data = content.encode()
                member = tarfile.TarInfo(package + '/' + relative)
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))
        data = buffer.getvalue()
        write(self.root / 'registry/cache/index' / (package + '.crate'), data)
        write(self.native / 'Cargo.lock', '[[package]]\nname = "sqlcipher-src"\nchecksum = "' +
              hashlib.sha256(data).hexdigest() + '"\n')
        return json.dumps({'packages': [{'name': 'sqlcipher-src', 'version': version,
                                        'manifest_path': str(source / 'Cargo.toml')}]}).encode()

    def test_provider_targets_preserve_approved_flags_and_entropy_source(self):
        metadata = self.provider_source()
        for abi, target in TARGETS.items():
            with self.subTest(abi=abi):
                self.calls.clear()
                output = self.root / ('provider-' + abi)
                argv = ['build-provider.py', '--abi', abi, '--ndk', str(self.ndk), '--output', str(output)]
                with patch.object(sys, 'argv', argv), patch.object(subprocess, 'check_output', return_value=metadata), \
                        patch.object(subprocess, 'run', side_effect=self.compile):
                    runpy.run_path(str(self.android / 'build-provider.py'), run_name='__main__')
                compiled = [call for call in self.calls if '-c' in call]
                self.assertEqual(len(compiled), 3)
                for call in compiled:
                    self.assertIn('--target=' + target + '24', call)
                    self.assertIn('-DSQLCIPHER_CRYPTO_LIBTOMCRYPT', call)
                    self.assertIn('-DSQLITE_TEMP_STORE=3', call)
                    self.assertIn('-fPIC', call)
                sources = [call[call.index('-c') + 1] for call in compiled]
                self.assertIn(str(self.android / 'hoya_entropy.c'), sources)
                self.assertFalse(any(p.endswith('rng_get_bytes.c') for p in sources))
                evidence = json.loads((output / 'evidence.json').read_text())
                self.assertEqual((evidence['abi'], evidence['min_sdk']), (abi, 24))
                self.assertFalse(evidence['runtime_verified'])

    def test_jni_targets_keep_no_undefined_exports_and_alignment(self):
        archive = write(self.root / 'input.a', b'synthetic Rust archive; never linked')
        for abi, target in TARGETS.items():
            with self.subTest(abi=abi):
                self.calls.clear()
                output = self.root / ('jni-' + abi)
                argv = ['build-jni.py', '--abi', abi, '--ndk', str(self.ndk), '--archive', str(archive), '--out', str(output)]
                with patch.object(sys, 'argv', argv), patch.object(subprocess, 'run', side_effect=self.compile):
                    runpy.run_path(str(self.android / 'build-jni.py'), run_name='__main__')
                self.assertEqual(len(self.calls), 1)
                call = self.calls[0]
                self.assertEqual(Path(call[0]).name, target + '24-clang')
                for flag in ['-Wl,--no-undefined', '-Wl,-z,max-page-size=16384', '-Werror', '-pthread']:
                    self.assertIn(flag, call)
                self.assertIn('-Wl,--version-script,' + str(self.android / 'exports.map'), call)
                evidence = json.loads((output / 'evidence.json').read_text())
                self.assertEqual((evidence['abi'], evidence['min_sdk']), (abi, 24))
                self.assertFalse(evidence['runtime_verified'])
                # The Rust source closure the archive was compiled from is recorded for packaging.
                self.assertEqual(evidence['rust_sources_sha256'], {
                    'crates/hoyadb-native/Cargo.toml': hashlib.sha256(b'[package]\nname = "synthetic"\n').hexdigest(),
                    'crates/hoyadb-native/include/hoyadb.h': hashlib.sha256((HERE.parent / 'include/hoyadb.h').read_bytes()).hexdigest(),
                    'crates/hoyadb-native/src/lib.rs': hashlib.sha256(b'// synthetic facade\n').hexdigest()})

    def test_jni_refuses_archive_older_than_a_rust_source(self):
        archive = write(self.root / 'input.a', b'synthetic Rust archive; never linked')
        source = self.native / 'src/lib.rs'
        stamp = archive.stat().st_mtime + 60
        os.utime(source, (stamp, stamp))
        output = self.root / 'jni-stale'
        argv = ['build-jni.py', '--abi', 'x86_64', '--ndk', str(self.ndk), '--archive', str(archive), '--out', str(output)]
        with patch.object(sys, 'argv', argv), patch.object(subprocess, 'run', side_effect=self.compile):
            with self.assertRaises(SystemExit) as failure:
                runpy.run_path(str(self.android / 'build-jni.py'), run_name='__main__')
        self.assertEqual(str(failure.exception), 'Rust archive predates a bound Rust source; rebuild it first')
        self.assertEqual(self.calls, [])
        self.assertFalse((output / 'evidence.json').exists())


if __name__ == '__main__':
    unittest.main()
