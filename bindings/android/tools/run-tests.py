#!/usr/bin/env python3
"""Run one reviewed test class on the task-owned Android runtime only."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from runtime_cache import require, validate_cache

ROOT = Path(__file__).resolve().parents[3]
ADB = '/Users/boristacyniak/Library/Android/sdk/platform-tools/adb'
def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--class-name', required=True)
    parser.add_argument('--log', type=Path, required=True)
    args = parser.parse_args()
    if not args.class_name.isidentifier(): raise SystemExit('invalid test class')
    manifest = json.loads((args.out / 'manifest.json').read_text())
    require(manifest['test_classes_included'], 'not a test artifact')
    dex = args.out / 'dex/classes.dex'
    require(hashlib.sha256(dex.read_bytes()).hexdigest() == manifest['dex_sha256'], 'test DEX changed')
    require(hashlib.sha256((args.out / 'classes.jar').read_bytes()).hexdigest() == manifest['classes_sha256'], 'test classes changed')
    owned = json.loads((ROOT / 'target/phase-seven-kotlin/owned-android-consumer.json').read_text())
    if owned['serial'] != 'emulator-5588' or not owned['remote_root'].startswith('/data/local/tmp/hoyadb-kotlin-owned-'):
        raise SystemExit('invalid owned test identity')
    remote = owned['remote_root'] + '/' + args.class_name + '.dex'
    subprocess.run([ADB, '-s', owned['serial'], 'push', str(dex), remote], check=True)
    classpath = remote
    if manifest.get('split_runtime'):
        runtime = manifest['runtime_dex']
        path = Path(runtime['path'])
        validate_cache(path.parent, runtime['plan'])
        require(hashlib.sha256(path.read_bytes()).hexdigest() == runtime['sha256'], 'test runtime changed')
        remote_runtime = owned['remote_root'] + '/runtime-' + runtime['sha256'] + '.dex'
        subprocess.run([ADB, '-s', owned['serial'], 'push', str(path), remote_runtime], check=True)
        classpath += ':' + remote_runtime
    result = subprocess.run([ADB, '-s', owned['serial'], 'shell', 'CLASSPATH=' + classpath, 'app_process',
        '-Djava.library.path=' + owned['remote_root'], '/system/bin', 'dev.hoyadb.' + args.class_name],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120)
    args.log.parent.mkdir(parents=True, exist_ok=True)
    args.log.write_bytes(result.stdout + ('\nexit=' + str(result.returncode) + '\n').encode())
    args.log.with_suffix('.json').write_text(json.dumps({'classpath': classpath, 'sdk_dex_sha256': manifest['dex_sha256'], 'runtime_dex_sha256': manifest.get('runtime_dex', {}).get('sha256') if manifest.get('runtime_dex') else None,
        'artifact_manifest_sha256': hashlib.sha256((args.out / 'manifest.json').read_bytes()).hexdigest(), 'exit': result.returncode}, indent=2) + '\n')
    print(result.stdout.decode('utf8', errors='replace'), end='')
    raise SystemExit(result.returncode)
if __name__ == '__main__': main()
