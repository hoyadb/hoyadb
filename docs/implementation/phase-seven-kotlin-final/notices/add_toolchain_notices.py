#!/usr/bin/env python3
"""Append root-directed supplemental toolchain aggregate notices to the Android corpus.

Link inputs come only from the hash-bound staticlib archive members (llvm-ar t)
and the selected NDK clang driver's `-###` link plan. Texts are copied byte-exact.
"""
import hashlib
import html
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[4]
THIRD = ROOT / 'bindings/android/third-party'
RUST = Path('/Users/boristacyniak/.rustup/toolchains/1.98.1-x86_64-apple-darwin')
NDK = Path('/Users/boristacyniak/Library/Android/sdk/ndk/27.1.12297006')
LLVM = NDK / 'toolchains/llvm/prebuilt/darwin-x86_64'
ABIS = {'x86_64': ('x86_64', 'target/phase-seven-native/android-x86_64/x86_64-linux-android/release/libhoyadb_native.a'),
        'arm64-v8a': ('aarch64', 'target/phase-seven-native/android-arm64/aarch64-linux-android/release/libhoyadb_native.a')}


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(command):
    return subprocess.run(command, check=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT).stdout.decode()


def main():
    index = json.loads((THIRD / 'index.json').read_text())
    if any(p['role'] == 'toolchain_aggregate' for p in index['packages']):
        raise SystemExit('supplemental notices already present')
    evidence = {}
    rust_inputs, ndk_inputs, named = [], [], {}
    copyright = RUST / 'share/doc/rust/COPYRIGHT-library.html'
    text = html.unescape(re.sub('<[^>]+>', ' ', copyright.read_text()))
    for abi, (arch, archive) in ABIS.items():
        native = json.loads((ROOT / 'target/phase-seven-native/jni-current' / abi / 'evidence.json').read_text())
        if native['inputs_sha256'][archive] != sha(ROOT / archive):
            raise SystemExit('staticlib not hash-bound to native evidence')
        ar = [str(LLVM / 'bin/llvm-ar'), 't', str(ROOT / archive)]
        members = sorted({m.group(1) for m in re.finditer(r'^([a-z_]+-[0-9a-f]{16})', run(ar), re.M)})
        sysroot = RUST / 'lib/rustlib' / (arch + '-linux-android') / 'lib'
        linked = [m for m in members if (sysroot / ('lib' + m + '.rlib')).is_file()]
        for member in linked:
            path = sysroot / ('lib' + member + '.rlib')
            rust_inputs.append({'abi': abi, 'path': str(path), 'sha256': sha(path)})
            crate = member.rsplit('-', 1)[0]
            named[crate] = crate in text or crate.replace('_', '-') in text
        driver = [str(LLVM / 'bin' / (arch + '-linux-android24-clang')), '-###', '-shared', '-fPIC', '-o', '/tmp/hoyadb-never-written.so',
                  '-x', 'c', '/dev/null', '-Wl,-z,max-page-size=16384', '-pthread', '-ldl', '-lm']
        plan = run(driver).strip().splitlines()[-1]
        paths = sorted({Path(p.strip('"')).resolve() for p in plan.split() if p.strip('"').endswith(('.a', '.o')) and p.strip('"').startswith(str(LLVM))})
        paths.append((LLVM / 'lib/clang/18/lib/linux' / arch / 'libunwind.a').resolve())
        if '"-l:libunwind.a"' not in plan.split():
            raise SystemExit('driver plan changed')
        for path in sorted(set(paths)):
            ndk_inputs.append({'abi': abi, 'path': str(path), 'sha256': sha(path)})
        evidence[abi] = {'llvm_ar_command': ar, 'archive_members_from_sysroot': linked, 'driver_command': driver, 'driver_link_plan': plan}
    packages = [
        ('rust-std', '1.98.1', 'Rust 1.98.1 standard library sysroot crates linked via the staticlib',
         [(copyright, 'toolchain/rust-1.98.1/COPYRIGHT-library.html')], rust_inputs),
        ('ndk-toolchain-runtime', '27.1.12297006', 'NDK r27b clang driver-added compiler-rt builtins, libunwind and bionic crt objects',
         [(NDK / 'NOTICE.toolchain', 'toolchain/ndk-27.1.12297006/NOTICE.toolchain'), (NDK / 'NOTICE', 'toolchain/ndk-27.1.12297006/NOTICE')], ndk_inputs),
    ]
    for name, version, description, texts, inputs in packages:
        files = []
        for source, relative in texts:
            target = THIRD / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(source.read_bytes())
            files.append({'file': relative, 'sha256': sha(target), 'origin': {'kind': 'toolchain_local', 'path': str(source)}})
        entry = {'name': name, 'version': version, 'license': 'aggregate toolchain notice (see texts)', 'role': 'toolchain_aggregate',
                 'description': description, 'verified_link_inputs': inputs, 'files': files,
                 'qualification': 'Aggregate notice; not every item it lists is linked; not a compiler SBOM or legal certification.'}
        if name == 'rust-std':
            entry['crates_not_named_in_aggregate_text'] = sorted(crate for crate, hit in named.items() if not hit)
        packages_index = index['packages']
        packages_index.append(entry)
    index['not_enumerated'] = []
    (THIRD / 'index.json').write_text(json.dumps(index, indent=2) + '\n')
    out = ROOT / 'target/phase-seven-kotlin/opus/toolchain-link-evidence.json'
    out.write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps({'rust_inputs': len(rust_inputs), 'ndk_inputs': len(ndk_inputs),
                      'not_named': sorted(c for c, h in named.items() if not h)}, indent=1))


if __name__ == '__main__':
    main()
