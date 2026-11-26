#!/usr/bin/env python3
"""Reviewed manual SDK build; no Gradle/dependency hooks/compiler downloads."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import uuid
import zipfile
from runtime_cache import canonical, fresh_output, require, runtime_plan, validate_cache

ROOT = Path(__file__).resolve().parents[3]
SDK = Path('/Users/boristacyniak/Library/Android/sdk')
JAVA = Path('/Users/boristacyniak/.sdkman/candidates/java/17.0.1-tem/bin')
TOOL = Path('/Applications/Android Studio.app/Contents/plugins/Kotlin/kotlinc')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_hashes(paths):
    return {str(path.relative_to(ROOT)): digest(path) for path in sorted(paths)}


def provenance_paths(tests):
    # Re-enumerated after the build: an added/removed source or tool is drift.
    android = ROOT / 'bindings/android'
    sources = sorted((android / 'src/main/kotlin').rglob('*.kt'))
    if tests:
        sources += sorted((android / 'src/test/kotlin').rglob('*.kt'))
    java = sorted((android / 'src/main/java').rglob('*.java'))
    return sources, java, [*sources, *java, android / 'toolchain-review.json', *sorted((android / 'tools').glob('*.py'))]


def current_sources(tests):
    try:
        return source_hashes(provenance_paths(tests)[2])
    except OSError:
        raise SystemExit('SDK source drift during build')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--tests', action='store_true')
    parser.add_argument('--split-runtime', action='store_true')
    args = parser.parse_args()
    require(not args.split_runtime or args.tests, 'split runtime is test-only')
    output = fresh_output(args.out)
    classes = output / 'classes'; classes.mkdir()
    reviewed = json.loads((ROOT / 'bindings/android/toolchain-review.json').read_text())
    tool_paths = []
    for name, expected in reviewed['installed_tool_closure'].items():
        path = TOOL / name
        require(digest(path) == expected, 'installed compiler closure changed')
        tool_paths.append(path)
    dependencies = []
    for artifact in reviewed['runtime_artifacts']:
        path = Path(artifact['path'])
        require(digest(path) == artifact['sha256'], 'runtime archive changed')
        dependencies.append(path)
    require(all(not os.environ.get(key) for key in ['JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS', 'JDK_JAVA_OPTIONS']), 'unreviewed Java injection')
    android = SDK / 'platforms/android-37.0/android.jar'
    d8 = SDK / 'build-tools/37.0.0/d8'
    tool_paths += [android, d8, d8.parent / 'lib/d8.jar', JAVA / 'java', JAVA / 'javac']
    tools_before = {str(path): digest(path) for path in tool_paths}
    sources, java, _ = provenance_paths(args.tests)
    before = current_sources(args.tests)
    commands = []
    env = os.environ.copy()
    env.update(JAVA_HOME=str(JAVA.parent), JAVACMD=str(JAVA / 'java'), JAVA_OPTS='-Xmx512M -Xms128M', KOTLIN_TOOL='', KOTLIN_RUNNER='', KOTLIN_COMPILER='org.jetbrains.kotlin.cli.jvm.K2JVMCompiler')

    def run(command):
        commands.append([str(value) for value in command])
        result = subprocess.run(commands[-1], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        with (output / 'build.txt').open('ab') as log:
            log.write(result.stdout)
        require(result.returncode == 0, 'manual build command failed; see build.txt')

    run([JAVA / 'javac', '--release', '8', '-d', classes, *java])
    cp = os.pathsep.join(str(path) for path in [*dependencies, android, classes])
    run([TOOL / 'bin/kotlinc', '-language-version', '2.1', '-api-version', '2.1', '-jvm-target', '1.8', '-no-stdlib', '-no-reflect', '-classpath', cp, '-module-name', 'hoyadb', *sources, '-d', classes])
    jar = output / 'classes.jar'
    with zipfile.ZipFile(jar, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(classes.rglob('*')):
            if path.is_file():
                entry = zipfile.ZipInfo(path.relative_to(classes).as_posix(), (1980, 1, 1, 0, 0, 0))
                entry.compress_type = zipfile.ZIP_DEFLATED
                archive.writestr(entry, path.read_bytes())
    runtime = None
    dex_sha = None
    if args.tests:
        dex = output / 'dex'; dex.mkdir()
        if args.split_runtime:
            inputs = {p.name: p for p in [*dependencies, android, d8, d8.parent / 'lib/d8.jar', JAVA / 'java']}
            plan = runtime_plan(inputs, min_api=24, profile='test-runtime-v1')
            parent = ROOT / 'target/phase-seven-kotlin/runtime-cache'
            canonical(parent); parent.mkdir(parents=True, exist_ok=True)
            cache = parent / plan['identity']
            if not cache.exists() and not cache.is_symlink():
                pending = parent / (plan['identity'] + '-pending-' + uuid.uuid4().hex)
                pending.mkdir()
                run([d8, '--lib', android, '--min-api', '24', '--output', pending, *dependencies])
                require({p.name for p in pending.iterdir()} == {'classes.dex'}, 'runtime D8 must produce one DEX')
                (pending / 'manifest.json').write_text(json.dumps({'plan': plan, 'dex_sha256': digest(pending / 'classes.dex')}, sort_keys=True, indent=2) + '\n')
                pending.rename(cache)
            runtime_dex = validate_cache(cache, plan)
            runtime = {'path': str(runtime_dex), 'sha256': digest(runtime_dex), 'plan': plan}
            run([d8, '--lib', android, '--min-api', '24', '--classpath', dependencies[0], '--classpath', dependencies[1], '--output', dex, jar])
        else:
            run([d8, '--lib', android, '--min-api', '24', '--output', dex, jar, *dependencies])
        require({p.name for p in dex.iterdir()} == {'classes.dex'}, 'SDK tests must produce one DEX')
        dex_sha = digest(dex / 'classes.dex')
    after = current_sources(args.tests)
    require(before == after, 'SDK source drift during build')
    require(tools_before == {str(path): digest(path) for path in tool_paths}, 'build tool drift')
    for artifact in reviewed['runtime_artifacts']:
        require(digest(Path(artifact['path'])) == artifact['sha256'], 'runtime archive drift')
    manifest = {'commands': commands, 'compiler': 'installed2.2.20', 'language_api': '2.1',
        'classes_sha256': digest(jar), 'runtime_sha256': {path.name: digest(path) for path in dependencies},
        'test_classes_included': args.tests, 'split_runtime': args.split_runtime, 'runtime_dex': runtime,
        'dex_sha256': dex_sha, 'sources_before': before, 'sources_after': after, 'tools_sha256': tools_before}
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print('manual build complete:', output)


if __name__ == '__main__':
    main()
