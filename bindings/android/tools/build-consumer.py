#!/usr/bin/env python3
"""Build an independent resource-free Android consumer from local Maven AAR only.

No Gradle/cache/hooks/downloads. This is a manual SDK consumer build, not AGP proof.
Review the exact installed-tool commands before first activation.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[3]
SDK = Path('/Users/boristacyniak/Library/Android/sdk')
JAVA = Path('/Users/boristacyniak/.sdkman/candidates/java/17.0.1-tem/bin')
TOOL = Path('/Applications/Android Studio.app/Contents/plugins/Kotlin/kotlinc')
BUILD = SDK / 'build-tools/37.0.0'
VERSION = '0.1.0'
SYNTHETIC_PASSWORD = 'hoyadb-owned-test-only'

def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def require(value, reason):
    if not value: raise SystemExit(reason)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--maven-repo', required=True, type=Path)
    parser.add_argument('--project', required=True, type=Path)
    args = parser.parse_args()
    project = args.project.resolve()
    require(not project.is_relative_to(ROOT) and project.is_relative_to(Path(tempfile.gettempdir()).resolve()), 'consumer must be outside checkout under task temp root')
    require(project.name.startswith('hoyadb-kotlin-consumer-') and not project.exists(), 'consumer project must be fresh and owned')
    coordinate = args.maven_repo.resolve() / 'dev/hoyadb/hoyadb-android' / VERSION
    aar = coordinate / ('hoyadb-android-' + VERSION + '.aar')
    pom = aar.with_suffix('.pom')
    require(aar.is_file() and pom.is_file(), 'local Maven artifact absent')
    require(aar.with_suffix('.aar.sha256').read_text().strip() == sha(aar), 'local AAR digest mismatch')
    require(pom.with_suffix('.pom.sha256').read_text().strip() == sha(pom), 'local POM digest mismatch')
    reviewed = json.loads((ROOT / 'bindings/android/toolchain-review.json').read_text())
    for name, expected in reviewed['installed_tool_closure'].items(): require(sha(TOOL / name) == expected, 'installed compiler closure changed')
    require(all(not os.environ.get(name) for name in ['JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS', 'JDK_JAVA_OPTIONS']), 'unreviewed Java injection')
    dependencies = []
    for entry in reviewed['runtime_artifacts']:
        path = Path(entry['path']); require(sha(path) == entry['sha256'], 'approved runtime changed'); dependencies.append(path)
    template = ROOT / 'bindings/android/consumer'
    before = {str(path.relative_to(template)): sha(path) for path in sorted(template.rglob('*')) if path.is_file()}
    tool_paths = [JAVA / name for name in ['java', 'javac', 'jar', 'keytool']] + [BUILD / name for name in ['aapt2', 'd8', 'zipalign', 'apksigner']] + [BUILD / 'lib/d8.jar', BUILD / 'lib/apksigner.jar', SDK / 'platforms/android-37.0/android.jar']
    tools_before = {str(path): sha(path) for path in tool_paths}
    aar_sha = sha(aar)
    project.mkdir()
    app_id = 'dev.hoyadb.consumer.p' + uuid.uuid4().hex[:16]
    for source in sorted(template.rglob('*')):
        if not source.is_file(): continue
        target = project / source.relative_to(template); target.parent.mkdir(parents=True, exist_ok=True); target.write_bytes(source.read_bytes())
    manifest = project / 'AndroidManifest.xml'
    manifest.write_text(manifest.read_text().replace('package="dev.hoyadb.consumer"', 'package="' + app_id + '"'))
    sdk_dir = project / 'sdk'; sdk_dir.mkdir()
    with zipfile.ZipFile(aar) as archive:
        entries = set(archive.namelist())
        require('classes.jar' in entries, 'AAR classes missing')
        (sdk_dir / 'classes.jar').write_bytes(archive.read('classes.jar'))
        for abi in ['x86_64', 'arm64-v8a']:
            name = 'jni/' + abi + '/libhoyadb_jni.so'; require(name in entries, 'AAR native slice missing')
            destination = sdk_dir / name; destination.parent.mkdir(parents=True, exist_ok=True); destination.write_bytes(archive.read(name))
    classes = project / 'classes'; classes.mkdir()
    android = SDK / 'platforms/android-37.0/android.jar'
    commands = []
    env = os.environ.copy(); env.update(JAVA_HOME=str(JAVA.parent), JAVACMD=str(JAVA / 'java'), JAVA_OPTS='-Xmx512M -Xms128M', KOTLIN_TOOL='', KOTLIN_RUNNER='', KOTLIN_COMPILER='org.jetbrains.kotlin.cli.jvm.K2JVMCompiler')
    def run(command):
        command = [str(arg) for arg in command]; commands.append(command)
        result = subprocess.run(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        with (project / 'build.txt').open('ab') as log: log.write(result.stdout)
        require(result.returncode == 0, 'owned consumer tool failed; see build.txt')
    sources = sorted((project / 'src').rglob('*.kt'))
    cp = os.pathsep.join(str(path) for path in [sdk_dir / 'classes.jar', *dependencies, android])
    run([TOOL / 'bin/kotlinc', '-language-version', '2.1', '-api-version', '2.1', '-jvm-target', '1.8', '-no-stdlib', '-no-reflect', '-classpath', cp, '-module-name', 'hoyadb_consumer', *sources, '-d', classes])
    consumer_jar = project / 'consumer.jar'
    with zipfile.ZipFile(consumer_jar, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(classes.rglob('*')):
            if path.is_file(): archive.write(path, path.relative_to(classes).as_posix())
    dex = project / 'dex'; dex.mkdir()
    # Complete ordinary APK program inputs: NEVER the iterative split-runtime mode.
    run([BUILD / 'd8', '--lib', android, '--min-api', '24', '--output', dex, consumer_jar, sdk_dir / 'classes.jar', *dependencies])
    base = project / 'base.apk'
    run([BUILD / 'aapt2', 'link', '-o', base, '--manifest', manifest, '-I', android, '--min-sdk-version', '24', '--target-sdk-version', '37'])
    unsigned = project / 'unsigned.apk'; unsigned.write_bytes(base.read_bytes())
    with zipfile.ZipFile(unsigned, 'a') as archive:
        for path in sorted(dex.glob('*.dex')): archive.write(path, path.name, compress_type=zipfile.ZIP_DEFLATED)
        for abi in ['x86_64', 'arm64-v8a']:
            path = sdk_dir / 'jni' / abi / 'libhoyadb_jni.so'
            archive.write(path, 'lib/' + abi + '/libhoyadb_jni.so', compress_type=zipfile.ZIP_STORED)
    aligned = project / 'aligned.apk'
    run([BUILD / 'zipalign', '-P', '16', '-f', '-v', '4', unsigned, aligned])
    signing = project / 'synthetic-owned-signing.p12'
    run([JAVA / 'keytool', '-genkeypair', '-alias', 'hoyadb-owned', '-keyalg', 'RSA', '-keysize', '2048', '-validity', '2', '-dname', 'CN=HoyaDB Owned Test', '-storetype', 'PKCS12', '-keystore', signing, '-storepass', SYNTHETIC_PASSWORD, '-keypass', SYNTHETIC_PASSWORD, '-noprompt'])
    apk = project / 'consumer.apk'
    run([BUILD / 'apksigner', 'sign', '--ks', signing, '--ks-pass', 'pass:' + SYNTHETIC_PASSWORD, '--out', apk, aligned])
    run([BUILD / 'apksigner', 'verify', '--verbose', apk])
    run([BUILD / 'zipalign', '-c', '-P', '16', '-v', '4', apk])
    require(before == {str(path.relative_to(template)): sha(path) for path in sorted(template.rglob('*')) if path.is_file()}, 'consumer template drift')
    require(tools_before == {str(path): sha(path) for path in tool_paths}, 'tool drift')
    require(sha(aar) == aar_sha, 'AAR drift')
    result = {'build': 'manual independent AAR consumer APK', 'project': str(project), 'app_id': app_id, 'activity': 'dev.hoyadb.consumer.MainActivity', 'coordinate': 'dev.hoyadb:hoyadb-android:0.1.0', 'aar_sha256': aar_sha, 'apk_sha256': sha(apk), 'sdk_classes_sha256': sha(sdk_dir / 'classes.jar'), 'templates_sha256': before, 'tools_sha256': tools_before, 'runtime_sha256': {path.name: sha(path) for path in dependencies}, 'commands': commands, 'installed': False, 'native_zip_alignment_verified': True, 'runtime_executed': False}
    (project / 'owned-consumer.json').write_text(json.dumps(result, indent=2) + '\n')
    print('manual consumer artifact:', apk)
if __name__ == '__main__': main()
