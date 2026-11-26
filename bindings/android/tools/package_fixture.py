"""Synthetic complete final-packaging fixture; nothing here is ever executed or linked."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tarfile
import zipfile

_SPEC = importlib.util.spec_from_file_location(
    'hoyadb_rust_sources', Path(__file__).resolve().parents[3] / 'crates/hoyadb-native/android/rust_sources.py')
RUST = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(RUST)

EXPORTS = ['hoya_v1_abi_version', 'Java_dev_hoyadb_NativeBridge_abiVersion']
MACHINE = {'x86_64': 'Advanced Micro Devices X86-64', 'arm64-v8a': 'AArch64',
           'armeabi-v7a': 'ARM', 'x86': 'Intel 80386'}
ELF_CLASS = {'x86_64': 'ELF64', 'arm64-v8a': 'ELF64', 'armeabi-v7a': 'ELF32', 'x86': 'ELF32'}
CRATE = 'sqlcipher-src-1.0.0+synthetic'
POM_LICENSE = b'<licenses><license><name>The Apache License, Version 2.0</name><url>http://www.apache.org/licenses/LICENSE-2.0.txt</url></license></licenses>'


def sha(data):
    return hashlib.sha256(data if isinstance(data, bytes) else Path(data).read_bytes()).hexdigest()


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data if isinstance(data, bytes) else data.encode())
    return path


class Fixture:
    def __init__(self, root, abis=('x86_64', 'arm64-v8a')):
        self.root = root
        self.android = root / 'bindings/android'
        self.crates = root / 'cargo-cache'
        self.readelf = write(root / 'ndk/llvm-readelf', b'synthetic readelf; never executed')
        self.elf = {abi: {'class': ELF_CLASS[abi], 'machine': MACHINE[abi], 'align': '0x4000', 'exports': list(EXPORTS),
                          'needed': ['libdl.so', 'libm.so', 'libc.so'], 'exit': 0} for abi in abis}
        self.readelf_calls = []
        self.sources()
        self.native()
        self.notices()
        self.link_native()
        self.classes = self.build()
        self.out = root / 'final-repo'

    def sources(self):
        write(self.android / 'src/main/kotlin/dev/hoyadb/Values.kt', 'synthetic main source')
        write(self.android / 'src/main/java/dev/hoyadb/NativeBridge.java', 'synthetic bridge source')
        write(self.android / 'src/test/kotlin/dev/hoyadb/SdkTests.kt', 'synthetic test source')
        write(self.android / 'tools/build.py', 'synthetic tool')
        runtime = []
        for coordinate, name in [('org.jetbrains.kotlin:kotlin-stdlib:2.1.20', 'kotlin-stdlib-2.1.20.jar'),
                                 ('org.jetbrains:annotations:13.0', 'annotations-13.0.jar')]:
            jar = write(self.root / 'deps' / name, b'synthetic runtime ' + name.encode())
            runtime.append({'coordinate': coordinate, 'path': str(jar), 'sha256': sha(jar)})
        write(self.android / 'toolchain-review.json', json.dumps({'runtime_artifacts': runtime, 'installed_tool_closure': {}}))

    def native(self):
        self.native_input = write(self.root / 'crates/hoyadb-native/android/hoya_jni.c', 'synthetic native source')
        write(self.root / 'crates/hoyadb-native/android/exports.map',
              'HOYADB_1 {\n  global:\n' + ''.join('    ' + name + ';\n' for name in EXPORTS) + '  local: *;\n};\n')
        rust = {'Cargo.toml': '[workspace]\n', 'rust-toolchain.toml': '[toolchain]\n',
                'crates/hoyadb-core/Cargo.toml': '[package]\n', 'crates/hoyadb-core/src/lib.rs': '// synthetic core\n',
                'crates/hoyadb-bindings/src/lib.rs': '// synthetic facade\n',
                'crates/hoyadb-storage-sqlite/build.rs': '// synthetic build script\n',
                'crates/hoyadb-native/include/hoyadb.h': '/* synthetic header */\n'}
        for relative, text in rust.items():
            write(self.root / relative, text)
        self.native_abis = list(self.elf)

    def link_native(self):
        """Record the evidence a JNI link writes; called once the lock files exist."""
        self.rust_sources = RUST.rust_sources(self.root)
        for abi in self.native_abis:
            library = write(self.root / 'target/phase-seven-native/jni-current' / abi / 'libhoyadb_jni.so',
                            b'synthetic native artifact ' + abi.encode())
            write(library.with_name('evidence.json'), json.dumps({
                'abi': abi, 'min_sdk': 24, 'artifact_sha256': sha(library),
                'inputs_sha256': {'crates/hoyadb-native/android/hoya_jni.c': sha(self.native_input)},
                'rust_sources_sha256': self.rust_sources}))

    def crate(self):
        members = {CRATE + '/sqlcipher/LICENSE-sqlcipher': b'synthetic sqlcipher license\n',
                   CRATE + '/sqlcipher/LICENSE-libtomcrypt': b'synthetic libtomcrypt license\n',
                   CRATE + '/sqlcipher/sqlite3.h': b'/* synthetic blessing */\nint sqlite3_synthetic;\n'}
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode='w:gz') as archive:
            for name, data in members.items():
                info = tarfile.TarInfo(name)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        return write(self.crates / (CRATE + '.crate'), buffer.getvalue())

    def notices(self):
        crate = self.crate()
        alpha = ['alpha', '1.0.0', 'registry+https://github.com/rust-lang/crates.io-index', '1' * 64]
        tool = ['beta-tool', '2.0.0', 'registry+https://github.com/rust-lang/crates.io-index', '2' * 64]
        source = ['sqlcipher-src', '1.0.0+synthetic', 'registry+https://github.com/rust-lang/crates.io-index', sha(crate)]
        inactive = ['gamma', '3.0.0', 'registry+https://github.com/rust-lang/crates.io-index', '3' * 64]
        active = [alpha, tool, source]
        lock = ''.join('[[package]]\nname = "%s"\nversion = "%s"\nsource = "%s"\nchecksum = "%s"\n\n' % tuple(item) for item in [*active, inactive])
        write(self.root / 'Cargo.lock', lock)
        write(self.root / 'crates/hoyadb-native/Cargo.lock', lock)
        write(self.root / 'licenses/native-dependencies.json', json.dumps({'packages': [
            {'name': n, 'version': v, 'source': s, 'checksum': c, 'active_native': True} for n, v, s, c in active]
            + [{'name': inactive[0], 'version': inactive[1], 'source': inactive[2], 'checksum': inactive[3], 'active_native': False}]}))
        corpus = self.root / 'licenses/javascript'
        js, packages = [], []
        third = self.android / 'third-party'
        for (name, version, src, checksum), role, license_ in [(alpha, 'linked_rust', 'MIT'), (tool, 'build_tool', 'MIT OR Apache-2.0'),
                                                              (source, 'build_native_c_source', 'MIT')]:
            relative = name + '-' + version + '/LICENSE'
            text = write(corpus / relative, 'synthetic license text for ' + name + '\n').read_bytes()
            js.append({'name': name, 'version': version, 'source': src, 'checksum': checksum, 'license': license_,
                       'files': [{'file': relative, 'sha256': sha(text)}]})
            write(third / 'rust' / relative, text)
            packages.append({'name': name, 'version': version, 'source': src, 'checksum': checksum, 'license': license_,
                             'role': role, 'files': [{'file': 'rust/' + relative, 'sha256': sha(text),
                                                      'origin': {'kind': 'js_corpus', 'file': relative}}]})
        write(corpus / 'index.json', json.dumps({'packages': js}))
        for name, member, kind, data in [
                ('SQLCipher', 'sqlcipher/LICENSE-sqlcipher', 'crate_entry', b'synthetic sqlcipher license\n'),
                ('SQLite', 'sqlcipher/sqlite3.h', 'crate_entry_prefix', b'/* synthetic blessing */\n'),
                ('LibTomCrypt', 'sqlcipher/LICENSE-libtomcrypt', 'crate_entry', b'synthetic libtomcrypt license\n')]:
            relative = 'native/' + name + '/LICENSE'
            write(third / relative, data)
            packages.append({'name': name, 'version': '1', 'license': 'synthetic', 'role': 'linked_native_c', 'via': 'sqlcipher-src',
                             'files': [{'file': relative, 'sha256': sha(data), 'origin': {'kind': kind, 'crate': 'sqlcipher-src', 'entry': member}}]})
        review = json.loads((self.android / 'toolchain-review.json').read_text())
        for artifact in review['runtime_artifacts']:
            name = artifact['coordinate'].split(':')[1]
            pom = write(self.root / 'poms' / (name + '.pom'), b'<project>' + POM_LICENSE + b'</project>')
            text = write(third / 'kotlin' / name / 'LICENSE.txt', b'synthetic apache text\n').read_bytes()
            packages.append({'name': name, 'version': artifact['coordinate'].split(':')[2], 'coordinate': artifact['coordinate'],
                             'license': 'Apache-2.0', 'role': 'kotlin_runtime', 'jar_sha256': artifact['sha256'],
                             'pom': str(pom.relative_to(self.root)), 'pom_sha256': sha(pom),
                             'files': [{'file': 'kotlin/' + name + '/LICENSE.txt', 'sha256': sha(text),
                                        'origin': {'kind': 'upstream_url', 'url': 'https://synthetic.invalid/LICENSE'}}]})
        toolchain = self.root / 'toolchains/rust-1.98.1'
        shipped = write(toolchain / 'share/doc/rust/COPYRIGHT-library.html', b'<p>synthetic aggregate library copyright</p>\n')
        self.link_input = write(toolchain / 'lib/rustlib/x86_64-linux-android/lib/libstd-0000000000000000.rlib', b'synthetic rlib; never linked')
        relative = 'toolchain/rust-1.98.1/COPYRIGHT-library.html'
        write(third / relative, shipped.read_bytes())
        packages.append({'name': 'rust-std', 'version': '1.98.1', 'license': 'aggregate (see text)', 'role': 'toolchain_aggregate',
                         'verified_link_inputs': [{'path': str(self.link_input), 'sha256': sha(self.link_input)}],
                         'files': [{'file': relative, 'sha256': sha(shipped), 'origin': {'kind': 'toolchain_local', 'path': str(shipped)}}]})
        self.index = third / 'index.json'
        project_license = (Path(__file__).resolve().parents[3] / 'LICENSE').read_bytes()
        write(third / 'LICENSE', project_license)
        self.write_index({'packages': packages, 'project_distribution_license': 'MIT',
                          'project_license_sha256': sha(project_license), 'lock_sha256': {
            name: sha(self.root / name) for name in ['Cargo.lock', 'crates/hoyadb-native/Cargo.lock']}})
        write(self.android / 'NOTICE.md', 'synthetic notice summary\n')

    def write_index(self, index):
        write(self.index, json.dumps(index, indent=2))

    def load_index(self):
        return json.loads(self.index.read_text())

    def production_sources(self):
        android = self.android
        paths = [*sorted((android / 'src/main/kotlin').rglob('*.kt')), *sorted((android / 'src/main/java').rglob('*.java')),
                 android / 'toolchain-review.json', *sorted((android / 'tools').glob('*.py'))]
        return {str(path.relative_to(self.root)): sha(path) for path in sorted(paths)}

    def build(self, entries=('dev/hoyadb/Values.class', 'dev/hoyadb/NativeBridge.class')):
        build = self.root / 'build'
        build.mkdir(exist_ok=True)
        classes = build / 'classes.jar'
        with zipfile.ZipFile(classes, 'w') as archive:
            for entry in entries:
                archive.writestr(entry, b'synthetic class')
        sources = self.production_sources()
        write(build / 'manifest.json', json.dumps({
            'compiler': 'installed2.2.20', 'language_api': '2.1', 'commands': [], 'classes_sha256': sha(classes),
            'test_classes_included': False, 'split_runtime': False, 'runtime_dex': None, 'dex_sha256': None,
            'sources_before': sources, 'sources_after': sources, 'tools_sha256': {}}))
        return classes

    def execute(self, command, **kwargs):
        self.readelf_calls.append(command)
        library = Path(command[-1])
        abi = library.parent.name
        elf = self.elf[abi]
        flags = command[1:-1]
        if flags == ['-h']:
            text = 'ELF Header:\n  Class:                             %s\n  Type:                              DYN (Shared object file)\n  Machine:                           %s\n' % (elf['class'], elf['machine'])
        elif flags == ['-lW']:
            text = ('Program Headers:\n  Type           Offset   VirtAddr           PhysAddr           FileSiz  MemSiz   Flg Align\n'
                    '  LOAD           0x000000 0x0000000000000000 0x0000000000000000 0x11a474 0x11a474 R   %s\n'
                    '  LOAD           0x11a480 0x000000000011e480 0x000000000011e480 0x504da0 0x504da0 R E 0x4000\n'
                    '  DYNAMIC        0x62e6c0 0x00000000006366c0 0x00000000006366c0 0x0001a0 0x0001a0 RW  0x8\n') % elf['align']
        elif flags == ['--dyn-syms', '-W']:
            text = "Symbol table '.dynsym' contains entries:\n   Num:    Value          Size Type    Bind   Vis       Ndx Name\n"
            text += '     0: 0000000000000000     0 NOTYPE  LOCAL  DEFAULT   UND \n'
            text += '     1: 0000000000000000     0 FUNC    GLOBAL DEFAULT   UND malloc@LIBC (2)\n'
            for number, name in enumerate(elf['exports'], 2):
                text += '   %3d: 0000000000120510     6 FUNC    GLOBAL DEFAULT    14 %s@@HOYADB_1\n' % (number, name)
        elif flags == ['-dW']:
            text = 'Dynamic section at offset 0x62e6c0 contains entries:\n  Tag                Type           Name/Value\n'
            text += ''.join('  0x0000000000000001 (NEEDED)       Shared library: [%s]\n' % name for name in elf['needed'])
            text += '  0x0000000000000000 (NULL)         0x0\n'
        else:
            raise AssertionError('unexpected synthetic readelf command')
        return subprocess.CompletedProcess(command, elf['exit'], text.encode(), b'')
