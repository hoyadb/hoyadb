#!/usr/bin/env python3
"""One-shot Android notice corpus materialization from verified inputs only.

Copies exact bytes; never edits text. bindings/android/tools/package-aar.py
independently re-verifies every claim recorded here before final packaging.
"""
import hashlib
import json
from pathlib import Path
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[4]
OUT = ROOT / 'bindings/android/third-party'
INPUTS = ROOT / 'target/phase-seven-kotlin/notice-inputs'
CRATES = Path.home() / '.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f'
SQLCIPHER = 'sqlcipher-src-419.0.6+sqlcipher-4.19.0-sqlite-3.53.4-libtomcrypt-1.18.2'
BUILD_TOOL = {'cc', 'find-msvc-tools', 'pkg-config', 'shlex', 'vcpkg'}
LINKED = {'bitflags', 'fallible-iterator', 'fallible-streaming-iterator', 'libsqlite3-sys', 'rusqlite', 'smallvec', 'zeroize'}
KOTLIN_TAG = 'https://raw.githubusercontent.com/JetBrains/kotlin/v2.1.20/license/'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def put(relative, data):
    path = OUT / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return {'file': relative, 'sha256': sha(data)}


def main():
    if OUT.exists():
        raise SystemExit('notice corpus output must be fresh')
    active = [p for p in json.loads((ROOT / 'docs/implementation/phase-six-dependencies.json').read_text())['packages'] if p.get('active_native')]
    js = {p['name']: p for p in json.loads((ROOT / 'bindings/javascript/third-party/index.json').read_text())['packages']}
    locks = {}
    for lock in ['Cargo.lock', 'bindings/native/Cargo.lock']:
        locks[lock] = {p['name']: [p['name'], p['version'], p.get('source'), p.get('checksum')]
                       for p in tomllib.loads((ROOT / lock).read_text())['package']}
    packages = []
    for item in sorted(active, key=lambda p: p['name']):
        name = item['name']
        reused = js[name]
        tuple_ = [item['name'], item['version'], item['source'], item['checksum']]
        if not (tuple_ == [reused['name'], reused['version'], reused['source'], reused['checksum']]
                == locks['Cargo.lock'][name] == locks['bindings/native/Cargo.lock'][name]):
            raise SystemExit('tuple mismatch ' + name)
        files = []
        for entry in reused['files']:
            data = (ROOT / 'bindings/javascript/third-party' / entry['file']).read_bytes()
            if sha(data) != entry['sha256']:
                raise SystemExit('JS corpus text changed ' + entry['file'])
            record = put('rust/' + entry['file'], data)
            record['origin'] = {'kind': 'js_corpus', 'file': entry['file']}
            files.append(record)
        role = 'build_tool' if name in BUILD_TOOL else 'linked_rust' if name in LINKED else 'build_native_c_source'
        packages.append({'name': name, 'version': item['version'], 'source': item['source'], 'checksum': item['checksum'],
                         'license': reused['license'], 'role': role, 'files': files})
    archive = CRATES / (SQLCIPHER + '.crate')
    with tarfile.open(archive) as crate:
        def entry(name):
            return crate.extractfile(SQLCIPHER + '/' + name).read()
        header = entry('sqlcipher/sqlite3.h')
        blessing = (ROOT / 'licenses/sqlcipher/LICENSE-sqlite').read_bytes()
        if not header.startswith(blessing):
            raise SystemExit('SQLite blessing not in sqlite3.h')
        providers = [
            ('SQLCipher', '4.19.0', 'BSD-3-Clause', 'native/sqlcipher-4.19.0/LICENSE-sqlcipher', entry('sqlcipher/LICENSE-sqlcipher'), 'crate_entry', 'sqlcipher/LICENSE-sqlcipher'),
            ('SQLite', '3.53.4', 'blessing', 'native/sqlite-3.53.4/LICENSE-sqlite', blessing, 'crate_entry_prefix', 'sqlcipher/sqlite3.h'),
            ('LibTomCrypt', '1.18.2', 'public domain OR WTFPL', 'native/libtomcrypt-1.18.2/LICENSE-libtomcrypt', entry('sqlcipher/LICENSE-libtomcrypt'), 'crate_entry', 'sqlcipher/LICENSE-libtomcrypt'),
        ]
    for name, version, license_, relative, data, kind, member in providers:
        record = put(relative, data)
        record['origin'] = {'kind': kind, 'crate': 'sqlcipher-src', 'entry': member}
        packages.append({'name': name, 'version': version, 'license': license_, 'role': 'linked_native_c',
                         'via': 'sqlcipher-src', 'files': [record]})
    provenance = {item['file']: item for item in json.loads((INPUTS / 'provenance.json').read_text())['files']}
    review = json.loads((ROOT / 'bindings/android/toolchain-review.json').read_text())
    jars = {item['coordinate']: item['sha256'] for item in review['runtime_artifacts']}
    kotlin = [
        ('org.jetbrains.kotlin:kotlin-stdlib:2.1.20', 'kotlin-stdlib', '2.1.20',
         'target/phase-seven-dependencies/native-source-review/kotlin-stdlib-2.1.20.pom',
         [('LICENSE.txt', 'kotlin-v2.1.20-license-LICENSE.txt'), ('NOTICE.txt', 'kotlin-v2.1.20-license-NOTICE.txt'),
          ('third_party/gwt_license.txt', 'kotlin-v2.1.20-third_party-gwt_license.txt'),
          ('third_party/threetenbp_license.txt', 'kotlin-v2.1.20-third_party-threetenbp_license.txt'),
          ('third_party/guava_license.txt', 'kotlin-v2.1.20-third_party-guava_license.txt'),
          ('third_party/boost_LICENSE.txt', 'kotlin-v2.1.20-third_party-boost_LICENSE.txt')]),
        ('org.jetbrains:annotations:13.0', 'annotations', '13.0',
         'target/phase-seven-dependencies/native-source-review/gradle-user-home/caches/modules-2/files-2.1/org.jetbrains/annotations/13.0/fa7d3d07cc80547e2d15bf4839d3267c637c642f/annotations-13.0.pom',
         [('LICENSE-2.0.txt', 'apache-LICENSE-2.0.txt')]),
    ]
    for coordinate, name, version, pom, texts in kotlin:
        files = []
        for target, source in texts:
            data = (INPUTS / source).read_bytes()
            if sha(data) != provenance[source]['sha256']:
                raise SystemExit('fetched notice changed ' + source)
            record = put('kotlin/' + name + '-' + version + '/' + target, data)
            record['origin'] = {'kind': 'upstream_url', 'url': provenance[source]['url']}
            files.append(record)
        packages.append({'name': name, 'version': version, 'coordinate': coordinate, 'license': 'Apache-2.0',
                         'role': 'kotlin_runtime', 'jar_sha256': jars[coordinate], 'pom': pom,
                         'pom_sha256': sha((ROOT / pom).read_bytes()), 'files': files})
    index = {
        'scope': 'Notices for the local unpublished dev.hoyadb:hoyadb-android:0.1.0 AAR: exact active native Rust graph '
                 '(linked, build-only and C-source roles), SQLCipher/SQLite/LibTomCrypt C provider and approved Kotlin runtime.',
        'project_distribution_license': None,
        'lock_sha256': {lock: sha((ROOT / lock).read_bytes()) for lock in locks},
        'not_enumerated': ['Rust 1.98.1 standard library (core/alloc/std) statically linked into the JNI library',
                           'Android NDK 27.1 compiler runtime builtins statically linked into the JNI library'],
        'packages': packages,
    }
    (OUT / 'index.json').write_text(json.dumps(index, indent=2) + '\n')
    print('notice packages', len(packages), 'files', sum(len(p['files']) for p in packages))


if __name__ == '__main__':
    main()
