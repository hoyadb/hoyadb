"""SPEC-0043 / TC-0047: build the Kotlin guide with the Android binding and run it on an emulator.

  python3 docs/public/run_kotlin.py build   # Android core, JNI bridge, binding and guide DEX, manifest
  python3 docs/public/run_kotlin.py run     # run the guide DEX on the task-owned Android emulator

Everything is written under target/documentation-languages/kotlin. The native core, JNI bridge and
Kotlin binding are built from a `git archive` of HEAD with the locally installed, reviewed tools
(NDK, Kotlin compiler, D8); nothing is downloaded and no Gradle build runs. `run` uses only the
task-owned AVD (started read-only when it is not already running, and stopped again afterwards),
runs the guide with app_process in a fresh /data/local/tmp directory, and removes that directory.
This is a real Android runtime on an x86_64 emulator; it is not an APK build or device certification.
"""
import hashlib
import io
import json
import os
import shutil
import socket
import subprocess
import sys
import tarfile
import time
import uuid
import zipfile
from pathlib import Path

import run_native
from run_native import fail, files, no_links, relative, require_inside, reset, sha256

REPO = run_native.REPO
GUIDE = REPO / 'examples/kotlin/TodoGuide.kt'
KOTLIN = run_native.OUT / 'kotlin'
MANIFEST = KOTLIN / 'manifest.json'
RUN_REPORT = KOTLIN / 'run.json'
SOURCE = KOTLIN / 'src'
PROVIDER = KOTLIN / 'provider'
NATIVE = KOTLIN / 'native'
JNI = KOTLIN / 'jni'
BUILD = KOTLIN / 'build'
ARCHIVED = ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'crates', 'crates/hoyadb-native', 'bindings/android']
TRIPLE = 'x86_64-linux-android'
LIBRARY = NATIVE / TRIPLE / 'release/libhoyadb_native.a'

ANDROID_SDK = Path.home() / 'Library/Android/sdk'
NDK = ANDROID_SDK / 'ndk/27.1.12297006'
NDK_BIN = NDK / 'toolchains/llvm/prebuilt/darwin-x86_64/bin'
ANDROID_JAR = ANDROID_SDK / 'platforms/android-37.0/android.jar'
D8 = ANDROID_SDK / 'build-tools/37.0.0/d8'
ADB = ANDROID_SDK / 'platform-tools/adb'
EMULATOR = ANDROID_SDK / 'emulator/emulator'
JAVA = Path.home() / '.sdkman/candidates/java/17.0.1-tem/bin'
KOTLINC = Path('/Applications/Android Studio.app/Contents/plugins/Kotlin/kotlinc')
ANDROID_USER_HOME = REPO / 'target/phase-seven-native/android-user'
AVD = 'hoyadb_phase7_owned_ps16k'
PORT = '5588'
SERIAL = 'emulator-' + PORT
COMPLETION = 'kotlin tour: ok'


def extract_head(commit):
    native_path = run_native.archived_native_path(REPO, commit)
    archive_paths = [p for p in ARCHIVED if p != 'crates/hoyadb-native'] + [native_path]
    archive = subprocess.run(['git', 'archive', '--format=tar', commit, '--', *archive_paths], cwd=REPO,
                             capture_output=True, check=True).stdout
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        for member in tar.getmembers():
            if member.issym() or member.islnk() or not (member.isfile() or member.isdir()):
                fail(f'refusing archive member {member.name}')
        tar.extractall(SOURCE, filter='data')
    no_links(SOURCE)
    return native_path, archive_paths


def reviewed_tools():
    """The reviewed compiler closure and runtime jars recorded by the Android binding itself."""
    review = json.loads((SOURCE / 'bindings/android/toolchain-review.json').read_text())
    tools = {}
    for name, expected in review['installed_tool_closure'].items():
        path = KOTLINC / name
        if sha256(path) != expected:
            fail(f'installed Kotlin compiler file changed: {path}')
        tools[str(path)] = expected
    runtime = []
    for artifact in review['runtime_artifacts']:
        path = Path(artifact['path'])
        if sha256(path) != artifact['sha256']:
            fail(f'reviewed Kotlin runtime archive changed: {path}')
        runtime.append(path)
    for path in [ANDROID_JAR, D8, D8.parent / 'lib/d8.jar', JAVA / 'java', JAVA / 'javac',
                 NDK_BIN / f'{TRIPLE}24-clang', NDK_BIN / 'llvm-ar']:
        tools[str(path)] = sha256(path)
    return tools, runtime


def jar(classes, output):
    with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for path in files(classes):
            entry = zipfile.ZipInfo(path.relative_to(classes).as_posix(), (1980, 1, 1, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(entry, path.read_bytes())


def build():
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()
    differing = subprocess.check_output(['git', 'status', '--porcelain', '--', *ARCHIVED], cwd=REPO,
                                        text=True).splitlines()
    if any(os.environ.get(key) for key in ['JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS', 'JDK_JAVA_OPTIONS']):
        fail('Java option injection is set in the environment')
    commands = []

    def step(name, args, cwd=REPO, overrides=None):
        overrides = {key: str(value) for key, value in (overrides or {}).items()}
        run_native.run(name, args, cwd, dict(os.environ, **overrides), KOTLIN / 'logs' / f'{name}.log')
        commands.append({'name': name, 'cwd': relative(cwd), 'command': [str(a) for a in args], 'env': overrides})

    require_inside(KOTLIN)
    KOTLIN.mkdir(parents=True, exist_ok=True)
    reset(KOTLIN / 'logs')
    reset(SOURCE)
    native_path, archive_paths = extract_head(commit)
    native_toml = SOURCE / native_path / 'Cargo.toml'
    original_toml = sha256(native_toml)
    with native_toml.open('a') as handle:
        handle.write(run_native.WORKSPACE_ROOT)
    tools, runtime = reviewed_tools()

    # The approved SQLCipher/LibTomCrypt archive with Android OS entropy, then the Rust core linked to it.
    require_inside(PROVIDER)
    if PROVIDER.exists():
        shutil.rmtree(PROVIDER)
    step('provider', [sys.executable, '-I', SOURCE / native_path / 'android/build-provider.py',
                      '--ndk', NDK, '--abi', 'x86_64', '--output', PROVIDER])
    cargo_env = dict(CARGO_TARGET_DIR=str(NATIVE), LIBSQLITE3_SYS_USE_PKG_CONFIG='1',
               SQLCIPHER_LIB_DIR=str(PROVIDER / 'lib'), SQLCIPHER_INCLUDE_DIR=str(PROVIDER / 'include'),
               SQLCIPHER_STATIC='1', PKG_CONFIG_ALLOW_CROSS='1', PKG_CONFIG_LIBDIR=str(PROVIDER / 'lib/pkgconfig'),
               CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=str(NDK_BIN / f'{TRIPLE}24-clang'),
               CC_x86_64_linux_android=str(NDK_BIN / f'{TRIPLE}24-clang'),
               AR_x86_64_linux_android=str(NDK_BIN / 'llvm-ar'),
               CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS='-Clink-arg=-Wl,-z,max-page-size=16384')
    require_inside(NATIVE)
    step('native', ['cargo', 'build', '--manifest-path', f'{native_path}/Cargo.toml', '--target', TRIPLE,
                    '--release', '--offline', '--locked'], SOURCE, cargo_env)
    reset(JNI)
    step('jni', [sys.executable, '-I', SOURCE / native_path / 'android/build-jni.py', '--abi', 'x86_64',
                 '--ndk', NDK, '--archive', LIBRARY, '--out', JNI])

    # The binding as its own module, then the guide against its public API only.
    reset(BUILD)
    sdk_classes, guide_classes, dex = BUILD / 'sdk', BUILD / 'guide', BUILD / 'dex'
    for directory in [sdk_classes, guide_classes, dex]:
        directory.mkdir()
    android = SOURCE / 'bindings/android/src/main'
    java_sources = sorted((android / 'java').rglob('*.java'))
    kotlin_sources = sorted((android / 'kotlin').rglob('*.kt'))
    kotlin_env = dict(JAVA_HOME=str(JAVA.parent), JAVACMD=str(JAVA / 'java'), JAVA_OPTS='-Xmx768M',
                      KOTLIN_COMPILER='org.jetbrains.kotlin.cli.jvm.K2JVMCompiler')
    step('javac', [JAVA / 'javac', '--release', '8', '-classpath', ANDROID_JAR, '-d', sdk_classes, *java_sources])
    classpath = os.pathsep.join(str(p) for p in [*runtime, ANDROID_JAR, sdk_classes])
    flags = ['-language-version', '2.1', '-api-version', '2.1', '-jvm-target', '1.8', '-no-stdlib', '-no-reflect',
             '-Werror']
    step('kotlinc-sdk', [KOTLINC / 'bin/kotlinc', *flags[:-1], '-classpath', classpath, '-module-name', 'hoyadb',
                         *kotlin_sources, '-d', sdk_classes], overrides=kotlin_env)
    sdk_jar = BUILD / 'hoyadb-android.jar'
    jar(sdk_classes, sdk_jar)
    guide_classpath = os.pathsep.join(str(p) for p in [*runtime, ANDROID_JAR, sdk_jar])
    step('kotlinc-guide', [KOTLINC / 'bin/kotlinc', *flags, '-classpath', guide_classpath, '-module-name', 'todo-guide',
                           GUIDE, '-d', guide_classes], overrides=kotlin_env)
    guide_jar = BUILD / 'todo-guide.jar'
    jar(guide_classes, guide_jar)
    step('d8', [D8, '--release', '--lib', ANDROID_JAR, '--min-api', '24', '--output', dex, guide_jar, sdk_jar, *runtime],
         overrides=dict(JAVA_HOME=str(JAVA.parent), PATH=f'{JAVA}{os.pathsep}{os.environ["PATH"]}'))
    if {p.name for p in dex.iterdir()} != {'classes.dex'}:
        fail('D8 must produce exactly one classes.dex')

    inputs = {relative(path): sha256(path) for path in [*files(SOURCE), GUIDE]}
    artifacts = {relative(path): sha256(path) for path in
                 [dex / 'classes.dex', JNI / 'libhoyadb_jni.so', LIBRARY, sdk_jar, guide_jar, PROVIDER / 'lib/libsqlcipher.a']}
    manifest = {
        'spec': 'SPEC-0043 / TC-0047',
        'source': {'commit': commit, 'kind': 'git archive of HEAD', 'paths': archive_paths,
                   'working_tree_differs_from_head': differing,
                   'modified_copy': {'path': f'{native_path}/Cargo.toml', 'original_sha256': original_toml,
                                     'appended': run_native.WORKSPACE_ROOT},
                   'guide': relative(GUIDE)},
        'target': {'abi': 'x86_64', 'triple': TRIPLE, 'min_sdk': 24, 'page_size_link': 16384},
        'tools': tools,
        'runtime_jars': {str(p): sha256(p) for p in runtime},
        'commands': commands,
        'inputs': inputs,
        'artifacts': artifacts,
        'run': {'dex': relative(dex / 'classes.dex'), 'jni': relative(JNI / 'libhoyadb_jni.so'),
                'main_class': 'TodoGuideKt', 'avd': AVD, 'serial': SERIAL},
    }
    MANIFEST.write_text(json.dumps(manifest, indent=2) + '\n')
    print(f'PASS Kotlin build from {commit}; {len(artifacts)} artifacts recorded in {relative(MANIFEST)}')


def validated_manifest():
    """Fails unless the built artifacts, every input and the exact source-file set match the manifest."""
    if not MANIFEST.is_file():
        fail(f'{relative(MANIFEST)} is missing; run `python3 docs/public/run_kotlin.py build` first')
    manifest = json.loads(MANIFEST.read_text())
    for group in ['artifacts', 'inputs']:
        for rel, digest in manifest[group].items():
            path = REPO / rel
            if path.resolve() != GUIDE.resolve():
                require_inside(path)
            if not path.is_file() or sha256(path) != digest:
                fail(f'{rel} differs from {relative(MANIFEST)}; rebuild')
    expected = {rel for rel in manifest['inputs'] if rel != relative(GUIDE)}
    if {relative(path) for path in files(SOURCE)} != expected:
        fail(f'the archived sources differ from {relative(MANIFEST)}; rebuild')
    return manifest


def adb(*args, check=True, timeout=120):
    result = subprocess.run([str(ADB), '-s', SERIAL, *[str(a) for a in args]], capture_output=True, text=True,
                            timeout=timeout)
    if check and result.returncode:
        fail(f'adb {" ".join(str(a) for a in args)} exited {result.returncode}: {result.stderr.strip()}')
    return result.stdout.strip()


def adb_server_running():
    """Whether a local adb server already listens on its default port. It is left running: others may use it."""
    try:
        with socket.create_connection(('127.0.0.1', 5037), timeout=2):
            return True
    except OSError:
        return False


def emulator_env():
    return dict(os.environ, ANDROID_USER_HOME=str(ANDROID_USER_HOME), ANDROID_AVD_HOME=str(ANDROID_USER_HOME / 'avd'))


def running_avd():
    state = subprocess.run([str(ADB), '-s', SERIAL, 'get-state'], capture_output=True, text=True, timeout=30)
    if state.returncode or state.stdout.strip() != 'device':
        return None
    return adb('emu', 'avd', 'name').splitlines()[0].strip()


def start_emulator():
    log = (KOTLIN / 'logs/emulator.log').open('w')
    command = [EMULATOR, '-avd', AVD, '-port', PORT, '-no-window', '-no-audio', '-no-snapshot', '-gpu', 'swiftshader',
               '-read-only']
    print('$', ' '.join(str(a) for a in command), flush=True)
    process = subprocess.Popen([str(a) for a in command], env=emulator_env(), stdout=log, stderr=subprocess.STDOUT)
    deadline = time.monotonic() + 900
    while time.monotonic() < deadline:
        if process.poll() is not None:
            fail(f'the emulator exited {process.returncode}; see {relative(KOTLIN / "logs/emulator.log")}')
        if running_avd() == AVD and adb('shell', 'getprop', 'sys.boot_completed', check=False) == '1':
            return process
        time.sleep(5)
    process.kill()
    fail('the emulator did not boot within 900 seconds')


def stop_emulator(process):
    """Stops the emulator this runner started; never touches whatever else holds the serial."""
    if process.poll() is not None:
        return
    if running_avd() == AVD:
        subprocess.run([str(ADB), '-s', SERIAL, 'emu', 'kill'], capture_output=True, timeout=60)
        try:
            process.wait(timeout=120)
            return
        except subprocess.TimeoutExpired:
            pass
    process.kill()
    process.wait()


def run():
    RUN_REPORT.unlink(missing_ok=True)
    adb_already_running = adb_server_running()
    manifest = validated_manifest()
    (KOTLIN / 'logs').mkdir(exist_ok=True)
    current = running_avd()
    if current is not None and current != AVD:
        fail(f'{SERIAL} is running {current!r}, not the task-owned {AVD}')
    process = start_emulator() if current is None else None
    remote = f'/data/local/tmp/hoyadb-docs-kotlin-{uuid.uuid4().hex}'
    try:
        device = {'serial': SERIAL, 'avd': AVD, 'started_by_runner': process is not None,
                  'read_only': process is not None,
                  'sdk': adb('shell', 'getprop', 'ro.build.version.sdk'),
                  'abi': adb('shell', 'getprop', 'ro.product.cpu.abi'),
                  'page_size': adb('shell', 'getconf', 'PAGE_SIZE'),
                  'fingerprint': adb('shell', 'getprop', 'ro.build.fingerprint'),
                  'remote_root': remote}
        if device['abi'] != 'x86_64':
            fail(f'expected an x86_64 device, got {device["abi"]}')
        adb('shell', 'mkdir', remote)
        try:
            adb('push', REPO / manifest['run']['dex'], f'{remote}/classes.dex')
            adb('push', REPO / manifest['run']['jni'], f'{remote}/libhoyadb_jni.so')
            command = [str(ADB), '-s', SERIAL, 'shell', f'CLASSPATH={remote}/classes.dex', 'app_process',
                       f'-Djava.library.path={remote}', '/system/bin', manifest['run']['main_class']]
            print('$', ' '.join(command), flush=True)
            result = subprocess.run(command, capture_output=True, text=True, timeout=300)
            (KOTLIN / 'logs/run.log').write_text(result.stdout + result.stderr + f'\nexit={result.returncode}\n')
        finally:
            adb('shell', 'rm', '-rf', remote, check=False)
    finally:
        if process is not None:
            stop_emulator(process)
    if result.returncode:
        sys.stdout.write(result.stdout + result.stderr)
        fail(f'the Kotlin guide exited {result.returncode}')
    run_native.expect_completion(result, COMPLETION)
    # Written only after a zero exit and the exact completion line.
    RUN_REPORT.write_text(json.dumps({'device': device, 'command': command, 'exit': result.returncode,
                                      'completion_line': COMPLETION, 'adb_server_already_running': adb_already_running,
                                      'manifest_sha256': sha256(MANIFEST), 'artifacts': manifest['artifacts']},
                                     indent=2) + '\n')


if __name__ == '__main__':
    commands = {'build': build, 'run': run}
    if len(sys.argv) != 2 or sys.argv[1] not in commands:
        raise SystemExit('usage: python3 docs/public/run_kotlin.py build|run')
    commands[sys.argv[1]]()
