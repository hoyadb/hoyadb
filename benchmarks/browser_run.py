#!/usr/bin/env python3
"""Installed browser benchmark application; measurements disabled until admission review."""
import hashlib
import json
from pathlib import Path
import re
import shutil


def require(condition, message):
    if not condition:
        raise ValueError(message)


def prepare_application(repo, app, output, command):
    repo, app, output = [Path(path).resolve(strict=True) for path in (repo, app, output)]
    require(app.is_dir() and not any(app.iterdir()), 'application root must be new and empty')
    archives_dir = output / 'archives'
    archives_dir.mkdir()
    archives = []
    for name in ('client', 'testing'):
        package = repo / 'tools/javascript/dist' / name
        manifest = json.loads((package / 'package.json').read_text())
        require(manifest.get('name') == '@hoyadb/' + name and manifest.get('version') == '0.1.0', 'unexpected SDK package identity')
        packed = json.loads(command(['npm', 'pack', '--json', '--ignore-scripts', '--pack-destination', str(archives_dir)], package))
        filename = 'hoyadb-' + name + '-0.1.0.tgz'
        require(isinstance(packed, list) and len(packed) == 1 and packed[0].get('filename') == filename, 'unexpected archive result')
        archive = archives_dir / filename
        require(archive.is_file() and not archive.is_symlink(), 'missing or aliased archive')
        data = archive.read_bytes()
        archives.append({'path': str(archive), 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
    (app / 'package.json').write_text(json.dumps({'private': True, 'type': 'module'}))
    command(['npm', 'install', '--offline', '--ignore-scripts', '--no-audit', '--no-fund', *[record['path'] for record in archives]], app)
    installed = json.loads((app / 'package-lock.json').read_text())
    require(set(installed.get('packages', {})) == {'', 'node_modules/@hoyadb/client', 'node_modules/@hoyadb/testing'}, 'unexpected installed dependency graph')
    for path in sorted((repo / 'tools/javascript/benchmark').iterdir()):
        if path.suffix in ('.mjs', '.html') and not path.name.endswith('.test.mjs'):
            require(path.is_file() and not path.is_symlink(), 'aliased harness source')
            shutil.copyfile(path, app / path.name)
    for name in ('control-client.mjs', 'control-worker.mjs'):
        source = (repo / 'tests/packaging/javascript' / name).read_text()
        source = source.replace("'../../../packages/client/src/index.js'", "'@hoyadb/client/host'")
        source = source.replace("'../../../packages/client/src/worker-host.js'", "'./node_modules/@hoyadb/client/src/worker-host.js'")
        source = source.replace("'../../../crates/hoyadb-web/pkg-test/hoyadb_web.js'", "'./test-wasm/hoyadb_web.js'")
        (app / name).write_text(source)
    controls = repo / 'crates/hoyadb-web/pkg-test'
    require(controls.is_dir() and not controls.is_symlink(), 'missing instrumented artifact')
    shutil.copytree(controls, app / 'test-wasm')
    return {'archives': archives, 'application': snapshot(app, ['.'])}

def snapshot(root, paths):
    root = Path(root).resolve(strict=True)
    files = {}
    for relative in sorted(set(paths)):
        require(isinstance(relative, str) and not Path(relative).is_absolute() and '..' not in Path(relative).parts, 'invalid source root')
        path = root / relative
        require(path.exists() and not path.is_symlink(), 'missing or aliased source')
        selected = sorted(path.rglob('*')) if path.is_dir() else [path]
        for candidate in selected:
            require(not candidate.is_symlink(), 'aliased source inventory')
            if candidate.is_file():
                files[candidate.relative_to(root).as_posix()] = hashlib.sha256(candidate.read_bytes()).hexdigest()
    require(files, 'empty source inventory')
    files = dict(sorted(files.items()))
    canonical = json.dumps(files, sort_keys=True, separators=(',', ':')).encode()
    return {'roots': sorted(set(paths)), 'files': files, 'sha256': hashlib.sha256(canonical).hexdigest()}

def verify_snapshot(root, before):
    current = snapshot(root, before['roots'])
    require(current == before, 'frozen source changed')
    return current


def release_reference(repo, app, relative):
    repo, app = Path(repo).resolve(strict=True), Path(app).resolve(strict=True)
    path = repo / relative
    require(not Path(relative).is_absolute() and '..' not in Path(relative).parts and path.is_file() and not path.is_symlink(), 'invalid release reference')
    data = path.read_bytes()
    reference = json.loads(data)
    require(reference.get('status') == 'passed', 'release reference not passed')
    artifacts = reference.get('source_closure', {}).get('artifacts')
    require(isinstance(artifacts, list) and artifacts, 'missing artifact reference')
    for record in artifacts:
        name = record.get('path', '')
        require(name.startswith(('crates/hoyadb-web/pkg/', 'crates/hoyadb-web/pkg-test/')) and '..' not in Path(name).parts, 'unexpected release artifact path')
        artifact = repo / name
        require(artifact.is_file() and not artifact.is_symlink(), 'missing release artifact')
        contents = artifact.read_bytes()
        require(len(contents) == record['bytes'] and hashlib.sha256(contents).hexdigest() == record['sha256'], 'release artifact differs from verified reference')
        if name.startswith('crates/hoyadb-web/pkg/'):
            installed = app / 'node_modules/@hoyadb/client/src/wasm' / artifact.name
            require(installed.is_file() and not installed.is_symlink() and installed.read_bytes() == contents, 'installed ordinary asset differs')
    return {'path': relative, 'sha256': hashlib.sha256(data).hexdigest(), 'artifacts': artifacts}


def package_footprint(app, archives):
    """Actual installed package bytes; never Rust executable or source-tree sizes."""
    import gzip
    packages = {}
    for name in ('client', 'testing'):
        root = Path(app) / 'node_modules/@hoyadb' / name
        require(root.is_dir() and not root.is_symlink(), 'missing installed package')
        files = {}
        for path in sorted(root.rglob('*')):
            require(not path.is_symlink(), 'aliased installed package file')
            if not path.is_file():
                continue
            data = path.read_bytes()
            upper = path.name.upper()
            kind = 'wasm' if path.suffix == '.wasm' else 'javascript' if path.suffix in ('.js', '.mjs') else \
                'notice' if upper.startswith(('LICENSE', 'NOTICE', 'COPYING')) else 'other'
            files[path.relative_to(root).as_posix()] = {'kind': kind, 'bytes': len(data), 'gzip_bytes': len(gzip.compress(data, compresslevel=9, mtime=0))}
        packages['@hoyadb/' + name] = {'files': files, 'total_bytes': sum(item['bytes'] for item in files.values()),
                                       'by_kind': {kind: sum(item['bytes'] for item in files.values() if item['kind'] == kind) for kind in ('wasm', 'javascript', 'notice', 'other')}}
    return {'archives': archives, 'packages': packages, 'gzip_command': 'python3 gzip.compress(data, compresslevel=9, mtime=0) per file'}


def accepted_samples(states):
    # Cold runs have one measured sample, warm runs thirty; untimed resource runs none.
    return sum({'cold': 1, 'resources': 0}.get(state.get('kind'), 30) for state in states)


def control_loop(stream, collector, emit):
    """Parent-only admission channel; the served page has no admission route."""
    from run import RunnerError, canonical, strict_json
    for raw in stream:
        command, _, body = raw.rstrip('\n').partition(' ')
        try:
            try:
                value = strict_json(body)
                require(isinstance(value, dict) and len(raw) <= 65536, 'malformed control input')
            except ValueError:
                collector.reject('malformed control input')
            if command == 'ADMIT':
                require(set(value) <= {'coordinates', 'environment'} and 'coordinates' in value, 'malformed admission')
                emit('ADMITTED ' + canonical(collector.admit(value['coordinates'], value.get('environment'))))
            elif command == 'DECLARE':
                collector.declare(value)
                emit('DECLARED ' + canonical(value))
            elif command == 'RECOVER':
                require(set(value) == {'run_id', 'reason'}, 'malformed recovery')
                collector.recover(value['run_id'], value['reason'])
                emit('RECOVERED ' + canonical(value))
            elif command == 'REVOKE':
                require(set(value) == {'reason'}, 'malformed revocation')
                collector.revoke(value['reason'])
                emit('REVOKED ' + canonical(value))
            elif command == 'CANCEL':
                collector.cancel(str(value.get('reason', 'parent cancelled')))
                emit('CANCELLED ' + canonical(value))
                return 'cancelled'
            else:
                collector.reject('unknown control command')
        except ValueError as error:
            if not isinstance(error, RunnerError):
                try:
                    collector.reject(str(error))
                except RunnerError:
                    pass
            emit('REJECTED ' + canonical({'reason': str(error)}))
    collector.cancel('control input closed')
    emit('CANCELLED ' + canonical({'reason': 'control input closed'}))
    return 'closed'


BROWSER_SOURCE_PATHS = [
    'Cargo.lock', 'rust-toolchain.toml', 'crates/hoyadb-web/Cargo.lock',
    'crates/hoyadb-web/src', 'crates/hoyadb-web/pkg', 'crates/hoyadb-web/pkg-test',
    'crates/hoyadb-bindings/src', 'crates/hoyadb-core/src',
    'crates/hoyadb-storage-memory/src', 'crates/hoyadb-storage-sqlite/src',
    'packages/client/src', 'packages/client/package.json',
    'packages/testing/src', 'packages/testing/package.json',
    'tools/javascript/benchmark', 'tests/packaging/javascript/control-client.mjs',
    'tests/packaging/javascript/control-worker.mjs', 'benchmarks/browser_run.py',
    'benchmarks/browser_server.py', 'benchmarks/browser-workloads.json',
    'benchmarks/workloads.json', 'benchmarks/browser-plan.md', 'benchmarks/PLAN.md',
    'benchmarks/run.py', 'benchmarks/summary.py',
]


def arguments(argv=None):
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, help='new immutable evidence directory')
    parser.add_argument('--port', type=int, default=8879)
    parser.add_argument('--prepare-only', action='store_true', help='verify installation and remove owned application without starting server')
    parser.add_argument('--release-reference', help='explicit verified artifact reference JSON')
    parser.add_argument('--admission', choices=['none', 'stdin'], default='none',
                        help='stdin enables measurements only through explicit parent ADMIT permits')
    parser.add_argument('--run-status', choices=['ok', 'contaminated'], default='contaminated',
                        help='ok requires an independently coordinated quiet window attested by the caller')
    args = parser.parse_args(argv)
    # A07: reject before any output directory, installation, server or worker acquisition.
    if args.prepare_only and args.admission != 'none':
        parser.error('prepare-only cannot admit measurements')
    if args.run_status == 'ok' and args.admission != 'stdin':
        parser.error('--run-status ok requires --admission stdin')
    if not 0 <= args.port <= 65535:
        parser.error('invalid loopback port')
    if not args.release_reference:
        parser.error('--release-reference is required; supply a verified artifact reference')
    return args


def main():
    import signal
    import subprocess
    import tempfile
    from browser_server import BrowserCollector, create_server
    from run import canonical, environment, write_new

    args = arguments()
    repo = Path(__file__).resolve().parents[1]
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    before = snapshot(repo, BROWSER_SOURCE_PATHS)
    write_new(output / 'source-before.json', before)
    manifest = {'format': 1, 'status': 'preparing', 'measurements_allowed': False,
                'accepted_samples': 0, 'source_sha256': before['sha256'],
                'environment': environment(), 'command': ['python3', 'benchmarks/browser_run.py', *list(__import__('sys').argv[1:])]}
    server = None

    def stop(*_):
        raise KeyboardInterrupt

    previous = signal.signal(signal.SIGTERM, stop)
    try:
        with tempfile.TemporaryDirectory(prefix='hoyadb-browser-benchmark-') as temporary:
            app = Path(temporary)
            manifest['owned_application_root'] = str(app)

            def command(command_args, cwd):
                with (output / 'installer.log').open('a') as log:
                    log.write(canonical({'command': command_args, 'cwd': str(cwd)}) + '\n')
                    result = subprocess.run(command_args, cwd=cwd, stdout=subprocess.PIPE,
                                            stderr=log, text=True, timeout=120, check=False)
                    log.write(result.stdout)
                    require(result.returncode == 0, 'local npm command failed')
                    return result.stdout

            installation = prepare_application(repo, app, output, command)
            reference = release_reference(repo, app, args.release_reference)
            verify_snapshot(repo, before)
            manifest.update({'installation': installation, 'release_reference': reference, 'status': 'prepared',
                             'package_footprint': package_footprint(app, installation['archives'])})
            write_new(output / 'manifest-prepared.json', manifest)
            if not args.prepare_only:
                metadata = json.loads((repo / 'benchmarks/browser-workloads.json').read_text())
                parent = json.loads((repo / 'benchmarks/workloads.json').read_text())
                ordinary = app / 'node_modules/@hoyadb/client/src/wasm/hoyadb_web_bg.wasm'
                artifact = hashlib.sha256(ordinary.read_bytes()).hexdigest()
                admitted = args.admission == 'stdin'
                installed = installation['application']

                def recheck():
                    # Frozen repository closure, installed application bytes and ordinary WASM.
                    current = snapshot(repo, BROWSER_SOURCE_PATHS)['sha256']
                    return (current if snapshot(app, ['.']) == installed else 'installed-application-drift',
                            hashlib.sha256(ordinary.read_bytes()).hexdigest())

                def emit(line):
                    print(line, flush=True)

                collector = BrowserCollector(output / 'raw', metadata, parent, before['sha256'], artifact,
                                             allow_measurements=admitted, recheck=recheck, run_status=args.run_status,
                                             on_held=lambda held: emit('BETWEEN_WORKERS ' + canonical(held)))
                server = create_server(app, collector, args.port)
                manifest['measurements_allowed'] = admitted
                manifest['admission'] = args.admission
                manifest['page_permit_wait_seconds'] = collector.next_wait_seconds
                manifest['run_status'] = args.run_status
                manifest['quiet_window_attested_by_caller'] = args.run_status == 'ok'
                print(('BROWSER_ADMISSION_SERVER ' if admitted else 'BROWSER_VERIFY_SERVER ') + canonical({'url': 'http://127.0.0.1:' + str(server.server_port) + '/browser.html', 'measurements_allowed': admitted, 'owned_root': str(app)}), flush=True)
                if admitted:
                    import sys
                    import threading

                    def control():
                        try:
                            manifest['control_result'] = control_loop(sys.stdin, collector, emit)
                        finally:
                            server.shutdown()
                    threading.Thread(target=control, daemon=True).start()
                try:
                    server.serve_forever()
                except KeyboardInterrupt:
                    if admitted and collector.state != 'cancelled':
                        collector.cancel('controller interrupted')
                finally:
                    # Wake any bounded page permit wait before closing the server.
                    collector.shutdown()
                    server.server_close()
                accepted = {run: state for run, state in collector.runs.items() if (output / 'raw' / (run + '.json')).exists()}
                manifest['accepted_runs'] = sorted(accepted)
                manifest['accepted_samples'] = accepted_samples(accepted.values())
            manifest['status'] = 'closed_after_admission' if args.admission == 'stdin' else 'closed_without_measurements'
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        manifest['status'] = 'failed'
        manifest['failure'] = type(error).__name__
        raise
    finally:
        signal.signal(signal.SIGTERM, previous)
        try:
            current = verify_snapshot(repo, before)
            manifest['source_unchanged'] = True
            write_new(output / 'source-after.json', current)
        except (OSError, ValueError):
            manifest['source_unchanged'] = False
            if manifest.get('accepted_runs'):
                # A changed closure invalidates every run accepted in this session.
                manifest['status'] = 'invalidated_source_changed'
        manifest['owned_application_removed'] = not Path(manifest.get('owned_application_root', '/')).exists()
        write_new(output / 'manifest-final.json', manifest)


if __name__ == '__main__':
    main()
