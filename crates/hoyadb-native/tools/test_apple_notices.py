#!/usr/bin/env python3
"""DEF-0030 regression suite: Apple native notice corpus, XCFramework finalizer and packers.

Stdlib unittest over synthetic owned fixtures in fresh temporary roots: no dependency fetching, no
native compilation. The actual tools run as subprocesses under ``python -I`` and ``python -I -O``
(AN03). The orchestration cases copy the actual build-artifacts.py and RN pack tool into a fresh
rebuilt-layout fixture and drive them with compile-command stubs on PATH (AN02).

  python3 -I crates/hoyadb-native/tools/test_apple_notices.py
  python3 -I -O crates/hoyadb-native/tools/test_apple_notices.py
"""
import ast
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[3]
TOOL = REPO / 'crates/hoyadb-native/tools/apple_notices.py'
BUILD = REPO / 'bindings/swift/build-artifacts.py'
PACK = REPO / 'packages/react-native/tools/pack.py'
CHECK = REPO / 'bindings/swift/check_artifacts.py'
NPMIGNORE = REPO / 'packages/react-native/android/.npmignore'
MODES = {'normal': ['-I'], 'optimized': ['-I', '-O']}
NAME = 'HoyaNative.xcframework'
TARGETS = ['x86_64-apple-darwin', 'aarch64-apple-ios', 'aarch64-apple-ios-sim', 'x86_64-apple-ios']
SLICES = {'macos-x86_64': ['x86_64'], 'ios-arm64': ['arm64'], 'ios-arm64_x86_64-simulator': ['arm64', 'x86_64']}
REGISTRY = 'registry+https://github.com/rust-lang/crates.io-index'
CC = ['_CCCryptorCreate', '_CCHmacInit', '_CCKeyDerivationPBKDF']
SEC = ['_SecRandomCopyBytes', '_kSecRandomDefault']


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data if isinstance(data, bytes) else data.encode('utf-8'))
    return path


def tree(directory):
    return {p.relative_to(directory).as_posix(): sha(p.read_bytes()) for p in sorted(directory.rglob('*')) if p.is_file()}


def lock_text(extra=''):
    packages = [('alpha', '1.0.0', REGISTRY, 'a' * 64), ('beta', '2.0.0', REGISTRY, 'b' * 64),
                ('csrc', '3.0.0+c-1.0', REGISTRY, 'c' * 64), ('wasmonly', '0.1.0', REGISTRY, 'd' * 64)]
    text = 'version = 4\n\n[[package]]\nname = "hoyadb-native"\nversion = "0.1.0"\n'
    for name, version, source, checksum in packages:
        text += f'\n[[package]]\nname = "{name}"\nversion = "{version}"\nsource = "{source}"\nchecksum = "{checksum}"\n'
    return text + extra


def native_members():
    """Synthetic stand-ins for the ten native members, keyed relative to the framework root."""
    libraries = [{'LibraryIdentifier': i, 'LibraryPath': 'libhoyadb_native.a', 'HeadersPath': 'Headers'} for i in sorted(SLICES)]
    members = {'Info.plist': plistlib.dumps({'AvailableLibraries': libraries, 'CFBundlePackageType': 'XFWK'})}
    for identifier in SLICES:
        members[f'{identifier}/libhoyadb_native.a'] = f'!<arch> synthetic {identifier}\n'.encode()
        members[f'{identifier}/Headers/hoyadb.h'] = b'int hoya_v1_abi_version(void);\n'
        members[f'{identifier}/Headers/HoyaNative/module.modulemap'] = b'module HoyaNative {\n    header "../hoyadb.h"\n    export *\n}\n'
    return members


def corpus_index(texts, notice, lock_bytes, libraries):
    def files(*names):
        return [{'file': n, 'sha256': sha(texts[n]), 'origin': {'kind': 'synthetic', 'file': n}} for n in names]

    evidence = lambda symbols: [{'library': lib, 'sha256': digest, 'imported_symbols': symbols, 'libtomcrypt_symbols': 0}
                                for lib, digest in sorted(libraries.items())]
    return {
        'format': 1, 'platform': 'apple', 'scope': 'synthetic fixture', 'project_distribution_license': 'MIT',
        'project_license_sha256': sha(texts['LICENSE']),
        'lock': {'file': 'crates/hoyadb-native/Cargo.lock', 'sha256': sha(lock_bytes)}, 'apple_targets': TARGETS,
        'graph': {'normal': ['alpha 1.0.0'], 'build': ['beta 2.0.0', 'csrc 3.0.0+c-1.0']},
        'excluded_lock_packages': [{'name': 'wasmonly', 'version': '0.1.0', 'reason': 'not in the Apple graph'}],
        'notice': {'file': 'NOTICE.md', 'sha256': sha(notice)},
        'packages': [
            {'name': 'alpha', 'version': '1.0.0', 'source': REGISTRY, 'checksum': 'a' * 64, 'license': 'MIT',
             'role': 'linked_rust', 'files': files('rust/alpha-1.0.0/LICENSE')},
            {'name': 'beta', 'version': '2.0.0', 'source': REGISTRY, 'checksum': 'b' * 64, 'license': 'MIT',
             'role': 'build_tool', 'files': files('rust/beta-2.0.0/LICENSE')},
            {'name': 'csrc', 'version': '3.0.0+c-1.0', 'source': REGISTRY, 'checksum': 'c' * 64, 'license': 'MIT',
             'role': 'build_native_c_source', 'files': files('rust/csrc-3.0.0+c-1.0/LICENSE')},
            {'name': 'CLib', 'version': '1.0', 'license': 'BSD-3-Clause', 'role': 'linked_native_c', 'via': 'csrc',
             'files': files('native/clib-1.0/LICENSE-clib')},
            {'name': 'LibTomCrypt', 'version': '1.18.2', 'license': 'n/a (not linked)', 'role': 'unused_source_not_linked',
             'via': 'csrc', 'files': []},
            {'name': 'CommonCrypto', 'role': 'os_framework_not_redistributed', 'files': [], 'evidence': evidence(CC)},
            {'name': 'Security', 'role': 'os_framework_not_redistributed', 'files': [], 'evidence': evidence(SEC)},
            {'name': 'rust-std', 'version': '1.98.1', 'license': 'aggregate toolchain notice', 'role': 'toolchain_aggregate',
             'verified_link_inputs': [{'target': t, 'path': f'/toolchain/{t}/libstd.rlib', 'sha256': 'e' * 64} for t in TARGETS],
             'files': files('toolchain/rust-1.98.1/COPYRIGHT-library.html')},
        ],
    }


def make_corpus(root, libraries=None):
    """Write a valid synthetic corpus plus lock under root; return (corpus, lock)."""
    if libraries is None:
        libraries = {k: sha(v) for k, v in native_members().items() if k.endswith('.a')}
    lock = write(root / 'Cargo.lock', lock_text())
    corpus = root / 'third-party'
    texts = {n: f'license text for {n}\n'.encode() for n in
             ['rust/alpha-1.0.0/LICENSE', 'rust/beta-2.0.0/LICENSE', 'rust/csrc-3.0.0+c-1.0/LICENSE',
              'native/clib-1.0/LICENSE-clib', 'toolchain/rust-1.98.1/COPYRIGHT-library.html']}
    texts['LICENSE'] = (Path(__file__).resolve().parents[3] / 'LICENSE').read_bytes()
    for name, data in texts.items():
        write(corpus / name, data)
    notice = b'# Synthetic Apple notices\n'
    write(corpus / 'NOTICE.md', notice)
    save_index(corpus, corpus_index(texts, notice, lock.read_bytes(), libraries))
    return corpus, lock


def save_index(corpus, index):
    write(corpus / 'index.json', json.dumps(index, indent=2) + '\n')


def load_index(corpus):
    return json.loads((corpus / 'index.json').read_text())


def make_framework(root, members=None):
    """Write a notice-free synthetic framework and its separate native-build.json parent."""
    members = native_members() if members is None else members
    artifacts = root / 'Artifacts'
    framework = artifacts / NAME
    for relative, data in members.items():
        write(framework / relative, data)
    parent = {'profile': 'synthetic', 'runtime_verified': False,
              'artifact_files_sha256': {f'{NAME}/{k}': sha(v) for k, v in sorted(members.items())}}
    record = write(artifacts / 'native-build.json', json.dumps(parent, indent=2) + '\n')
    return framework, record


def run_tool(mode, *args, tool=TOOL, env=None, cwd=None):
    return subprocess.run([sys.executable, *MODES[mode], str(tool), *map(str, args)], stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, text=True, timeout=300, env=env, cwd=cwd)


class Base(unittest.TestCase):
    def setUp(self):
        self._scratch = tempfile.TemporaryDirectory(prefix='hoya-def0030-')
        self.addCleanup(self._scratch.cleanup)
        self.root = Path(self._scratch.name)

    def fresh(self, name):
        path = self.root / f'{len(list(self.root.iterdir())):03d}-{name}'
        path.mkdir(parents=True)
        return path

    def expect_rejected(self, result, reason, mode):
        self.assertEqual(result.returncode, 2, f'{mode}: {result.stdout}{result.stderr}')
        self.assertIn(f'REJECTED {reason}:', result.stderr, mode)

    def expect_ok(self, result, mode):
        self.assertEqual(result.returncode, 0, f'{mode}: {result.stdout}{result.stderr}')
        return json.loads(result.stdout)

    def corpus_case(self, mutate, reason):
        """Every corpus rejection is checked in a fresh fixture under both interpreter modes."""
        for mode in MODES:
            with self.subTest(mode=mode):
                corpus, lock = make_corpus(self.fresh(f'{mode}-corpus'))
                mutate(corpus, lock)
                self.expect_rejected(run_tool(mode, 'validate-corpus', '--corpus', corpus, '--lock', lock), reason, mode)


class AN01SourceCorpus(Base):
    def test_selected_project_license_cannot_be_changed_or_omitted(self):
        for value in (None, 'Apache-2.0'):
            def mutate(corpus, lock):
                index = load_index(corpus)
                index['project_distribution_license'] = value
                save_index(corpus, index)
            self.corpus_case(mutate, 'unclassified')

    def test_changed_project_license_text_rejects(self):
        self.corpus_case(lambda c, l: write(c / 'LICENSE', b'changed project license\n'), 'changed')

    def test_valid_corpus_is_accepted_in_both_modes(self):
        summaries = []
        for mode in MODES:
            corpus, lock = make_corpus(self.fresh(mode))
            summaries.append(self.expect_ok(run_tool(mode, 'validate-corpus', '--corpus', corpus, '--lock', lock), mode))
        self.assertEqual(summaries[0], summaries[1])
        self.assertEqual(summaries[0]['result'], 'corpus-valid')

    def test_missing_text_rejects(self):
        self.corpus_case(lambda c, l: (c / 'rust/alpha-1.0.0/LICENSE').unlink(), 'missing')

    def test_changed_text_rejects(self):
        self.corpus_case(lambda c, l: write(c / 'native/clib-1.0/LICENSE-clib', b'edited\n'), 'changed')

    def test_changed_notice_rejects(self):
        self.corpus_case(lambda c, l: write(c / 'NOTICE.md', b'# edited\n'), 'changed')

    def test_unexpected_corpus_file_rejects(self):
        self.corpus_case(lambda c, l: write(c / 'rust/extra/LICENSE', b'x\n'), 'unexpected')

    def test_linked_package_without_text_rejects(self):
        def mutate(corpus, lock):
            index = load_index(corpus)
            alpha = index['packages'][0]
            (corpus / alpha['files'][0]['file']).unlink()
            alpha['files'] = []
            save_index(corpus, index)
        self.corpus_case(mutate, 'missing')

    def test_unclassified_lock_package_rejects(self):
        def mutate(corpus, lock):
            write(lock, lock_text(f'\n[[package]]\nname = "gamma"\nversion = "9.0.0"\nsource = "{REGISTRY}"\nchecksum = "{"f" * 64}"\n'))
            index = load_index(corpus)
            index['lock']['sha256'] = sha(lock.read_bytes())
            save_index(corpus, index)
        self.corpus_case(mutate, 'unclassified')

    def test_unattributed_graph_member_rejects(self):
        def mutate(corpus, lock):
            index = load_index(corpus)
            index['graph']['normal'].append('wasmonly 0.1.0')
            index['excluded_lock_packages'] = []
            save_index(corpus, index)
        self.corpus_case(mutate, 'unclassified')

    def test_unknown_role_rejects(self):
        def mutate(corpus, lock):
            index = load_index(corpus)
            index['packages'][3]['role'] = 'bundled_somehow'
            save_index(corpus, index)
        self.corpus_case(mutate, 'unclassified')

    def test_lock_bytes_mismatch_rejects(self):
        self.corpus_case(lambda c, l: write(l, l.read_text() + '\n'), 'lock-mismatch')

    def test_stale_lock_path_label_rejects(self):
        # The digest still matches: only the recorded path names a file that is not the bound lock.
        def mutate(corpus, lock):
            index = load_index(corpus)
            index['lock']['file'] = 'bindings/native/Cargo.lock'
            save_index(corpus, index)
        self.corpus_case(mutate, 'lock-mismatch')

    def test_lock_outside_the_recorded_path_rejects(self):
        # Identical bytes supplied from another location are not the lock the index names.
        for mode in MODES:
            corpus, lock = make_corpus(self.fresh(f'{mode}-elsewhere'))
            other = write(self.fresh(f'{mode}-other') / 'Cargo.lock', lock.read_bytes())
            self.expect_rejected(run_tool(mode, 'validate-corpus', '--corpus', corpus, '--lock', other), 'lock-mismatch', mode)

    def test_crate_checksum_mismatch_rejects(self):
        def mutate(corpus, lock):
            index = load_index(corpus)
            index['packages'][0]['checksum'] = '0' * 64
            save_index(corpus, index)
        self.corpus_case(mutate, 'lock-mismatch')

    def test_build_crate_claimed_linked_rejects(self):
        def mutate(corpus, lock):
            index = load_index(corpus)
            index['packages'][1]['role'] = 'linked_rust'
            save_index(corpus, index)
        self.corpus_case(mutate, 'wrong-role')

    def test_linked_libtomcrypt_on_apple_rejects(self):
        def mutate(corpus, lock):
            index = load_index(corpus)
            libtomcrypt = index['packages'][4]
            libtomcrypt['role'] = 'linked_native_c'
            text = write(corpus / 'native/libtomcrypt-1.18.2/LICENSE-libtomcrypt', b'ltc\n')
            libtomcrypt['files'] = [{'file': 'native/libtomcrypt-1.18.2/LICENSE-libtomcrypt', 'sha256': sha(text.read_bytes()),
                                     'origin': {'kind': 'synthetic'}}]
            save_index(corpus, index)
        self.corpus_case(mutate, 'wrong-platform')

    def test_ndk_aggregate_on_apple_rejects(self):
        def mutate(corpus, lock):
            index = load_index(corpus)
            ndk = dict(index['packages'][7], name='ndk-toolchain-runtime', version='27.1.12297006')
            ndk['verified_link_inputs'] = [{'target': 'aarch64-apple-ios', 'path': '/ndk/libunwind.a', 'sha256': 'e' * 64}]
            index['packages'].append(ndk)
            save_index(corpus, index)
        self.corpus_case(mutate, 'wrong-platform')

    def test_android_link_input_on_apple_rejects(self):
        def mutate(corpus, lock):
            index = load_index(corpus)
            index['packages'][7]['verified_link_inputs'][0]['target'] = 'aarch64-linux-android'
            save_index(corpus, index)
        self.corpus_case(mutate, 'wrong-platform')

    def test_android_runtime_role_on_apple_rejects(self):
        def mutate(corpus, lock):
            index = load_index(corpus)
            index['packages'][3]['role'] = 'kotlin_runtime'
            save_index(corpus, index)
        self.corpus_case(mutate, 'wrong-platform')

    def test_os_crypto_provider_must_be_recorded_separately(self):
        def missing(corpus, lock):
            index = load_index(corpus)
            index['packages'] = [p for p in index['packages'] if p['name'] != 'CommonCrypto']
            save_index(corpus, index)
        self.corpus_case(missing, 'provider-role')

        def redistributed(corpus, lock):
            index = load_index(corpus)
            security = index['packages'][6]
            security['files'] = index['packages'][3]['files']
            save_index(corpus, index)
        self.corpus_case(redistributed, 'provider-role')

    def test_libtomcrypt_symbols_in_evidence_reject(self):
        def mutate(corpus, lock):
            index = load_index(corpus)
            index['packages'][5]['evidence'][0]['libtomcrypt_symbols'] = 3
            save_index(corpus, index)
        self.corpus_case(mutate, 'wrong-platform')

    def test_unsafe_text_path_rejects(self):
        def mutate(corpus, lock):
            index = load_index(corpus)
            index['packages'][0]['files'][0]['file'] = '../outside/LICENSE'
            save_index(corpus, index)
        self.corpus_case(mutate, 'unsafe-path')


class AN02Framework(Base):
    def finalized(self, mode, name):
        base = self.fresh(name)
        corpus, lock = make_corpus(base)
        framework, parent = make_framework(base)
        summary = self.expect_ok(run_tool(mode, 'finalize', '--framework', framework, '--parent', parent,
                                          '--corpus', corpus, '--lock', lock), mode)
        return base, corpus, lock, framework, parent, summary

    def validate(self, mode, framework, corpus, lock, expected):
        return run_tool(mode, 'validate-framework', '--framework', framework, '--corpus', corpus, '--lock', lock,
                        '--expected-parent-sha256', expected)

    def test_notice_free_framework_is_rejected(self):
        for mode in MODES:
            base = self.fresh(f'{mode}-free')
            corpus, lock = make_corpus(base)
            framework, parent = make_framework(base)
            result = self.validate(mode, framework, corpus, lock, sha(parent.read_bytes()))
            self.expect_rejected(result, 'missing-packaging', mode)

    def test_finalize_adds_exact_closure_and_keeps_native_bytes(self):
        results = []
        for mode in MODES:
            base, corpus, lock, framework, parent, summary = self.finalized(mode, f'{mode}-final')
            members = native_members()
            actual = tree(framework)
            for relative, data in members.items():
                self.assertEqual(actual[relative], sha(data), relative)
            self.assertEqual((framework / 'native-build-parent.json').read_bytes(), parent.read_bytes())
            self.assertEqual((framework / 'NOTICE.md').read_bytes(), (corpus / 'NOTICE.md').read_bytes())
            self.assertEqual(tree(framework / 'third-party'), {k: v for k, v in tree(corpus).items() if k != 'NOTICE.md'})
            packaging = json.loads((framework / 'packaging.json').read_bytes())
            self.assertEqual(packaging['parent'], {'file': 'native-build-parent.json', 'sha256': sha(parent.read_bytes())})
            expected_members = {k: v for k, v in actual.items() if k != 'packaging.json'}
            self.assertEqual(packaging['members'], expected_members)
            self.assertNotIn('packaging.json', packaging['members'])
            self.assertEqual(len(packaging['members']), len(members) + 1 + 1 + len(tree(corpus)) - 1)
            self.expect_ok(self.validate(mode, framework, corpus, lock, sha(parent.read_bytes())), mode)
            results.append((actual, summary))
        self.assertEqual(results[0], results[1])

    def mutation_case(self, mutate, reason):
        for mode in MODES:
            with self.subTest(mode=mode):
                base, corpus, lock, framework, parent, _ = self.finalized(mode, f'{mode}-{reason}')
                expected = sha(parent.read_bytes())
                mutate(framework, corpus, parent)
                self.expect_rejected(self.validate(mode, framework, corpus, lock, expected), reason, mode)

    def test_removed_notice_rejects(self):
        self.mutation_case(lambda f, c, p: (f / 'third-party/rust/alpha-1.0.0/LICENSE').unlink(), 'notice-mismatch')

    def test_changed_notice_rejects(self):
        self.mutation_case(lambda f, c, p: write(f / 'NOTICE.md', b'# edited\n'), 'notice-mismatch')

    def test_unexpected_member_rejects(self):
        self.mutation_case(lambda f, c, p: write(f / 'third-party/rust/extra/LICENSE', b'x\n'), 'unexpected')

    def test_directory_existence_alone_cannot_pass(self):
        def hollow(framework, corpus, parent):
            shutil.rmtree(framework / 'third-party')
            (framework / 'third-party').mkdir()
        self.mutation_case(hollow, 'notice-mismatch')

    def test_changed_native_member_rejects(self):
        self.mutation_case(lambda f, c, p: write(f / 'ios-arm64/Headers/hoyadb.h', b'int changed(void);\n'), 'native-mismatch')

    def test_removed_packaging_record_rejects(self):
        self.mutation_case(lambda f, c, p: (f / 'packaging.json').unlink(), 'missing-packaging')

    def test_wrong_expected_parent_rejects(self):
        for mode in MODES:
            base, corpus, lock, framework, parent, _ = self.finalized(mode, f'{mode}-wrong-parent')
            self.expect_rejected(self.validate(mode, framework, corpus, lock, '0' * 64), 'parent-mismatch', mode)
            self.expect_rejected(self.validate(mode, framework, corpus, lock, 'not-a-digest'), 'invalid-input', mode)

    def test_consistent_record_rewrite_cannot_self_certify(self):
        """A changed library plus a rewritten parent copy and packaging record must still reject."""
        def rewrite(framework, corpus, parent):
            library = write(framework / 'ios-arm64/libhoyadb_native.a', b'!<arch> substituted\n')
            record = json.loads((framework / 'native-build-parent.json').read_bytes())
            record['artifact_files_sha256'][f'{NAME}/ios-arm64/libhoyadb_native.a'] = sha(library.read_bytes())
            copy = write(framework / 'native-build-parent.json', json.dumps(record, indent=2) + '\n')
            packaging = json.loads((framework / 'packaging.json').read_bytes())
            packaging['parent']['sha256'] = sha(copy.read_bytes())
            packaging['members'] = {k: v for k, v in tree(framework).items() if k != 'packaging.json'}
            write(framework / 'packaging.json', json.dumps(packaging, indent=2, sort_keys=True) + '\n')
        self.mutation_case(rewrite, 'parent-mismatch')

    def test_consistent_notice_rewrite_is_checked_against_source_corpus(self):
        def rewrite(framework, corpus, parent):
            write(framework / 'third-party/native/clib-1.0/LICENSE-clib', b'weakened\n')
            packaging = json.loads((framework / 'packaging.json').read_bytes())
            packaging['members'] = {k: v for k, v in tree(framework).items() if k != 'packaging.json'}
            write(framework / 'packaging.json', json.dumps(packaging, indent=2, sort_keys=True) + '\n')
        self.mutation_case(rewrite, 'notice-mismatch')

    def test_stale_provider_evidence_rejects(self):
        """The CommonCrypto/Security role is bound to the shipped library bytes, not just asserted."""
        for mode in MODES:
            base = self.fresh(f'{mode}-stale')
            corpus, lock = make_corpus(base, libraries={k: '9' * 64 for k in native_members() if k.endswith('.a')})
            framework, parent = make_framework(base)
            result = run_tool(mode, 'finalize', '--framework', framework, '--parent', parent, '--corpus', corpus, '--lock', lock)
            self.expect_rejected(result, 'stale-provider-evidence', mode)

    def test_finalize_refuses_unexpected_input_tree(self):
        for mode in MODES:
            base = self.fresh(f'{mode}-dirty')
            corpus, lock = make_corpus(base)
            framework, parent = make_framework(base)
            write(framework / 'stray.txt', b'x\n')
            result = run_tool(mode, 'finalize', '--framework', framework, '--parent', parent, '--corpus', corpus, '--lock', lock)
            self.expect_rejected(result, 'unexpected', mode)
            self.assertFalse((framework / 'packaging.json').exists(), mode)

    def test_finalize_refuses_invalid_corpus_before_writing(self):
        for mode in MODES:
            base = self.fresh(f'{mode}-bad-corpus')
            corpus, lock = make_corpus(base)
            (corpus / 'rust/beta-2.0.0/LICENSE').unlink()
            framework, parent = make_framework(base)
            before = tree(framework)
            result = run_tool(mode, 'finalize', '--framework', framework, '--parent', parent, '--corpus', corpus, '--lock', lock)
            self.expect_rejected(result, 'missing', mode)
            self.assertEqual(tree(framework), before, mode)


STUB = '''#!{python} -I
import os, plistlib, shutil, sys
from pathlib import Path
argv = sys.argv[1:]
name = Path(sys.argv[0]).name
with open(os.environ['HOYA_STUB_LOG'], 'a') as log:
    log.write(name + ' ' + ' '.join(argv) + '\\n')
if name == 'cargo':
    triple = argv[argv.index('--target') + 1] if '--target' in argv else 'x86_64-apple-darwin'
    manifest = Path(argv[argv.index('--manifest-path') + 1])
    if '--target' in argv:
        out = Path(os.environ['CARGO_TARGET_DIR']) / triple / 'release/libhoyadb_native.a'
    else:
        out = manifest.parent / 'target/release/libhoyadb_native.a'
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(b'!<arch> stub ' + triple.encode() + b'\\n')
elif name == 'xcrun' and argv[:1] == ['--sdk']:
    print('/stub/sdk/' + argv[1])
elif name == 'xcrun' and argv[:1] == ['lipo']:
    out = Path(argv[argv.index('-output') + 1])
    out.write_bytes(b''.join(Path(p).read_bytes() for p in argv[2:argv.index('-output')]))
elif name == 'xcodebuild':
    output = Path(argv[argv.index('-output') + 1])
    libraries = [Path(argv[i + 1]) for i, a in enumerate(argv) if a == '-library']
    headers = [Path(argv[i + 1]) for i, a in enumerate(argv) if a == '-headers']
    ids = []
    for library, header in zip(libraries, headers):
        text = str(library)
        identifier = ('ios-arm64_x86_64-simulator' if 'xcframework-simulator' in text else
                      'ios-arm64' if 'aarch64-apple-ios' in text else 'macos-x86_64')
        ids.append(identifier)
        (output / identifier).mkdir(parents=True)
        shutil.copyfile(library, output / identifier / library.name)
        shutil.copytree(header, output / identifier / 'Headers')
    (output / 'Info.plist').write_bytes(plistlib.dumps({{'AvailableLibraries': sorted(ids)}}))
else:
    sys.exit('unexpected stub call')
'''


def stub_library(triple):
    return b'!<arch> stub ' + triple.encode() + b'\n'


def stub_slices():
    simulator = stub_library('x86_64-apple-ios') + stub_library('aarch64-apple-ios-sim')
    return {'macos-x86_64/libhoyadb_native.a': sha(stub_library('x86_64-apple-darwin')),
            'ios-arm64/libhoyadb_native.a': sha(stub_library('aarch64-apple-ios')),
            'ios-arm64_x86_64-simulator/libhoyadb_native.a': sha(simulator)}


def rebuilt_layout(root):
    """A fresh repository layout carrying the actual build and notice tools plus compile-command stubs."""
    # build-artifacts.py hashes the root manifest too; without it the build stops before any assertion below.
    for relative in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'crates/hoyadb-native/Cargo.toml', 'crates/hoyadb-native/src/lib.rs']:
        write(root / relative, f'# fixture {relative}\n')
    for name in ['hoyadb-core', 'hoyadb-storage-memory', 'hoyadb-storage-sqlite', 'hoyadb-bindings']:
        write(root / 'crates' / name / 'Cargo.toml', '[package]\n')
        write(root / 'crates' / name / 'src/lib.rs', '// fixture\n')
    write(root / 'crates/hoyadb-native/include/hoyadb.h', b'int hoya_v1_abi_version(void);\n')
    write(root / 'crates/hoyadb-native/include/HoyaNative/module.modulemap', b'module HoyaNative {\n    header "../hoyadb.h"\n    export *\n}\n')
    corpus, lock = make_corpus(root / 'crates/hoyadb-native', libraries=stub_slices())
    shutil.copyfile(TOOL, write(root / 'crates/hoyadb-native/tools/apple_notices.py', b''))
    shutil.copyfile(BUILD, write(root / 'bindings/swift/build-artifacts.py', b''))
    stubs = root / 'stub-bin'
    for name in ['cargo', 'xcrun', 'xcodebuild']:
        write(stubs / name, STUB.format(python=sys.executable)).chmod(0o755)
    env = dict(os.environ, PATH=f'{stubs}{os.pathsep}{os.environ["PATH"]}', HOYA_STUB_LOG=str(root / 'stub.log'))
    return corpus, lock, env


class AN02BuildOrchestration(Base):
    def test_build_artifacts_always_finalizes_notices(self):
        results = []
        for mode in MODES:
            root = self.fresh(f'{mode}-repo')
            corpus, lock, env = rebuilt_layout(root)
            result = run_tool(mode, tool=root / 'bindings/swift/build-artifacts.py', env=env, cwd=root)
            self.assertEqual(result.returncode, 0, f'{mode}: {result.stdout}{result.stderr}')
            calls = (root / 'stub.log').read_text().splitlines()
            self.assertTrue(calls[-1].startswith('xcodebuild -create-xcframework'), calls)
            artifacts = root / 'bindings/swift/Artifacts'
            framework = artifacts / NAME
            parent = artifacts / 'native-build.json'
            recorded = json.loads(parent.read_bytes())['artifact_files_sha256']
            self.assertEqual(len(recorded), 10)
            self.assertTrue(all(k.startswith(NAME + '/') for k in recorded))
            self.assertTrue((framework / 'packaging.json').is_file(), f'{mode}: build left a notice-free framework')
            self.assertEqual((framework / 'native-build-parent.json').read_bytes(), parent.read_bytes())
            self.expect_ok(run_tool(mode, 'validate-framework', '--framework', framework, '--corpus', corpus, '--lock', lock,
                                    '--expected-parent-sha256', sha(parent.read_bytes())), mode)
            results.append({k: v for k, v in tree(framework).items() if k not in ('native-build-parent.json', 'packaging.json')})
        self.assertEqual(results[0], results[1])

    def test_build_artifacts_fails_without_a_valid_corpus(self):
        for mode in MODES:
            root = self.fresh(f'{mode}-repo-bad')
            corpus, lock, env = rebuilt_layout(root)
            write(corpus / 'NOTICE.md', b'# edited\n')
            result = run_tool(mode, tool=root / 'bindings/swift/build-artifacts.py', env=env, cwd=root)
            self.assertNotEqual(result.returncode, 0, mode)
            self.assertIn('changed', result.stderr, mode)
            self.assertNotIn('HOYA_APPLE_ARTIFACT_BUILD_GREEN', result.stdout, mode)
            self.assertFalse((root / 'bindings/swift/Artifacts' / NAME / 'packaging.json').exists(), mode)


def git(root, *args):
    """Fixture-only Git: no system/global configuration or hooks; a fixed identity."""
    env = dict(os.environ, GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull, GIT_AUTHOR_NAME='fixture',
               GIT_AUTHOR_EMAIL='fixture@invalid', GIT_COMMITTER_NAME='fixture', GIT_COMMITTER_EMAIL='fixture@invalid')
    result = subprocess.run(['git', '-C', str(root), *args], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
    if result.returncode != 0:
        raise RuntimeError(f'git {args}: {result.stderr}')
    return result.stdout


def pack_layout(root, finalize=True):
    """A fresh committed Git fixture: actual RN pack/notice tools, a package with tracked sources, the actual
    android/.npmignore, and a source framework. Untracked Gradle-like state is added after the commit."""
    native = root / 'crates/hoyadb-native'
    corpus, lock = make_corpus(native)
    shutil.copyfile(TOOL, write(native / 'tools/apple_notices.py', b''))
    framework, parent = make_framework(root / 'bindings/swift')
    if finalize:
        result = run_tool('normal', 'finalize', '--framework', framework, '--parent', parent, '--corpus', corpus, '--lock', lock)
        if result.returncode != 0:
            raise RuntimeError(result.stderr)
    package = root / 'packages/react-native'
    write(package / 'package.json', json.dumps({'name': '@hoyadb/fixture-rn', 'version': '0.0.1', 'private': True,
                                                 'files': ['index.js', 'android', 'Artifacts']}, indent=2) + '\n')
    write(package / 'index.js', 'export {};\n')
    write(package / 'android/src/Main.java', 'class Main {}\n')
    shutil.copyfile(NPMIGNORE, write(package / 'android/.npmignore', b''))
    write(package / '.gitignore', 'Artifacts/\n')
    shutil.copyfile(PACK, write(package / 'tools/pack.py', b''))
    git(root, 'init', '-q', '-b', 'main')
    git(root, 'add', 'packages/react-native')
    git(root, 'commit', '-q', '-m', 'fixture')
    write(package / 'android/.gradle/9.8.1/gc.properties', b'external state\n')
    write(package / 'android/build/intermediates/out.txt', b'build output\n')
    return package, framework, parent


class AN02ReactNativePack(Base):
    def npm_env(self, root, stub=False):
        config = write(root / 'npm-userconfig', b'')
        env = dict(os.environ, npm_config_cache=str(root / 'npm-cache'), npm_config_userconfig=str(config),
                   npm_config_offline='true', npm_config_update_notifier='false', npm_config_audit='false',
                   npm_config_fund='false')
        if stub:
            bin_dir = root / 'npm-stub'
            write(bin_dir / 'npm', f'#!{sys.executable} -I\nopen({str(root / "npm-called")!r}, "w").close()\n').chmod(0o755)
            env['PATH'] = f'{bin_dir}{os.pathsep}{env["PATH"]}'
        return env

    def pack(self, mode, root, package, framework, parent, env, expected=None):
        return run_tool(mode, '--source-framework', framework, '--expected-parent-sha256', expected or sha(parent.read_bytes()),
                        '--pack-destination', root / 'packed', '--extract-to', root / 'extracted', '--record', root / 'pack.json',
                        tool=package / 'tools/pack.py', env=env, cwd=root)

    def test_pack_validates_before_tar_and_after_extraction(self):
        for mode in MODES:
            root = self.fresh(f'{mode}-pack')
            package, framework, parent = pack_layout(root)
            result = self.pack(mode, root, package, framework, parent, self.npm_env(root))
            self.assertEqual(result.returncode, 0, f'{mode}: {result.stdout}{result.stderr}')
            record = json.loads((root / 'pack.json').read_bytes())
            self.assertEqual([v['label'] for v in record['validations']], ['source', 'package-copy', 'extracted'])
            archive = root / 'packed' / record['archive']['file']
            self.assertEqual(sha(archive.read_bytes()), record['archive']['sha256'])
            with tarfile.open(archive, 'r:gz') as packed:
                names = sorted(m.name for m in packed.getmembers() if m.isfile())
            framework_entries = sorted(n for n in names if n.startswith(f'package/Artifacts/{NAME}/'))
            self.assertEqual(framework_entries, sorted(f'package/Artifacts/{NAME}/{k}' for k in tree(framework)))
            self.assertEqual(tree(root / 'extracted/Artifacts' / NAME), tree(framework))

    def test_pack_accepts_paths_relative_to_the_caller(self):
        root = self.fresh('pack-relative')
        package, framework, parent = pack_layout(root)
        result = run_tool('normal', '--source-framework', framework.relative_to(root), '--expected-parent-sha256',
                          sha(parent.read_bytes()), '--pack-destination', 'packed', '--extract-to', 'extracted',
                          '--record', 'pack.json', tool=package / 'tools/pack.py', env=self.npm_env(root), cwd=root)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(tree(root / 'extracted/Artifacts' / NAME), tree(framework))

    def test_pack_refuses_notice_free_source_before_npm(self):
        for mode in MODES:
            root = self.fresh(f'{mode}-pack-free')
            package, framework, parent = pack_layout(root, finalize=False)
            result = self.pack(mode, root, package, framework, parent, self.npm_env(root, stub=True))
            self.expect_rejected(result, 'missing-packaging', mode)
            self.assertFalse((root / 'npm-called').exists(), mode)
            self.assertFalse((package / 'Artifacts').exists(), mode)

    def test_pack_requires_the_trusted_parent_digest(self):
        for mode in MODES:
            root = self.fresh(f'{mode}-pack-parent')
            package, framework, parent = pack_layout(root)
            result = self.pack(mode, root, package, framework, parent, self.npm_env(root, stub=True), expected='1' * 64)
            self.expect_rejected(result, 'parent-mismatch', mode)
            self.assertFalse((root / 'npm-called').exists(), mode)

    def test_pack_refuses_to_overwrite_an_existing_archive(self):
        root = self.fresh('pack-exists')
        package, framework, parent = pack_layout(root)
        write(root / 'packed/fixture-rn-0.0.1.tgz', b'previous archive\n')
        write(root / 'packed/hoyadb-fixture-rn-0.0.1.tgz', b'previous archive\n')
        result = self.pack('normal', root, package, framework, parent, self.npm_env(root, stub=True))
        self.expect_rejected(result, 'output-exists', 'normal')
        self.assertFalse((root / 'npm-called').exists())


class AN02CheckArtifacts(Base):
    """check_artifacts.py verifies the closure against the trusted parent digest; no packed-copy skip."""

    def check(self, mode, *args):
        root = self.fresh(f'{mode}-check')
        native = root / 'crates/hoyadb-native'
        corpus, lock = make_corpus(native)
        shutil.copyfile(TOOL, write(native / 'tools/apple_notices.py', b''))
        checker = write(root / 'bindings/swift/check_artifacts.py', b'')
        shutil.copyfile(CHECK, checker)
        framework, parent = make_framework(root / 'packed-copy')
        finalized = run_tool('normal', 'finalize', '--framework', framework, '--parent', parent, '--corpus', corpus, '--lock', lock)
        self.assertEqual(finalized.returncode, 0, finalized.stderr)
        parent.unlink()  # A packed copy carries no adjacent native-build.json.
        values = {'parent': sha((framework / 'native-build-parent.json').read_bytes())}
        argv = [a.format(**values) for a in args] + [framework, 'XCFrameworkPackaging.test_notice_packaging_closure']
        return run_tool(mode, *argv, tool=checker, cwd=root)

    def test_trusted_parent_digest_passes(self):
        for mode in MODES:
            result = self.check(mode, '--expected-parent-sha256', '{parent}')
            self.assertEqual(result.returncode, 0, f'{mode}: {result.stdout}{result.stderr}')
            self.assertIn('test_notice_packaging_closure', result.stderr)
            self.assertNotIn('skipped', result.stderr)

    def test_missing_or_wrong_parent_digest_fails_instead_of_skipping(self):
        for mode in MODES:
            for args, message in [((), 'expected compilation parent SHA256 is required'),
                                  (('--expected-parent-sha256', '0' * 64), 'parent-mismatch')]:
                result = self.check(mode, *args)
                self.assertEqual(result.returncode, 1, f'{mode} {args}: {result.stdout}{result.stderr}')
                self.assertNotIn('skipped', result.stderr)
                self.assertIn('FAIL: test_notice_packaging_closure', result.stderr)
                self.assertIn(message, result.stderr)


class AN05PackSourceIdentity(Base):
    """The actual pack tool, pointed at a temp Git fixture through explicit roots, accepts only HEAD-identical
    package sources plus the exact validated XCFramework, and rejects before extraction otherwise."""

    def run_pack(self, mode, root, package, parent, stub=False, repo_root=None, extra_env=None, npm_before=None):
        env = AN02ReactNativePack.npm_env(self, root, stub=stub)
        env.update(extra_env or {})
        if npm_before is not None:
            # Real npm wrapped by a step that runs after the Git root/commit is resolved and before the members are checked.
            real_npm = shutil.which('npm')
            bin_dir = root / 'npm-wrapper'
            write(bin_dir / 'step.py', npm_before)
            write(bin_dir / 'npm', f'#!/bin/sh\n{sys.executable} -I {bin_dir / "step.py"} || exit 99\nexec {real_npm} "$@"\n').chmod(0o755)
            env['PATH'] = f'{bin_dir}{os.pathsep}{env["PATH"]}'
        return run_tool(mode, '--package-root', package, '--repo-root', repo_root or root,
                        '--source-framework', root / 'bindings/swift/Artifacts' / NAME,
                        '--expected-parent-sha256', sha(parent.read_bytes()), '--pack-destination', root / 'packed',
                        '--extract-to', root / 'extracted', '--record', root / 'pack.json', tool=PACK, env=env, cwd=REPO)

    def archive_files(self, root):
        archives = list((root / 'packed').glob('*.tgz'))
        self.assertEqual(len(archives), 1, archives)
        with tarfile.open(archives[0], 'r:gz') as packed:
            return {m.name: packed.extractfile(m).read() for m in packed.getmembers() if m.isfile()}

    def rejected_before_extraction(self, mode, root, package, parent, reason, npm_before=None):
        result = self.run_pack(mode, root, package, parent, npm_before=npm_before)
        self.expect_rejected(result, reason, mode)
        self.assertFalse((root / 'extracted').exists(), mode)
        self.assertEqual(len(list((root / 'packed').glob('*.tgz'))), 1, f'{mode}: failure archive must be preserved')
        record = json.loads((root / 'pack.json').read_bytes())
        self.assertIn(reason, record['error'])
        self.assertIn('archive', record)

    def test_shipped_npmignore_excludes_only_gradle_and_build_state(self):
        self.assertEqual(NPMIGNORE.read_bytes(), b'.gradle/\nbuild/\n')

    def test_head_identical_sources_and_validated_framework_pass(self):
        for mode in MODES:
            root = self.fresh(f'{mode}-identity')
            package, framework, parent = pack_layout(root)
            result = self.run_pack(mode, root, package, parent)
            self.assertEqual(result.returncode, 0, f'{mode}: {result.stdout}{result.stderr}')
            files = self.archive_files(root)
            prefix = f'package/Artifacts/{NAME}/'
            sources = {k: v for k, v in files.items() if not k.startswith(prefix)}
            self.assertEqual(sorted(sources), ['package/android/src/Main.java', 'package/index.js', 'package/package.json'])
            for name, data in sources.items():
                head = git(root, 'cat-file', 'blob', 'HEAD:packages/react-native/' + name.removeprefix('package/'))
                self.assertEqual(data, head.encode(), name)
            self.assertEqual(files['package/package.json'], (package / 'package.json').read_bytes())
            self.assertEqual({k.removeprefix(prefix) for k in files if k.startswith(prefix)}, set(tree(framework)))
            record = json.loads((root / 'pack.json').read_bytes())
            self.assertEqual(record['source_identity']['head_identical'], sorted(sources))
            self.assertEqual(tree(root / 'extracted/Artifacts' / NAME), tree(framework))

    def test_untracked_package_file_rejects_before_extraction(self):
        for mode in MODES:
            root = self.fresh(f'{mode}-untracked')
            package, framework, parent = pack_layout(root)
            write(package / 'android/src/Untracked.java', 'class Untracked {}\n')
            self.rejected_before_extraction(mode, root, package, parent, 'unexpected-entry')

    def test_tracked_but_modified_file_rejects_before_extraction(self):
        for mode in MODES:
            root = self.fresh(f'{mode}-modified')
            package, framework, parent = pack_layout(root)
            write(package / 'index.js', 'export const changed = 1;\n')
            self.rejected_before_extraction(mode, root, package, parent, 'modified-entry')

    def test_generated_member_outside_the_validated_framework_rejects(self):
        for mode in MODES:
            root = self.fresh(f'{mode}-stray')
            package, framework, parent = pack_layout(root)
            write(package / 'Artifacts/stray.txt', b'not the framework\n')
            self.rejected_before_extraction(mode, root, package, parent, 'unexpected-entry')

    def head_move_step(self, root, restore_original):
        """Commit B (changed index.js) during npm; optionally restore the original A bytes in the work tree."""
        return (f'import os, subprocess\nroot = {str(root)!r}\nenv = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull, '
                f'GIT_AUTHOR_NAME="f", GIT_AUTHOR_EMAIL="f@invalid", GIT_COMMITTER_NAME="f", GIT_COMMITTER_EMAIL="f@invalid")\n'
                f'path = os.path.join(root, "packages/react-native/index.js")\n'
                f'original = open(path, "rb").read()\n'
                f'open(path, "w").write("export const movedHead = 1;\\n")\n'
                f'subprocess.run(["git", "-C", root, "commit", "-q", "-am", "B"], env=env, check=True)\n'
                + ('open(path, "wb").write(original)\n' if restore_original else ''))

    def test_head_moving_to_new_bytes_cannot_borrow_the_pinned_commit(self):
        for mode in MODES:
            root = self.fresh(f'{mode}-moved-b')
            package, framework, parent = pack_layout(root)
            pinned = git(root, 'rev-parse', 'HEAD').strip()
            self.rejected_before_extraction(mode, root, package, parent, 'modified-entry',
                                            npm_before=self.head_move_step(root, restore_original=False))
            self.assertNotEqual(git(root, 'rev-parse', 'HEAD').strip(), pinned, f'{mode}: HEAD did not move')
            self.assertEqual(json.loads((root / 'pack.json').read_bytes())['head_commit'], pinned)

    def test_pinned_commit_bytes_still_verify_after_head_moves(self):
        for mode in MODES:
            root = self.fresh(f'{mode}-moved-a')
            package, framework, parent = pack_layout(root)
            pinned = git(root, 'rev-parse', 'HEAD').strip()
            result = self.run_pack(mode, root, package, parent, npm_before=self.head_move_step(root, restore_original=True))
            self.assertEqual(result.returncode, 0, f'{mode}: {result.stdout}{result.stderr}')
            self.assertNotEqual(git(root, 'rev-parse', 'HEAD').strip(), pinned, f'{mode}: HEAD did not move')
            record = json.loads((root / 'pack.json').read_bytes())
            self.assertEqual(record['head_commit'], pinned)
            self.assertIn('package/index.js', record['source_identity']['head_identical'])

    def test_inherited_git_environment_cannot_redirect_explicit_roots(self):
        foreign = self.fresh('foreign')
        write(foreign / 'packages/react-native/index.js', 'export const foreign = 1;\n')
        git(foreign, 'init', '-q', '-b', 'main')
        git(foreign, 'add', '.')
        git(foreign, 'commit', '-q', '-m', 'foreign')
        hostile = {'GIT_DIR': str(foreign / '.git'), 'GIT_WORK_TREE': str(foreign)}
        for mode in MODES:
            root = self.fresh(f'{mode}-git-env')
            package, framework, parent = pack_layout(root)
            # Control: the synthetic variables really do redirect plain Git away from the fixture.
            control = subprocess.run(['git', '-C', str(package), 'rev-parse', '--show-toplevel'], stdout=subprocess.PIPE,
                                     text=True, env=dict(os.environ, **hostile))
            self.assertEqual(Path(control.stdout.strip()).resolve(), foreign.resolve())
            result = self.run_pack(mode, root, package, parent, extra_env=hostile)
            self.assertEqual(result.returncode, 0, f'{mode}: {result.stdout}{result.stderr}')
            record = json.loads((root / 'pack.json').read_bytes())
            self.assertEqual(record['head_commit'], git(root, 'rev-parse', 'HEAD').strip())

    def test_wrong_git_root_rejects_before_npm(self):
        for mode in MODES:
            root = self.fresh(f'{mode}-root')
            package, framework, parent = pack_layout(root)
            result = self.run_pack(mode, root, package, parent, stub=True, repo_root=root / 'bindings')
            self.expect_rejected(result, 'git-root-mismatch', mode)
            self.assertFalse((root / 'npm-called').exists(), mode)
            self.assertFalse((package / 'Artifacts').exists(), mode)


class AN03Static(unittest.TestCase):
    def test_tools_use_explicit_checks_only(self):
        for path in [TOOL, PACK]:
            source = path.read_text()
            asserts = [n.lineno for n in ast.walk(ast.parse(source)) if isinstance(n, ast.Assert)]
            self.assertEqual(asserts, [], f'{path.name} uses assert statements')

    def test_build_script_uses_the_shared_finalizer(self):
        source = BUILD.read_text()
        self.assertNotIn('assert ', source)
        self.assertIn('apple_notices.py', source)


if __name__ == '__main__':
    unittest.main(verbosity=2)
