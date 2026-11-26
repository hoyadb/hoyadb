#!/usr/bin/env python3
"""Offline local Maven repository assembler and verifier for the RN Android consumer (DEF-0028).

assemble: copy exactly the entries of a trusted input plan into a fresh repository, checking every source
          path, size and hash, then write a success record. verify: recheck a repository against its plan
          and record before any build consumes it.

Every check is an explicit raise (behavior is identical under python -O). The tool performs no network
or subprocess operation: a missing input is a local rejection. A plan is produced by the trusted
activation controller after checking frozen approval records; a plan is not itself dependency approval.

  prepare_repo.py assemble --inputs PLAN.json --out REPO_DIR --record RECORD.json
  prepare_repo.py verify   --inputs PLAN.json --out REPO_DIR --record RECORD.json
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import secrets
import sys

KINDS = ('binary', 'metadata')
HEX64 = re.compile(r'[0-9a-f]{64}')
HEX128 = re.compile(r'[0-9a-f]{128}')


class RepositoryError(Exception):
    def __init__(self, reason, message):
        super().__init__(f'{reason}: {message}')
        self.reason = reason


def _sha(data, algorithm='sha256'):
    return hashlib.new(algorithm, data).hexdigest()


def _safe_relative(value, what):
    if not isinstance(value, str) or not value or '\\' in value or value.startswith('/'):
        raise RepositoryError('unsafe-path', f'{what} must be a relative POSIX path: {value!r}')
    parts = PurePosixPath(value).parts
    if any(part in ('', '.', '..') for part in parts) or '//' in value:
        raise RepositoryError('unsafe-path', f'{what} must not contain empty, "." or ".." segments: {value!r}')
    return value


def _load_plan(plan_path):
    plan_bytes = Path(plan_path).read_bytes()
    try:
        plan = json.loads(plan_bytes)
    except ValueError as error:
        raise RepositoryError('invalid-plan', f'plan is not JSON: {error}')
    if not isinstance(plan, dict) or plan.get('format') != 1:
        raise RepositoryError('invalid-plan', 'plan format must be 1')
    roots = plan.get('roots')
    entries = plan.get('entries')
    if not isinstance(roots, dict) or not roots or not isinstance(entries, list) or not entries:
        raise RepositoryError('invalid-plan', 'plan needs non-empty roots and entries')
    for name, root in roots.items():
        if not isinstance(root, str) or not os.path.isabs(root) or not os.path.isdir(root):
            raise RepositoryError('invalid-plan', f'root {name!r} must be an existing absolute directory')
    for entry in entries:
        if not isinstance(entry, dict):
            raise RepositoryError('invalid-plan', 'entry must be an object')
        _safe_relative(entry.get('path'), 'output path')
        source = entry.get('source')
        if not isinstance(source, dict) or source.get('root') not in roots:
            raise RepositoryError('invalid-plan', f'entry {entry.get("path")!r} has an unknown source root')
        _safe_relative(source.get('path'), 'source path')
        if entry.get('kind') not in KINDS:
            raise RepositoryError('invalid-plan', f'entry {entry["path"]!r} kind must be one of {KINDS}')
        if not isinstance(entry.get('size'), int) or entry['size'] < 0:
            raise RepositoryError('invalid-plan', f'entry {entry["path"]!r} size must be a non-negative integer')
        if not isinstance(entry.get('sha256'), str) or not HEX64.fullmatch(entry['sha256']):
            raise RepositoryError('invalid-plan', f'entry {entry["path"]!r} sha256 must be 64 lowercase hex characters')
        if entry.get('sha512') is not None and not (isinstance(entry['sha512'], str) and HEX128.fullmatch(entry['sha512'])):
            raise RepositoryError('invalid-plan', f'entry {entry["path"]!r} sha512 must be null or 128 lowercase hex characters')
        provenance = entry.get('provenance')
        if not isinstance(provenance, dict) or not isinstance(provenance.get('url'), str) or not isinstance(provenance.get('identity'), str):
            raise RepositoryError('invalid-plan', f'entry {entry["path"]!r} needs provenance url and identity')
    return plan, plan_bytes


def _collect(entries):
    """Index entries by output path. Collection is a plain statement, never part of a validation expression."""
    by_path = {}
    for entry in entries:
        previous = by_path.get(entry['path'])
        if previous is None:
            by_path[entry['path']] = entry
            continue
        if previous['provenance'] != entry['provenance']:
            raise RepositoryError('provenance-conflict', f'{entry["path"]!r} has conflicting provenance identities')
        if (previous['size'], previous['sha256'], previous.get('sha512')) != (entry['size'], entry['sha256'], entry.get('sha512')):
            raise RepositoryError('byte-conflict', f'{entry["path"]!r} has conflicting declared bytes')
        raise RepositoryError('duplicate-entry', f'{entry["path"]!r} is listed more than once')
    return by_path


def _read_source(roots, entry):
    root = os.path.realpath(roots[entry['source']['root']])
    candidate = os.path.join(root, *PurePosixPath(entry['source']['path']).parts)
    resolved = os.path.realpath(candidate)
    if os.path.commonpath([root, resolved]) != root:
        raise RepositoryError('unsafe-path', f'source for {entry["path"]!r} resolves outside its root')
    if not os.path.isfile(resolved):
        raise RepositoryError('missing-input', f'source for {entry["path"]!r} is missing: {entry["source"]}')
    data = Path(resolved).read_bytes()
    if len(data) != entry['size'] or _sha(data) != entry['sha256'] or (entry.get('sha512') is not None and _sha(data, 'sha512') != entry['sha512']):
        raise RepositoryError('integrity', f'source for {entry["path"]!r} does not match its declared size/hashes')
    return data


def _inventory_sha(inventory):
    return _sha(json.dumps(inventory, sort_keys=True, separators=(',', ':')).encode('utf-8'))


def _actual_files(directory):
    actual = {}
    for dirpath, dirnames, filenames in os.walk(directory):
        for name in dirnames + filenames:
            path = os.path.join(dirpath, name)
            if os.path.islink(path):
                raise RepositoryError('unexpected-file', f'symbolic link in repository: {path}')
        for name in filenames:
            path = os.path.join(dirpath, name)
            actual[PurePosixPath(os.path.relpath(path, directory)).as_posix()] = path
    return actual


def _check_tree(directory, expected):
    actual = _actual_files(directory)
    extra = sorted(set(actual) - set(expected))
    if extra:
        raise RepositoryError('unexpected-file', f'unexpected repository files: {extra[:5]}')
    missing = sorted(set(expected) - set(actual))
    if missing:
        raise RepositoryError('missing-file', f'missing repository files: {missing[:5]}')
    for relative, digest in expected.items():
        if _sha(Path(actual[relative]).read_bytes()) != digest:
            raise RepositoryError('integrity', f'repository file changed: {relative}')


def assemble(plan_path, out, record):
    plan, plan_bytes = _load_plan(plan_path)
    by_path = _collect(plan['entries'])
    contents = {path: _read_source(plan['roots'], entry) for path, entry in sorted(by_path.items())}
    out, record = Path(out), Path(record)
    if out.exists() or record.exists():
        raise RepositoryError('output-exists', 'output repository and success record must not exist yet')
    if not out.parent.is_dir():
        raise RepositoryError('invalid-output', f'output parent does not exist: {out.parent}')
    partial = out.parent / f'{out.name}.partial-{secrets.token_hex(8)}'
    partial.mkdir()
    for relative, data in contents.items():
        target = partial.joinpath(*PurePosixPath(relative).parts)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    inventory = {path: entry['sha256'] for path, entry in sorted(by_path.items())}
    _check_tree(partial, inventory)
    os.rename(partial, out)
    counts = {kind: sum(1 for entry in by_path.values() if entry['kind'] == kind) for kind in KINDS}
    success = {'format': 1, 'plan_sha256': _sha(plan_bytes), 'inventory': inventory,
               'inventory_sha256': _inventory_sha(inventory), 'counts': counts}
    temporary = record.parent / f'.{record.name}.partial-{secrets.token_hex(8)}'
    temporary.write_text(json.dumps(success, indent=2, sort_keys=True) + '\n')
    os.replace(temporary, record)
    return {'result': 'assembled', 'plan_sha256': success['plan_sha256'], 'inventory_sha256': success['inventory_sha256'], 'counts': counts}


def verify_repository(plan_path, out, record):
    plan, plan_bytes = _load_plan(plan_path)
    by_path = _collect(plan['entries'])
    expected = {path: entry['sha256'] for path, entry in sorted(by_path.items())}
    record = Path(record)
    if not record.is_file():
        raise RepositoryError('missing-record', f'no success record: {record}')
    try:
        success = json.loads(record.read_bytes())
    except ValueError:
        raise RepositoryError('record-mismatch', 'success record is not JSON')
    if not isinstance(success, dict) or success.get('format') != 1:
        raise RepositoryError('record-mismatch', 'success record format must be 1')
    if success.get('plan_sha256') != _sha(plan_bytes):
        raise RepositoryError('stale-record', 'success record belongs to a different plan')
    counts = {kind: sum(1 for entry in by_path.values() if entry['kind'] == kind) for kind in KINDS}
    if success.get('inventory') != expected or success.get('inventory_sha256') != _inventory_sha(expected) or success.get('counts') != counts:
        raise RepositoryError('record-mismatch', 'success record inventory does not match the plan')
    if not Path(out).is_dir():
        raise RepositoryError('missing-repository', f'repository directory does not exist: {out}')
    _check_tree(out, expected)
    return {'result': 'verified', 'plan_sha256': success['plan_sha256'], 'inventory_sha256': success['inventory_sha256'], 'counts': counts}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('command', choices=['assemble', 'verify'])
    parser.add_argument('--inputs', required=True)
    parser.add_argument('--out', required=True)
    parser.add_argument('--record', required=True)
    args = parser.parse_args(argv)
    action = assemble if args.command == 'assemble' else verify_repository
    try:
        summary = action(args.inputs, args.out, args.record)
    except RepositoryError as error:
        print(f'REJECTED {error}', file=sys.stderr)
        return 2
    except OSError as error:
        print(f'REJECTED io-error: {error}', file=sys.stderr)
        return 2
    print(json.dumps(summary, sort_keys=True))
    return 0


if __name__ == '__main__':
    sys.exit(main())
