#!/usr/bin/env python3
"""Manual resource-free AAR packaging; requires a production-only build manifest.

`--development --out X.aar` writes a development-only AAR. Without it, `--out`
is a fresh local Maven repository holding the final certificate (PACKAGING_CASES).
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tarfile
import tomllib
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[3]
READELF = Path('/Users/boristacyniak/Library/Android/sdk/ndk/27.1.12297006/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-readelf')
CRATES = Path.home() / '.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f'
ABIS = ['x86_64', 'arm64-v8a']
MACHINE = {'x86_64': 'Advanced Micro Devices X86-64', 'arm64-v8a': 'AArch64',
           'armeabi-v7a': 'ARM', 'x86': 'Intel 80386'}
ELF_CLASS = {'x86_64': 'ELF64', 'arm64-v8a': 'ELF64', 'armeabi-v7a': 'ELF32', 'x86': 'ELF32'}
NEEDED = {'libc.so', 'libdl.so', 'libm.so'}
RUST_ROLES = {'linked_rust', 'build_tool', 'build_native_c_source'}
ROLES = RUST_ROLES | {'linked_native_c', 'kotlin_runtime', 'toolchain_aggregate'}
APACHE_URL = '<url>http://www.apache.org/licenses/LICENSE-2.0.txt</url>'
LOCKS = ['Cargo.lock', 'crates/hoyadb-native/Cargo.lock']
RUST_SOURCES = Path(__file__).resolve().parents[3] / 'crates/hoyadb-native/android/rust_sources.py'
_SPEC = importlib.util.spec_from_file_location('hoyadb_rust_sources', RUST_SOURCES)
RUST = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(RUST)
VERSION = '0.1.0'
BASE = 'hoyadb-android-' + VERSION
MANIFEST = b'<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="dev.hoyadb"><uses-sdk android:minSdkVersion="24" /></manifest>\n'
KEEP = b'-keep class dev.hoyadb.NativeBridge { *; }\n'
POM = '''<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
  <modelVersion>4.0.0</modelVersion>
  <groupId>dev.hoyadb</groupId>
  <artifactId>hoyadb-android</artifactId>
  <version>0.1.0</version>
  <packaging>aar</packaging>
  <name>HoyaDB Android</name>
  <description>HoyaDB Android SDK. First-party code uses MIT; third-party terms are retained in META-INF/hoyadb-android/NOTICE.md.</description>
  <dependencies>
    <dependency>
      <groupId>org.jetbrains.kotlin</groupId>
      <artifactId>kotlin-stdlib</artifactId>
      <version>2.1.20</version>
      <scope>compile</scope>
    </dependency>
  </dependencies>
</project>
'''


def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def digest(data): return hashlib.sha256(data).hexdigest()
def require(value, reason):
    if not value: raise SystemExit(reason)


def native_slices(abis=ABIS):
    """P01: current SO bytes, every recorded input and the Rust source closure match the native evidence."""
    slices = {}
    # The archive digest alone cannot show which Rust sources it was compiled from.
    rust = RUST.rust_sources(ROOT)
    for abi in abis:
        path = ROOT / 'target/phase-seven-native/jni-current' / abi / 'libhoyadb_jni.so'
        record = path.with_name('evidence.json')
        require(not path.is_symlink() and path.is_file() and not record.is_symlink() and record.is_file(), 'native artifact missing')
        try:
            evidence = json.loads(record.read_text())
        except ValueError:
            evidence = None
        require(isinstance(evidence, dict), 'native evidence profile mismatch')
        require(evidence.get('abi') == abi and evidence.get('min_sdk') == 24, 'native evidence profile mismatch')
        require(sha(path) == evidence.get('artifact_sha256'), 'native artifact digest mismatch')
        inputs = evidence.get('inputs_sha256')
        require(isinstance(inputs, dict) and inputs, 'native evidence inputs missing')
        for name, expected in inputs.items():
            source = Path(name)
            if not source.is_absolute(): source = ROOT / source
            require(source.is_file() and not source.is_symlink(), 'native input missing')
            require(sha(source) == expected, 'native input digest mismatch')
        recorded = evidence.get('rust_sources_sha256')
        require(isinstance(recorded, dict) and recorded, 'native Rust source closure missing')
        require(recorded == rust, 'native Rust source closure mismatch')
        slices[abi] = {'path': path, 'sha256': sha(path), 'native_evidence': str(path.with_name('evidence.json')),
                       'evidence_sha256': sha(path.with_name('evidence.json')), 'inputs_sha256': inputs,
                       'rust_sources_sha256': recorded}
    return slices


def inspect_elf(abi, path):
    """P02: independent installed llvm-readelf over the hash-bound SO."""
    inspections, outputs = [], {}
    for key, flags in [('header', ['-h']), ('segments', ['-lW']), ('symbols', ['--dyn-syms', '-W']), ('dynamic', ['-dW'])]:
        command = [str(READELF), *flags, str(path)]
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
        outputs[key] = result.stdout.decode('utf8', errors='replace')
        inspections.append({'command': command, 'exit': result.returncode, 'stdout_sha256': digest(result.stdout)})
    require(all(item['exit'] == 0 for item in inspections), 'native ELF inspection failed')
    header = {line.split(':', 1)[0].strip(): line.split(':', 1)[1].strip() for line in outputs['header'].splitlines() if ':' in line}
    require(header.get('Class') == ELF_CLASS[abi] and header.get('Type', '').startswith('DYN') and header.get('Machine') == MACHINE[abi], 'native ELF machine mismatch')
    loads = [line.split() for line in outputs['segments'].splitlines() if line.split()[:1] == ['LOAD']]
    require(loads and all(len(fields) >= 8 for fields in loads), 'native ELF inspection failed')
    require(all(int(fields[-1], 16) >= 0x4000 for fields in loads), 'native ELF LOAD alignment below 16KiB')
    exports = []
    for line in outputs['symbols'].splitlines():
        fields = line.split()
        if len(fields) >= 8 and fields[0].endswith(':') and fields[6] != 'UND' and fields[4] in ('GLOBAL', 'WEAK'):
            exports.append(fields[7].split('@')[0])
    require(sorted(exports) == exported_symbols(), 'native ELF exports differ from exports.map')
    needed = [line.split('[', 1)[1].split(']', 1)[0] for line in outputs['dynamic'].splitlines() if '(NEEDED)' in line]
    require(set(needed) <= NEEDED, 'native ELF unexpected NEEDED dependency')
    return {'inspections': inspections, 'exports': sorted(exports), 'needed': sorted(needed),
            'load_alignments': [fields[-1] for fields in loads]}


def exported_symbols():
    text = (ROOT / 'crates/hoyadb-native/android/exports.map').read_text()
    block = text.split('global:', 1)[1].split('local:', 1)[0]
    return sorted(name.strip() for name in block.split(';') if name.strip())


def regular(path):
    require(path.is_file() and not path.is_symlink(), 'notice file missing')
    return path.read_bytes()


def lock_tuples(relative):
    return {(p['name'], p['version'], p.get('source'), p.get('checksum'))
            for p in tomllib.loads((ROOT / relative).read_text())['package']}


def notice_files():
    """P04: every notice text is bound to its verified source; set is exact."""
    third = ROOT / 'bindings/android/third-party'
    index_bytes = regular(third / 'index.json')
    index = json.loads(index_bytes)
    require(index.get('project_distribution_license') == 'MIT', 'project distribution license must match MIT')
    project_license = regular(third / 'LICENSE')
    require(digest(project_license) == index.get('project_license_sha256')
            and project_license.startswith(b'MIT License\n')
            and b'Permission is hereby granted, free of charge' in project_license, 'project license digest mismatch')
    files = {'NOTICE.md': regular(ROOT / 'bindings/android/NOTICE.md'), 'third-party/index.json': index_bytes,
             'third-party/LICENSE': project_license}
    for package in index['packages']:
        require(package.get('role') in ROLES, 'unclassified notice package')
        require(isinstance(package.get('files'), list) and package['files'], 'notice file missing')
        for entry in package['files']:
            relative = Path(entry['file'])
            require(not relative.is_absolute() and '..' not in relative.parts, 'notice file missing')
            data = regular(third / relative)
            require(digest(data) == entry['sha256'], 'notice file digest mismatch')
            files['third-party/' + relative.as_posix()] = data
    present = {path.relative_to(third).as_posix() for path in third.rglob('*') if path.is_file() or path.is_symlink()}
    require(present == {name[len('third-party/'):] for name in files if name.startswith('third-party/')}, 'unindexed notice file')
    active = {(p['name'], p['version'], p['source'], p['checksum'])
              for p in json.loads((ROOT / 'docs/implementation/phase-six-dependencies.json').read_text())['packages'] if p.get('active_native')}
    rust = [p for p in index['packages'] if p['role'] in RUST_ROLES]
    claimed = {(p['name'], p['version'], p['source'], p['checksum']) for p in rust}
    require(claimed == active and len(rust) == len(active), 'notice set differs from active native set')
    require(all(claimed <= lock_tuples(lock) for lock in LOCKS), 'notice tuple differs from Cargo.lock')
    # The shipped index names the lock files it was generated from: exact current paths and bytes.
    bound = index.get('lock_sha256')
    require(isinstance(bound, dict) and sorted(bound) == sorted(LOCKS), 'notice index lock path mismatch')
    require(all(not (ROOT / lock).is_symlink() and sha(ROOT / lock) == bound[lock] for lock in LOCKS), 'notice index lock digest mismatch')
    corpus = ROOT / 'licenses/javascript'
    reusable = {(p['name'], p['version'], p['source'], p['checksum']): p for p in json.loads((corpus / 'index.json').read_text())['packages']}
    crate_tuple = next((t for t in claimed if t[0] == 'sqlcipher-src'), None)
    require(crate_tuple is not None, 'notice set differs from active native set')
    archive = CRATES / (crate_tuple[0] + '-' + crate_tuple[1] + '.crate')
    require(archive.is_file() and not archive.is_symlink() and sha(archive) == crate_tuple[3], 'notice text differs from verified source')
    with tarfile.open(archive) as crate:
        def member(name):
            handle = crate.extractfile(crate_tuple[0] + '-' + crate_tuple[1] + '/' + name)
            require(handle is not None, 'notice text differs from verified source')
            return handle.read()
        for package in index['packages']:
            for entry in package['files']:
                data, origin = files['third-party/' + entry['file']], entry.get('origin', {})
                if package['role'] in RUST_ROLES:
                    source = reusable.get((package['name'], package['version'], package['source'], package['checksum']))
                    match = source is not None and origin.get('kind') == 'js_corpus' and package['license'] == source['license'] and any(
                        item['file'] == origin.get('file') and item['sha256'] == digest(data) and regular(corpus / item['file']) == data
                        for item in source['files'])
                elif package['role'] == 'linked_native_c':
                    match = package.get('via') == 'sqlcipher-src' and origin.get('crate') == 'sqlcipher-src' and (
                        origin.get('kind') == 'crate_entry' and member(origin.get('entry', '')) == data
                        or origin.get('kind') == 'crate_entry_prefix' and member(origin.get('entry', '')).startswith(data))
                elif package['role'] == 'toolchain_aggregate':
                    shipped = Path(str(origin.get('path', '')))
                    match = origin.get('kind') == 'toolchain_local' and shipped.is_absolute() and shipped.is_file() \
                        and not shipped.is_symlink() and shipped.read_bytes() == data
                else:
                    match = origin.get('kind') == 'upstream_url' and str(origin.get('url', '')).startswith('https://')
                require(match, 'notice text differs from verified source')
    for package in index['packages']:
        if package['role'] == 'toolchain_aggregate':
            inputs = package.get('verified_link_inputs')
            require(isinstance(inputs, list) and inputs and all(
                Path(item['path']).is_file() and not Path(item['path']).is_symlink() and sha(Path(item['path'])) == item['sha256']
                for item in inputs), 'toolchain link input changed')
    providers = sorted(p['name'] for p in index['packages'] if p['role'] == 'linked_native_c')
    require(providers == ['LibTomCrypt', 'SQLCipher', 'SQLite'], 'notice set differs from active native set')
    review = {a['coordinate']: a['sha256'] for a in json.loads((ROOT / 'bindings/android/toolchain-review.json').read_text())['runtime_artifacts']}
    kotlin = {p.get('coordinate'): p for p in index['packages'] if p['role'] == 'kotlin_runtime'}
    require(set(kotlin) == set(review) and len(kotlin) == len(review), 'Kotlin runtime notice provenance mismatch')
    for coordinate, package in kotlin.items():
        pom = ROOT / package.get('pom', '')
        require(package.get('jar_sha256') == review[coordinate] and pom.is_file() and not pom.is_symlink()
                and sha(pom) == package.get('pom_sha256') and APACHE_URL in pom.read_text()
                and package.get('license') == 'Apache-2.0', 'Kotlin runtime notice provenance mismatch')
    return files


def production_sources():
    android = ROOT / 'bindings/android'
    paths = [*sorted((android / 'src/main/kotlin').rglob('*.kt')), *sorted((android / 'src/main/java').rglob('*.java')),
             android / 'toolchain-review.json', *sorted((android / 'tools').glob('*.py'))]
    try:
        return {str(path.relative_to(ROOT)): sha(path) for path in sorted(paths)}
    except OSError:
        raise SystemExit('source closure mismatch')


def verify_classes(classes, manifest):
    """P03: production-only classes.jar from the current complete source closure."""
    tests = {path.stem for path in (ROOT / 'bindings/android/src/test/kotlin').rglob('*.kt')}
    with zipfile.ZipFile(classes) as archive:
        for name in archive.namelist():
            stem = name.rsplit('/', 1)[-1].split('.', 1)[0].split('$', 1)[0]
            require(stem not in tests and stem.removesuffix('Kt') not in tests, 'test class in production classes')
    current = production_sources()
    require(manifest.get('sources_before') == manifest.get('sources_after') == current, 'source closure mismatch')
    return current


def source_archive():
    android = ROOT / 'bindings/android/src/main'
    files = {}
    for language, pattern in [('kotlin', '*.kt'), ('java', '*.java')]:
        for path in sorted((android / language).rglob(pattern)):
            files[path.relative_to(android / language).as_posix()] = path.read_bytes()
    return files


def write_zip(path, files):
    with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for name, content in sorted(files.items()):
            entry = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0)); entry.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(entry, content)


def packaging_tools():
    return {str(READELF): sha(READELF), str(RUST_SOURCES): sha(RUST_SOURCES),
            **{str(path): sha(path) for path in sorted(Path(__file__).resolve().parent.glob('*.py'))}}


def fresh(out):
    out = out.absolute()
    require(not out.exists() and not out.is_symlink(), 'final output must be fresh')
    require(out.parent.is_dir() and out.parent == out.parent.resolve(), 'final output must be fresh')
    return out


def final(args, files, slices, manifest):
    out = fresh(args.out)
    pending = out.with_name(out.name + '.pending-' + uuid.uuid4().hex)
    pending.mkdir()
    phase = 'validation'
    try:
        tools = packaging_tools()
        sources = verify_classes(args.classes, manifest)
        phase = 'native ELF inspection'
        elf = {abi: inspect_elf(abi, slices[abi]['path']) for abi in args.abis}
        phase = 'notices'
        notices = notice_files()
        for name, content in notices.items():
            files['META-INF/hoyadb-android/' + name] = content
        phase = 'write'
        coordinate = pending / 'dev/hoyadb/hoyadb-android' / VERSION
        coordinate.mkdir(parents=True)
        archives = {BASE + '.aar': files, BASE + '-sources.jar': source_archive(), BASE + '-notices.zip': notices}
        for name, content in archives.items():
            write_zip(coordinate / name, content)
        (coordinate / (BASE + '.pom')).write_text(POM)
        phase = 'reverification'
        for name, content in archives.items():
            with zipfile.ZipFile(coordinate / name) as archive:
                require({entry: digest(archive.read(entry)) for entry in archive.namelist()}
                        == {entry: digest(data) for entry, data in content.items()}, 'produced archive mismatch')
        require(verify_classes(args.classes, manifest) == sources, 'source closure mismatch')
        require(notice_files() == notices, 'notice file digest mismatch')
        require(native_slices(args.abis).keys() == slices.keys() and all(sha(slices[abi]['path']) == slices[abi]['sha256'] for abi in args.abis), 'native artifact digest mismatch')
        require(tools == packaging_tools(), 'packaging tool drift')
        produced = {}
        for name in [BASE + '.aar', BASE + '.pom', BASE + '-sources.jar', BASE + '-notices.zip']:
            produced[name] = sha(coordinate / name)
            (coordinate / (name + '.sha256')).write_text(produced[name] + '\n')
        certificate = {'development_only': False, 'build': 'manual SDK build; resource-free AAR; not an AGP build',
            'coordinate': 'dev.hoyadb:hoyadb-android:' + VERSION, 'compiler': 'installed2.2.20 language/API/metadata2.1',
            'abis': {abi: 'runtime pending independent consumer' if abi == 'x86_64' else 'build-only' for abi in args.abis},
            'files': produced, 'aar_entries_sha256': {name: digest(content) for name, content in sorted(files.items())},
            'classes_sha256': sha(args.classes), 'build_manifest_sha256': sha(args.classes.parent / 'manifest.json'),
            'build_commands': manifest.get('commands'), 'build_tools_sha256': manifest.get('tools_sha256'),
            'sources': sources, 'packaging_tools_sha256': tools,
            'native_provenance_sha256': {name: sha(ROOT / name) for name in ['Cargo.lock', 'crates/hoyadb-native/Cargo.lock',
                'crates/hoyadb-native/Cargo.toml', 'rust-toolchain.toml', 'crates/hoyadb-native/android/exports.map'] if (ROOT / name).is_file()},
            'native_rust_sources_sha256': RUST.rust_sources(ROOT),
            'jni': {abi: {k: v for k, v in record.items() if k not in ('path', 'rust_sources_sha256')} for abi, record in slices.items()},
            'elf': elf, 'notice_index_sha256': digest(notices['third-party/index.json'])}
        (pending / 'certificate.json').write_text(json.dumps(certificate, indent=2) + '\n')
    except SystemExit as error:
        (pending / 'failure.json').write_text(json.dumps({'phase': phase, 'reason': str(error)}, indent=2) + '\n')
        raise
    except BaseException as error:
        (pending / 'failure.json').write_text(json.dumps({'phase': phase, 'reason': type(error).__name__}, indent=2) + '\n')
        raise
    require(not out.exists() and not out.is_symlink(), 'final output must be fresh')
    pending.rename(out)
    print(out / 'certificate.json', sha(out / 'certificate.json'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--classes', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--development', action='store_true')
    parser.add_argument('--abis', nargs='+', choices=MACHINE, default=ABIS,
                        help='native slices to package (default: x86_64 arm64-v8a)')
    args = parser.parse_args()
    require(len(args.abis) == len(set(args.abis)), 'duplicate ABI selection')
    manifest = json.loads((args.classes.parent / 'manifest.json').read_text())
    if manifest['test_classes_included'] or manifest.get('split_runtime') or manifest.get('runtime_dex') or manifest.get('dex_sha256') or sha(args.classes) != manifest['classes_sha256']: raise SystemExit('not a verified production-only class artifact')
    files = {'AndroidManifest.xml': MANIFEST, 'classes.jar': args.classes.read_bytes(), 'consumer-rules.pro': KEEP}
    slices = native_slices(args.abis)
    for abi in args.abis:
        files['jni/' + abi + '/libhoyadb_jni.so'] = slices[abi]['path'].read_bytes()
    if not args.development:
        return final(args, files, slices, manifest)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    write_zip(args.out, files)
    sources = {abi: {k: v for k, v in record.items() if k not in ('path', 'inputs_sha256', 'rust_sources_sha256')} for abi, record in slices.items()}
    args.out.with_suffix('.json').write_text(json.dumps({'development_only': True, 'build': 'manual SDK build', 'aar_sha256': sha(args.out), 'classes_sha256': sha(args.classes), 'jni': sources}, indent=2) + '\n')
    print(args.out, sha(args.out))
if __name__ == '__main__': main()
