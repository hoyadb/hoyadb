"""Static package-footprint/input alignment for the DEF-0026 client tarball. No browser, worker or timing.

Usage: python3 -I package_alignment.py <repo> <new-client.tgz> <testing.tgz> <old-client.tgz> <historical-prepare-manifest> <output.json>
Tarballs are read only; installation happens in a fresh owned OS temporary directory that is removed.
"""
import hashlib, json, platform, shutil, subprocess, sys, tarfile, tempfile
from pathlib import Path

def sha(data): return hashlib.sha256(data).hexdigest()

def members(path):
    result = {}
    with tarfile.open(path, 'r:gz') as archive:
        for member in archive.getmembers():
            if member.isfile():
                data = archive.extractfile(member).read()
                result[member.name] = {'bytes': len(data), 'sha256': sha(data)}
            elif not member.isdir():
                raise SystemExit('unexpected non-regular tar member: ' + member.name)
    return result

def tool(command):
    done = subprocess.run(command, capture_output=True, text=True, check=True)
    return done.stdout.strip()

def main():
    repo, new_client, testing, old_client, historical, output = [Path(value) for value in sys.argv[1:7]]
    sys.path.insert(0, str(repo / 'benchmarks'))
    from browser_run import package_footprint  # existing collector, unchanged
    tarballs = {name: {'bytes': path.stat().st_size, 'sha256': sha(path.read_bytes())}
                for name, path in (('new_client', new_client), ('testing', testing), ('old_client', old_client))}
    old = {'client/' + k: v for k, v in members(old_client).items()} | {'testing/' + k: v for k, v in members(testing).items()}
    new = {'client/' + k: v for k, v in members(new_client).items()} | {'testing/' + k: v for k, v in members(testing).items()}
    changed = {name: {'before': old[name], 'after': new[name]} for name in sorted(old) if name in new and old[name] != new[name]}
    with tempfile.TemporaryDirectory(prefix='hoyadb-package-alignment-') as temporary:
        root = Path(temporary)
        app, archives = root / 'app', root / 'archives'
        app.mkdir(); archives.mkdir()
        copies = []
        for path in (new_client, testing):
            target = archives / path.name
            shutil.copyfile(path, target)
            copies.append({'path': 'owned-temp/archives/' + path.name, 'bytes': target.stat().st_size, 'sha256': sha(target.read_bytes())})
        (app / 'package.json').write_text(json.dumps({'private': True, 'type': 'module'}))
        install = ['npm', 'install', '--offline', '--ignore-scripts', '--no-audit', '--no-fund', str(archives / new_client.name), str(archives / testing.name)]
        subprocess.run(install, cwd=app, capture_output=True, text=True, check=True)
        lock = json.loads((app / 'package-lock.json').read_text())
        graph = sorted(lock.get('packages', {}))
        if graph != ['', 'node_modules/@hoyadb/client', 'node_modules/@hoyadb/testing']:
            raise SystemExit('unexpected installed dependency graph')
        footprint = package_footprint(app, copies)
        fidelity = []
        for package, prefix in (('client', 'client/package/'), ('testing', 'testing/package/')):
            installed = app / 'node_modules/@hoyadb' / package
            for name, record in new.items():
                if name.startswith(prefix):
                    data = (installed / name[len(prefix):]).read_bytes()
                    if {'bytes': len(data), 'sha256': sha(data)} != record:
                        fidelity.append(name)
            extra = {p.relative_to(installed).as_posix() for p in installed.rglob('*') if p.is_file()} - {n[len(prefix):] for n in new if n.startswith(prefix)}
            fidelity += ['extra installed file: ' + package + '/' + e for e in sorted(extra)]
        temp_path = str(root)
    historical_manifest = json.loads(historical.read_text())
    previous = historical_manifest['package_footprint']['packages']
    deltas = {}
    for package, current in footprint['packages'].items():
        for name in sorted(set(current['files']) | set(previous[package]['files'])):
            if current['files'].get(name) != previous[package]['files'].get(name):
                deltas[package + '/' + name] = {'historical': previous[package]['files'].get(name), 'current': current['files'].get(name)}
    result = {'format': 1, 'kind': 'static package footprint/input alignment; no browser, worker, timing or memory measurement',
              'tarballs': tarballs, 'tar_file_count': {'old': len(old), 'new': len(new)}, 'same_file_set': set(old) == set(new),
              'changed_tar_members': changed, 'unchanged_tar_members': sum(1 for n in old if n in new and old[n] == new[n]),
              'install_command': install[:6] + ['<owned-temp>/' + new_client.name, '<owned-temp>/' + testing.name],
              'installed_graph': graph, 'install_fidelity_mismatches': fidelity, 'footprint': footprint,
              'historical_footprint': {'manifest': str(historical), 'manifest_sha256': sha(historical.read_bytes()),
                                       # Repository-relative archive paths; the historical manifest itself is unchanged.
                                       'archives': [{**record, 'path': Path(record['path']).resolve().relative_to(repo.resolve()).as_posix()}
                                                    for record in historical_manifest['package_footprint']['archives']]},
              'installed_footprint_deltas_vs_historical': deltas,
              'owned_temp_removed': not Path(temp_path).exists(),
              'tools': {'python': platform.python_version(), 'node': tool(['node', '--version']), 'npm': tool(['npm', '--version']),
                        'script_sha256': sha(Path(__file__).read_bytes()), 'collector': 'benchmarks/browser_run.py package_footprint',
                        'collector_file_sha256': sha((repo / 'benchmarks/browser_run.py').read_bytes())}}
    output.write_text(json.dumps(result, indent=1, sort_keys=True) + '\n')

if __name__ == '__main__':
    main()
