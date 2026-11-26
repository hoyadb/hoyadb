"""SPEC-0043 / TC-0047: build the native core for the Swift and C guides, then run them.

  python3 docs/public/run_native.py build   # native library, task-owned Swift package and manifest
  python3 docs/public/run_native.py swift   # compile and run examples/swift
  python3 docs/public/run_native.py c       # compile and run examples/c/todo.c

Everything is written under target/documentation-languages. The packaged
bindings/swift/Artifacts and every source file stay untouched: the native core is
built from a `git archive` of HEAD (so uncommitted working-tree edits are not
built), and the Swift guide runs against a copy of bindings/swift whose macOS-only
XCFramework is created from that fresh library. Builds are offline and locked.
"""
import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
OUT = REPO / 'target/documentation-languages'
MANIFEST = OUT / 'native-build.json'
SOURCE = OUT / 'src'
NATIVE = OUT / 'native'
LIBRARY = NATIVE / 'release/libhoyadb_native.a'
PACKAGE = OUT / 'swift-package'
XCFRAMEWORK = PACKAGE / 'Artifacts/HoyaNative.xcframework'
GUIDE = OUT / 'swift-guide'
ARCHIVED = ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'crates', 'crates/hoyadb-native', 'bindings/swift']
WORKSPACE_ROOT = '\n# Task-owned documentation build copy: standalone workspace root (not in the original file).\n[workspace]\n'
PUBLIC_DEPENDENCY = 'path: "../../bindings/swift"'
COPY_DEPENDENCY = 'path: "../swift-package"'


def fail(message):
    raise SystemExit(f'FAIL {message}')


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return path.relative_to(REPO).as_posix()


def files(base):
    return sorted(p for p in base.rglob('*') if p.is_file())


def require_inside(path):
    """Refuses any path outside OUT, or reached through a symlink, so no write can escape it."""
    if not path.is_absolute() or '..' in path.parts or not path.is_relative_to(OUT):
        fail(f'path escapes {relative(OUT)}: {path}')
    for current in [path, *path.parents]:
        if current.is_symlink():
            fail(f'symlink in a task-owned path: {current}')
        if current == OUT.parent:
            break
    if not path.resolve().is_relative_to(OUT.resolve()):
        fail(f'path escapes {relative(OUT)}: {path}')


def reset(path):
    require_inside(path)
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True)


def no_links(base):
    for path in base.rglob('*'):
        if path.is_symlink():
            fail(f'unexpected symlink: {relative(path)}')


def run(name, args, cwd, env=None, log=None):
    print('$', ' '.join(str(a) for a in args), flush=True)
    result = subprocess.run([str(a) for a in args], cwd=cwd, env=env, capture_output=True, text=True)
    if log:
        log.write_text(result.stdout + result.stderr)
    if result.returncode:
        sys.stdout.write(result.stdout + result.stderr)
        fail(f'{name} exited {result.returncode}')
    return result


def tool_versions():
    def first(args, cwd=REPO):
        result = subprocess.run(args, cwd=cwd, capture_output=True, text=True)
        return (result.stdout + result.stderr).strip()
    return {'rustc': first(['rustc', '-vV'], SOURCE), 'cargo': first(['cargo', '-V'], SOURCE),
            'swift': first(['swift', '--version']), 'xcodebuild': first(['xcodebuild', '-version']),
            'cc': first(['cc', '--version']).splitlines()[0]}


def archived_native_path(repo, commit):
    """Read the selected commit's layout, never substitute working-tree source."""
    for path in ['crates/hoyadb-native', 'bindings/native']:
        probe = subprocess.run(['git', 'cat-file', '-e', f'{commit}:{path}/Cargo.toml'],
                               cwd=repo, capture_output=True)
        if probe.returncode == 0:
            return path
    fail(f'native crate not found in commit {commit}')


def extract_head(commit):
    native_path = archived_native_path(REPO, commit)
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


def build():
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()
    differing = subprocess.check_output(['git', 'status', '--porcelain', '--', *ARCHIVED], cwd=REPO,
                                        text=True).splitlines()
    commands = []

    reset(SOURCE)
    native_path, archive_paths = extract_head(commit)
    native_toml = SOURCE / native_path / 'Cargo.toml'
    original_toml = sha256(native_toml)
    with native_toml.open('a') as handle:
        handle.write(WORKSPACE_ROOT)
    inputs = {path.relative_to(SOURCE).as_posix(): sha256(path) for path in files(SOURCE)}

    require_inside(NATIVE)
    env = dict(os.environ, CARGO_TARGET_DIR=str(NATIVE), MACOSX_DEPLOYMENT_TARGET='13.0')
    cargo = ['cargo', 'build', '--manifest-path', f'{native_path}/Cargo.toml', '--release', '--offline', '--locked']
    run('native build', cargo, SOURCE, env, OUT / 'native-build.log')
    commands.append({'cwd': relative(SOURCE), 'env': {'CARGO_TARGET_DIR': relative(NATIVE),
                     'MACOSX_DEPLOYMENT_TARGET': '13.0'}, 'command': cargo})

    reset(PACKAGE)
    shutil.copytree(SOURCE / 'bindings/swift', PACKAGE, symlinks=True, dirs_exist_ok=True)
    no_links(PACKAGE)
    xcodebuild = ['xcodebuild', '-create-xcframework', '-library', relative(LIBRARY),
                  '-headers', relative(SOURCE / native_path / 'include'), '-output', relative(XCFRAMEWORK)]
    run('xcframework', xcodebuild, REPO, log=OUT / 'xcframework.log')
    commands.append({'cwd': '.', 'command': xcodebuild})

    artifacts = {relative(path): sha256(path) for path in [LIBRARY, *files(XCFRAMEWORK)]}
    manifest = {
        'spec': 'SPEC-0043 / TC-0047',
        'source': {'commit': commit, 'kind': 'git archive of HEAD', 'paths': archive_paths,
                   'working_tree_differs_from_head': differing,
                   'modified_copy': {'path': f'{native_path}/Cargo.toml', 'original_sha256': original_toml,
                                     'appended': WORKSPACE_ROOT}},
        'tools': tool_versions(),
        'commands': commands,
        'inputs': inputs,
        'artifacts': artifacts,
        'swift_inputs':{relative(path):sha256(path) for path in [PACKAGE/'Package.swift', *files(PACKAGE/'Sources')]},
        'package': {'path': relative(PACKAGE), 'xcframework': relative(XCFRAMEWORK), 'platforms': ['macOS']},
    }
    MANIFEST.write_text(json.dumps(manifest, indent=2) + '\n')
    print(f'PASS native build from {commit}; {len(artifacts)} artifacts recorded in {relative(MANIFEST)}')


def built_artifacts():
    """Fails unless the artifacts on disk are exactly the ones the last build recorded."""
    if not MANIFEST.is_file():
        fail(f'{relative(MANIFEST)} is missing; run `python3 docs/public/run_native.py build` first')
    report = json.loads(MANIFEST.read_text())
    recorded = report['artifacts']
    inputs = report.get('swift_inputs')
    if not isinstance(inputs, dict) or not inputs:
        fail('missing swift_inputs in native manifest; rebuild')
    for rel, digest in {**recorded, **inputs}.items():
        path = REPO / rel
        require_inside(path)
        if not path.is_file() or sha256(path) != digest:
            fail(f'{rel} differs from {relative(MANIFEST)}; rebuild')
    current_inputs = {relative(path) for path in [PACKAGE/'Package.swift', *files(PACKAGE/'Sources')]}
    if current_inputs != set(inputs):
        fail('unrecorded or missing Swift compile inputs; rebuild')
    current = {relative(path) for path in files(XCFRAMEWORK)}
    if not current <= set(recorded):
        fail(f'unrecorded files in {relative(XCFRAMEWORK)}')


def expect_completion(result, line):
    """Requires the example's own final line, then ends this runner's output with that same line."""
    lines = result.stdout.splitlines()
    if not lines or lines[-1] != line:
        sys.stdout.write(result.stdout + result.stderr)
        fail(f'missing completion line {line!r}')
    sys.stderr.write(result.stderr)
    print(line, flush=True)


def swift():
    built_artifacts()
    reset(GUIDE)
    shutil.copytree(REPO / 'examples/swift', GUIDE, symlinks=True, dirs_exist_ok=True)
    no_links(GUIDE)
    manifest = GUIDE / 'Package.swift'
    text = manifest.read_text()
    if text.count(PUBLIC_DEPENDENCY) != 1:
        fail(f'expected exactly one {PUBLIC_DEPENDENCY} in the Swift guide Package.swift')
    manifest.write_text(text.replace(PUBLIC_DEPENDENCY, COPY_DEPENDENCY))
    paths = {f'--{name}-path': OUT / f'swift-{name}' for name in ['scratch', 'cache', 'config', 'security']}
    for path in paths.values():
        require_inside(path)
    args = ['swift', 'run', '--package-path', GUIDE, *[str(a) for pair in paths.items() for a in pair],
            '-Xswiftc', '-warnings-as-errors', 'TodoGuide']
    expect_completion(run('swift guide', args, REPO, log=OUT / 'swift-run.log'), 'swift tour: ok')


def c():
    built_artifacts()
    native_manifest = json.loads(MANIFEST.read_text())['source']['modified_copy']['path']
    headers = SOURCE / Path(native_manifest).parent / 'include'
    require_inside(headers)
    binary = OUT / 'c/todo'
    reset(binary.parent)
    args = ['cc', '-std=c11', '-Wall', '-Wextra', '-Werror', 'examples/c/todo.c',
            '-I', relative(headers), relative(LIBRARY),
            '-framework', 'Security', '-framework', 'CoreFoundation', '-o', relative(binary)]
    run('c compile', args, REPO, log=OUT / 'c-compile.log')
    expect_completion(run('c guide', [binary], REPO, log=OUT / 'c-run.log'), 'c tour: ok')


if __name__ == '__main__':
    commands = {'build': build, 'swift': swift, 'c': c}
    if len(sys.argv) != 2 or sys.argv[1] not in commands:
        raise SystemExit('usage: python3 docs/public/run_native.py build|swift|c')
    commands[sys.argv[1]]()
