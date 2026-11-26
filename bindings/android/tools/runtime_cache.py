"""Private reviewed test-runtime plan; never contains an SDK or test class."""
import hashlib
import json
from pathlib import Path

NAMES = ['kotlin-stdlib-2.1.20.jar', 'annotations-13.0.jar', 'android.jar', 'd8', 'd8.jar', 'java']


def require(value, reason):
    if not value:
        raise SystemExit(reason)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def canonical(path):
    path = Path(path).absolute()
    require(not path.is_symlink() and path == path.resolve(), 'symlinked build input')
    return path


def runtime_plan(inputs, *, min_api, profile):
    require(min_api == 24 and profile == 'test-runtime-v1', 'unsupported runtime profile')
    require(set(inputs) == set(NAMES), 'incomplete runtime inputs')
    hashes = {}
    for name in NAMES:
        path = canonical(inputs[name])
        require(path.is_file(), 'missing runtime input')
        hashes[name] = digest(path)
    plan = {'program_inputs': NAMES[:2], 'inputs_sha256': hashes, 'min_api': 24,
        'profile': profile, 'd8_flags': ['--lib', 'android.jar', '--min-api', '24']}
    plan['identity'] = hashlib.sha256(json.dumps(plan, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    return plan


def validate_cache(directory, plan):
    directory = canonical(directory)
    require(directory.is_dir() and directory.name == plan['identity'], 'runtime cache identity mismatch')
    require({p.name for p in directory.iterdir()} == {'manifest.json', 'classes.dex'}, 'incomplete runtime cache')
    manifest_path, dex = directory / 'manifest.json', directory / 'classes.dex'
    for path in (manifest_path, dex):
        canonical(path)
        require(path.is_file(), 'incomplete runtime cache')
    try:
        manifest = json.loads(manifest_path.read_text())
    except (ValueError, OSError):
        raise SystemExit('malformed runtime cache')
    require(isinstance(manifest, dict) and set(manifest) == {'plan', 'dex_sha256'}, 'malformed runtime cache')
    require(manifest['plan'] == plan and manifest['dex_sha256'] == digest(dex), 'runtime cache digest mismatch')
    return dex


def fresh_output(directory):
    directory = canonical(directory)
    require(not directory.exists(), 'build output must be fresh')
    directory.mkdir(parents=True)
    return directory
