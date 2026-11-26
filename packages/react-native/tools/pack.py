#!/usr/bin/env python3
"""Pack the RN package with a validated notice-bearing XCFramework (DEF-0030).

Validates the source XCFramework with the shared Apple notice validator, replaces only the package's
generated Artifacts copy, validates that copy, runs npm pack --ignore-scripts, checks every archive member,
extracts the archive into a fresh directory and validates the extracted copy. The compilation parent
SHA256 is trusted external input for every validation. An existing archive is never overwritten.

Archive members (DEF-0030 AN05), checked before extraction: the only generated exception is the exact
validated XCFramework copy (same member set and bytes); every other member must be byte-identical to its
blob in the commit HEAD resolved to at start (one immutable object ID recorded as head_commit, used for
every lookup). An untracked member rejects unexpected-entry and a tracked member
with different bytes rejects modified-entry; a rejected archive and its record are kept. --repo-root must
be the Git work tree root containing --package-root. Every check is an explicit raise.

  pack.py --source-framework DIR --expected-parent-sha256 HEX --pack-destination DIR --extract-to DIR --record FILE
          [--package-root DIR] [--repo-root DIR]
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tarfile

PACKAGE = Path(__file__).resolve().parents[1]
REPO = Path(__file__).resolve().parents[3]
NATIVE = 'crates/hoyadb-native'


class PackError(Exception):
    def __init__(self, reason, message):
        super().__init__(f'{reason}: {message}')
        self.reason = reason


def _load_notices(repo_root):
    spec = importlib.util.spec_from_file_location('apple_notices', repo_root / NATIVE / 'tools/apple_notices.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _digest(path, algorithm):
    return hashlib.new(algorithm, Path(path).read_bytes()).hexdigest()


def _git(repo_root, *args):
    # Inherited GIT_* variables (GIT_DIR, GIT_WORK_TREE, ...) would override -C; the explicit roots stay authoritative.
    env = {key: value for key, value in os.environ.items() if not key.startswith('GIT_')}
    return subprocess.run(['git', '-C', str(repo_root), *args], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120, env=env)


def _check_git_root(package_root, repo_root):
    if repo_root != package_root and repo_root not in package_root.parents:
        raise PackError('git-root-mismatch', f'package {package_root} is not inside repository root {repo_root}')
    result = _git(package_root, 'rev-parse', '--show-toplevel')
    if result.returncode != 0 or Path(result.stdout.decode().strip()).resolve() != repo_root:
        raise PackError('git-root-mismatch', f'Git work tree of {package_root} is not {repo_root}: {result.stdout.decode().strip()!r}')
    head = _git(repo_root, 'rev-parse', '--verify', 'HEAD^{commit}')
    if head.returncode != 0:
        raise PackError('git-root-mismatch', f'repository {repo_root} has no HEAD commit')
    return head.stdout.decode().strip()


def _relative(member):
    """Package-relative path of a safe regular-file or directory member; anything else is rejected."""
    parts = PurePosixPath(member.name).parts
    if not parts or parts[0] != 'package' or '..' in parts or member.issym() or member.islnk():
        raise PackError('unsafe-archive', f'unexpected archive member {member.name!r}')
    if not (member.isfile() or member.isdir()):
        raise PackError('unsafe-archive', f'unexpected archive member type {member.name!r}')
    return str(PurePosixPath(*parts[1:])) if len(parts) > 1 else '.'


def _check_members(archive, package_root, repo_root, copy, commit):
    """Before extraction: byte identity with the pinned commit for every member except the exact validated framework
    copy. Every lookup uses the resolved object ID, never a symbolic ref that could move during packing."""
    prefix = f'Artifacts/{copy.name}/'
    package_prefix = package_root.relative_to(repo_root).as_posix()
    framework = {p.relative_to(copy).as_posix(): p for p in sorted(copy.rglob('*')) if p.is_file()}
    seen, identical = set(), []
    with tarfile.open(archive, 'r:gz') as packed:
        for member in packed.getmembers():
            relative = _relative(member)
            if not member.isfile():
                continue
            data = packed.extractfile(member).read()
            if relative.startswith(prefix):
                inner = relative[len(prefix):]
                if inner not in framework:
                    raise PackError('unexpected-entry', f'{relative} is not a member of the validated framework copy')
                if framework[inner].read_bytes() != data:
                    raise PackError('framework-mismatch', f'{relative} differs from the validated framework copy')
                seen.add(inner)
                continue
            path = relative if package_prefix == '.' else f'{package_prefix}/{relative}'
            blob = _git(repo_root, 'cat-file', 'blob', f'{commit}:{path}')
            if blob.returncode != 0:
                raise PackError('unexpected-entry', f'{relative} is not a committed file in {commit} ({path})')
            if blob.stdout != data:
                raise PackError('modified-entry', f'{relative} differs from its bytes in {commit} ({path})')
            identical.append(member.name)
    missing = sorted(set(framework) - seen)
    if missing:
        raise PackError('framework-mismatch', f'archive lacks validated framework members: {missing[:5]}')
    return {'head_identical': sorted(identical), 'framework_members': len(seen)}


def _extract(archive, destination):
    destination.mkdir(parents=True)
    with tarfile.open(archive, 'r:gz') as packed:
        for member in packed.getmembers():
            member.name = _relative(member)
            if member.name != '.':
                packed.extract(member, destination, filter='data')


def pack(source, expected, destination, extract_to, record_path, package_root=PACKAGE, repo_root=REPO):
    record = {}
    try:
        return _pack(source, expected, destination, extract_to, record_path, Path(package_root).resolve(),
                     Path(repo_root).resolve(), record)
    except PackError as error:
        if 'pack' in record:  # Preserve failure evidence once npm ran.
            record['error'] = str(error)
            Path(record_path).write_text(json.dumps(record, indent=2, sort_keys=True) + '\n')
        raise


def _pack(source, expected, destination, extract_to, record_path, package_root, repo_root, record):
    # npm runs from the package directory: every caller path is made absolute first.
    source, destination, extract_to, record_path = (Path(p).resolve() for p in (source, destination, extract_to, record_path))
    if extract_to.exists() or record_path.exists():
        raise PackError('output-exists', 'extraction directory and record must not exist yet')
    head = _check_git_root(package_root, repo_root)  # Before loading any validator code from repo_root.
    notices = _load_notices(repo_root)
    manifest = json.loads((package_root / 'package.json').read_bytes())
    archive = destination / (manifest['name'].lstrip('@').replace('/', '-') + f'-{manifest["version"]}.tgz')
    if archive.exists():
        raise PackError('output-exists', f'refusing to overwrite an existing archive: {archive}')
    record.update({'format': 1, 'source_framework': str(source), 'expected_parent_sha256': expected, 'validations': [],
                   'package_root': str(package_root), 'repo_root': str(repo_root), 'head_commit': head})

    def validate(framework, label):
        try:
            summary = notices.validate_framework(framework, repo_root / NATIVE / 'third-party', repo_root / NATIVE / 'Cargo.lock',
                                                 expected)
        except notices.NoticeError as error:
            raise PackError(error.reason, f'{label} copy: {error}')
        record['validations'].append({'label': label, 'path': str(framework), 'summary': summary})

    validate(source, 'source')
    copy = package_root / 'Artifacts' / source.name
    if copy.exists():
        shutil.rmtree(copy)  # Generated, ignored package copy of the previous packager output only.
    copy.parent.mkdir(exist_ok=True)
    shutil.copytree(source, copy)
    validate(copy, 'package-copy')

    destination.mkdir(parents=True, exist_ok=True)
    argv = ['npm', 'pack', '--ignore-scripts', '--json', '--pack-destination', str(destination)]
    result = subprocess.run(argv, cwd=package_root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=600)
    record['pack'] = {'argv': argv, 'cwd': str(package_root), 'exit_code': result.returncode, 'stdout': result.stdout,
                      'stderr': result.stderr}
    if result.returncode != 0:
        raise PackError('pack-failed', result.stderr.strip()[-500:])
    if json.loads(result.stdout)[0]['filename'] != archive.name or not archive.is_file():
        raise PackError('pack-failed', f'npm did not produce {archive.name}')
    record['archive'] = {'file': archive.name, 'sha256': _digest(archive, 'sha256'), 'sha512': _digest(archive, 'sha512')}
    with tarfile.open(archive, 'r:gz') as packed:
        record['archive']['entries'] = sorted(m.name for m in packed.getmembers() if m.isfile())
    record['source_identity'] = _check_members(archive, package_root, repo_root, copy, head)

    _extract(archive, extract_to)
    validate(extract_to / 'Artifacts' / source.name, 'extracted')
    record_path.write_text(json.dumps(record, indent=2, sort_keys=True) + '\n')
    return {'result': 'packed', 'archive': archive.name, 'sha256': record['archive']['sha256'],
            'validations': [v['label'] for v in record['validations']]}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for name in ('--source-framework', '--expected-parent-sha256', '--pack-destination', '--extract-to', '--record'):
        parser.add_argument(name, required=True)
    parser.add_argument('--package-root', default=str(PACKAGE), help='package directory (default: this tool\'s package)')
    parser.add_argument('--repo-root', default=str(REPO), help='Git work tree root (default: this tool\'s repository)')
    args = parser.parse_args(argv)
    try:
        summary = pack(args.source_framework, args.expected_parent_sha256, args.pack_destination, args.extract_to, args.record,
                       args.package_root, args.repo_root)
    except PackError as error:
        print(f'REJECTED {error}', file=sys.stderr)
        return 2
    except OSError as error:
        print(f'REJECTED io-error: {error}', file=sys.stderr)
        return 2
    print(json.dumps(summary, sort_keys=True))
    return 0


if __name__ == '__main__':
    sys.exit(main())
