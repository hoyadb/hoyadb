#!/usr/bin/env python3
"""Build/run TC-0036 G23 against a supplied local AAR and an owned API30 AVD.

The installed API30 x86_64 image may execute ARM32 through its native bridge;
that is recorded separately from direct x86 execution. No downloads are made.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import time
import uuid
import zipfile

HERE = Path(__file__).resolve().parent
TARGETS = {'x86': 'i686-linux-android', 'armeabi-v7a': 'armv7a-linux-androideabi'}
IMAGE = 'system-images;android-30;google_apis_playstore;x86_64'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(argv, **kwargs):
    return subprocess.run(list(map(str, argv)), check=True, text=True, **kwargs)


def build(args):
    args.out.mkdir(parents=True, exist_ok=False)
    sdk, java = args.sdk, args.java / 'bin'
    tools = sdk / 'build-tools/37.0.0'
    android = sdk / 'platforms/android-37.0/android.jar'
    compiler_dir = args.ndk / 'toolchains/llvm/prebuilt/darwin-x86_64/bin'
    record = {'aar': str(args.aar), 'aar_sha256': sha(args.aar), 'image': IMAGE, 'abis': {},
              'sources': {str(HERE / name): sha(HERE / name) for name in ['run.py', 'Native32Instrumentation.java']}}
    with zipfile.ZipFile(args.aar) as archive:
        full_classes = args.out / 'hoya-classes.jar'
        full_classes.write_bytes(archive.read('classes.jar'))
        with zipfile.ZipFile(io.BytesIO(full_classes.read_bytes())) as jar:
            bridge = args.out / 'hoya/dev/hoyadb/NativeBridge.class'
            bridge.parent.mkdir(parents=True)
            bridge.write_bytes(jar.read('dev/hoyadb/NativeBridge.class'))
        for abi, target in TARGETS.items():
            out = args.out / abi
            out.mkdir()
            library = out / 'libhoyadb_jni.so'
            library.write_bytes(archive.read('jni/' + abi + '/libhoyadb_jni.so'))
            sources = [HERE / 'Native32Instrumentation.java']
            libraries = [library]
            if args.monstera_module:
                module = args.monstera_module
                sqlite = module.parents[1] / 'node_modules/@nozbe/sqlite/sqlite-amalgamation-3460000'
                source = module / 'test/plain-sqlite-probe.c'
                plain = out / 'libmonstera_plain_sqlite_probe.so'
                command = [compiler_dir / (target + '24-clang'), '-shared', '-fPIC', '-g',
                           '-I' + str(sqlite), sqlite / 'sqlite3.c', source, '-landroid', '-llog',
                           '-Wl,-z,max-page-size=16384', '-o', plain]
                run(command)
                symbols = run([compiler_dir / 'llvm-readelf', '--dyn-syms', '--wide', library], capture_output=True).stdout
                (out / 'hoya-dynsym.txt').write_text(symbols)
                if any('sqlite3' in line for line in symbols.splitlines()):
                    raise RuntimeError('Hoya dynamic SQLite symbol boundary changed')
                plain_symbols = run([compiler_dir / 'llvm-readelf', '--dyn-syms', '--wide', plain], capture_output=True).stdout
                if 'sqlite3_open' not in plain_symbols:
                    raise RuntimeError('Plain SQLite fixture must retain default-visible provider symbols')
                (out / 'plain-dynsym.txt').write_text(plain_symbols)
                (out / 'plain-build.json').write_text(json.dumps({'command': list(map(str, command)), 'inputs': {
                    str(p): sha(p) for p in [sqlite / 'sqlite3.c', sqlite / 'sqlite3.h', source, command[0]]}}, indent=2) + '\n')
                sources.append(module / 'test/CoexistenceProbe.java')
                record['sources'][str(sources[-1])] = sha(sources[-1])
                libraries.append(plain)
            classes = out / 'classes'
            classes.mkdir()
            run([java / 'javac', '--release', '8', '-cp', os.pathsep.join(map(str, [android, full_classes])), '-d', classes, *sources])
            jar = out / 'probe.jar'
            with zipfile.ZipFile(jar, 'w') as compiled:
                for path in classes.rglob('*.class'):
                    compiled.write(path, path.relative_to(classes))
            dex = out / 'dex'
            dex.mkdir()
            run([tools / 'd8', '--min-api', '24', '--lib', android, '--output', dex, jar, bridge])
            package = 'dev.hoyadb.android32.p' + uuid.uuid4().hex
            manifest = out / 'AndroidManifest.xml'
            manifest.write_text(f'''<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="{package}">
  <uses-sdk android:minSdkVersion="24" android:targetSdkVersion="30" />
  <application android:debuggable="true" android:label="Hoya32Probe" android:allowBackup="false" />
  <instrumentation android:name="dev.hoyadb.Native32Instrumentation" android:targetPackage="{package}" />
</manifest>''')
            raw, apk = out / 'raw.apk', out / 'probe.apk'
            run([tools / 'aapt2', 'link', '-I', android, '--manifest', manifest, '-o', raw])
            with zipfile.ZipFile(raw, 'a') as packed:
                packed.write(dex / 'classes.dex', 'classes.dex')
                for native in libraries:
                    packed.write(native, 'lib/' + abi + '/' + native.name)
            run([tools / 'zipalign', '-P', '16', '-f', '4', raw, apk])
            key = out / 'fixture.keystore'
            run([java / 'keytool', '-genkeypair', '-keystore', key, '-storepass', 'android', '-keypass', 'android',
                 '-alias', 'fixture', '-keyalg', 'RSA', '-keysize', '2048', '-validity', '1', '-dname', 'CN=Disposable Hoya32 Probe'])
            run([tools / 'apksigner', 'sign', '--ks', key, '--ks-pass', 'pass:android', apk])
            record['abis'][abi] = {'package': package, 'apk': str(apk), 'apk_sha256': sha(apk),
                                   'jni_sha256': sha(library), 'coexistence': bool(args.monstera_module)}
            print('Built actual AAR probe: ' + abi, flush=True)
    (args.out / 'build.json').write_text(json.dumps(record, indent=2) + '\n')


def execute(args):
    record = json.loads((args.out / 'build.json').read_text())
    for path, expected in record['sources'].items():
        if sha(Path(path)) != expected:
            raise RuntimeError('Probe source changed after build')
    if sha(Path(record['aar'])) != record['aar_sha256']:
        raise RuntimeError('AAR changed after probe build')
    adb = args.sdk / 'platform-tools/adb'
    manager = args.sdk / 'cmdline-tools/latest/bin/avdmanager'
    identity, serial = 'hoya32_' + uuid.uuid4().hex, 'emulator-5588'
    env = {**os.environ, 'ANDROID_AVD_HOME': str(args.out / 'avd-home'), 'JAVA_HOME': str(args.java),
           'ANDROID_HOME': str(args.sdk)}
    Path(env['ANDROID_AVD_HOME']).mkdir(exist_ok=True)
    emulator, created = None, False
    results = {'identity': identity, 'serial': serial, 'image': IMAGE, 'build_sha256': sha(args.out / 'build.json'),
               'abis': {}, 'cleanup': False}

    def device(*argv, **kwargs):
        return run([adb, '-s', serial, *argv], **kwargs)

    try:
        devices = run([adb, 'devices'], capture_output=True).stdout.splitlines()[1:]
        if any(line.strip() for line in devices):
            raise RuntimeError('Existing Android device; coordinate slot first')
        ios = json.loads(run(['xcrun', 'simctl', 'list', 'devices', 'booted', '-j'], capture_output=True).stdout)
        if any(d['state'] == 'Booted' for group in ios['devices'].values() for d in group):
            raise RuntimeError('Existing booted iOS simulator; coordinate slot first')
        created = True
        run([manager, 'create', 'avd', '--name', identity, '--path', args.out / 'avd', '--package', IMAGE], input='no\n', env=env)
        with (args.out / 'emulator.log').open('w') as log:
            emulator = subprocess.Popen([str(args.sdk / 'emulator/emulator'), '-avd', identity, '-port', '5588',
                                         '-no-window', '-no-audio', '-no-snapshot', '-no-boot-anim'], env=env,
                                        stdout=log, stderr=subprocess.STDOUT)
        device('wait-for-device', timeout=120)
        for _ in range(120):
            if device('shell', 'getprop', 'sys.boot_completed', capture_output=True).stdout.strip() == '1':
                break
            if emulator.poll() is not None:
                raise RuntimeError('Owned emulator exited; inspect retained emulator.log')
            time.sleep(1)
        else:
            raise RuntimeError('Owned emulator did not boot')
        props = ['ro.build.version.sdk', 'ro.product.cpu.abilist', 'ro.product.cpu.abilist32', 'ro.dalvik.vm.native.bridge']
        results['device_properties'] = {p: device('shell', 'getprop', p, capture_output=True).stdout.strip() for p in props}
        for abi, build_record in record['abis'].items():
            package, apk = build_record['package'], Path(build_record['apk'])
            if sha(apk) != build_record['apk_sha256']:
                raise RuntimeError('APK changed after build')
            device('install', '--abi', abi, apk)
            package_details = device('shell', 'dumpsys', 'package', package, capture_output=True).stdout
            (args.out / abi / 'installed-package.txt').write_text(package_details)
            if 'primaryCpuAbi=' + abi not in package_details:
                raise RuntimeError('Package ABI selection differs')
            phases, pids = [], set()
            results['abis'][abi] = {'execution': 'direct x86' if abi == 'x86' else 'ARM32 via installed image native bridge', 'phases': phases}
            for phase in ['seed', 'parent', 'child', 'restart'] + (['coexistence'] if build_record['coexistence'] else []):
                device('shell', 'am', 'force-stop', package)
                result = device('shell', 'am', 'instrument', '-w', '-e', 'phase', phase, '-e', 'abi', abi,
                                package + '/dev.hoyadb.Native32Instrumentation', capture_output=True, timeout=90)
                (args.out / abi / (phase + '.log')).write_text(result.stdout + result.stderr)
                if 'INSTRUMENTATION_RESULT: status=PASS ' + phase not in result.stdout:
                    raise RuntimeError(result.stdout + result.stderr)
                details_line = next(line for line in result.stdout.splitlines() if line.startswith('INSTRUMENTATION_RESULT: details='))
                details = json.loads(details_line.split('=', 1)[1])
                if details['is64Bit'] or details['pid'] in pids or details['abi'] != abi:
                    raise RuntimeError('Process width, restart or selected ABI proof failed')
                pids.add(details['pid'])
                phases.append({'phase': phase, **details})
                print('PASS ' + abi + ' ' + phase + ' ' + json.dumps(details), flush=True)
            device('uninstall', package)
        results['passed'] = True
    finally:
        if emulator:
            owner = subprocess.run([str(adb), '-s', serial, 'emu', 'avd', 'name'], capture_output=True, text=True)
            if identity in owner.stdout.splitlines():
                for built in record['abis'].values():
                    subprocess.run([str(adb), '-s', serial, 'uninstall', built['package']], capture_output=True)
                subprocess.run([str(adb), '-s', serial, 'emu', 'kill'], capture_output=True)
            elif emulator.poll() is None:
                emulator.terminate()
            try:
                emulator.wait(timeout=30)
            except subprocess.TimeoutExpired:
                emulator.terminate()
                emulator.wait(timeout=30)
        if created:
            run([manager, 'delete', 'avd', '--name', identity], capture_output=True, env=env)
        results['cleanup'] = not (args.out / 'avd').exists() and (emulator is None or emulator.poll() is not None)
        (args.out / 'runtime.json').write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['build', 'run'])
    parser.add_argument('--out', required=True, type=lambda value: Path(value).resolve())
    parser.add_argument('--aar', type=lambda value: Path(value).resolve())
    parser.add_argument('--monstera-module', type=lambda value: Path(value).resolve())
    parser.add_argument('--sdk', type=Path, default=Path.home() / 'Library/Android/sdk')
    parser.add_argument('--ndk', type=Path, default=Path.home() / 'Library/Android/sdk/ndk/27.1.12297006')
    parser.add_argument('--java', type=Path, default=Path(os.environ.get('JAVA_HOME', '/missing-java-home')))
    options = parser.parse_args()
    if options.action == 'build':
        if not options.aar:
            parser.error('build requires --aar')
        build(options)
    else:
        execute(options)
