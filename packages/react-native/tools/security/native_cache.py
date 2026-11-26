"""SPEC-0034 / TC-0038 SR-22a/b: reviewed macOS system shared-cache identities for observed native loads.

Pure functions over three externally supplied records: the reviewed expected host/cache record and two host
records read independently before and after the capture (OS identity, complete cache-file SHA256 manifest,
cache header values, image identities re-read from the cache and per-path file status). Nothing here reads
the host, the cache or a process.

Source semantics (studied, pinned by `capture_source`): JDK 17.0.1+12 `jdk.NativeLibrary` reports per dyld
image the image path, the Mach-O header address as `baseAddress` and `topAddress` 0 on macOS; dyld places every
cached image at its unslid `__TEXT` address plus one shared-region slide that xnu draws below `maxSlide` and
aligns to the architecture's slide page (4 KiB for the studied x86_64h cache). JFR does not report which cache
a process mapped: association with the rehashed host cache is an inference under the trusted host boundary,
not process attestation.

SR-22d: `hoyadb.system-shared-cache/2` adds `dual_resident_images`, an explicit canonical subset of `images` whose
paths are also regular on-disk files, each bound to the on-disk file's size and SHA256 as re-read by both host
records (`dual_resident_files`). That binds the disk copies only, never the mapped cache bytes; the complete
cache-file manifest stays the cache integrity anchor. Version 1 is unchanged and rejects every real-file cache path.
The on-disk/cache Mach-O UUID agreement is review-time evidence, not checked here.
"""
from dataclasses import dataclass
import re

SCHEMA = 'hoyadb.system-shared-cache/1'
SCHEMA_V2 = 'hoyadb.system-shared-cache/2'
HOST_IDENTITY_KEYS = ('os', 'host_integrity', 'arch', 'cache_dir', 'cache', 'cache_files')
OS_KEYS = ('product_version', 'product_build_version', 'build_id', 'product_version_extra', 'system_version_plist_sha256')
INTEGRITY_KEYS = ('sip_enabled', 'authenticated_root', 'system_volume_sealed')
CACHE_INT_KEYS = ('cache_type', 'platform', 'os_version_field', 'shared_region_start', 'shared_region_size', 'max_slide', 'images_count')
CAPTURE_KEYS = ('jdk', 'event', 'base_address', 'top_address')
IMAGE_KEYS = ('install_path', 'image_uuid', 'unslid_load_address', 'text_size')
IDENTITY_KEYS = IMAGE_KEYS[1:]
RECORD_KEYS = ('schema', *HOST_IDENTITY_KEYS, 'slide_alignment', 'capture_source', 'images', 'aliases', 'mandatory_images')
REQUIRED_MANDATORY_IMAGES = ('/usr/lib/libSystem.B.dylib',)  # every process of the studied macOS workflow maps it
HOST_KEYS = (*HOST_IDENTITY_KEYS, 'boot_session_uuid', 'image_path_status', 'cache_image_identities')
RECORD_KEYS_V2 = (*RECORD_KEYS, 'dual_resident_images')
HOST_KEYS_V2 = (*HOST_KEYS, 'dual_resident_files')
DUAL_KEYS = ('bytes', 'sha256')
SHA256 = re.compile(r'[0-9a-f]{64}')
GLOB = set('*?[]{}')
STUDIED_ADDRESS_SEMANTICS = ('mach_header', 'zero')  # jdk-17.0.1+12 os_bsd.cpp get_loaded_modules_info (__APPLE__)


@dataclass(frozen=True)
class SystemCacheEvidence:
    """Trusted caller snapshots. The frozen dataclass is shallow: the dicts inside stay mutable, so the caller must
    bind the approved record identity and prevent concurrent mutation while a check runs."""
    expected: dict
    pre_host: dict
    post_host: dict


def _int(value):
    return isinstance(value, int) and not isinstance(value, bool)


def _path(value):
    return (isinstance(value, str) and value.startswith('/') and not value.endswith('/')
            and '//' not in value and not GLOB.intersection(value) and '/../' not in value + '/')


def _same(a, b):
    """Type-strict equality (1 != True != 1.0) over the JSON-shaped records."""
    if type(a) is not type(b):
        return False
    if isinstance(a, dict):
        return a.keys() == b.keys() and all(_same(a[k], b[k]) for k in a)
    if isinstance(a, list):
        return len(a) == len(b) and all(_same(x, y) for x, y in zip(a, b))
    return a == b


def _exact_keys(value, keys):
    return isinstance(value, dict) and set(value) == set(keys)


def _record_errors(record):
    """Structural validation of the reviewed record; returns setting strings (no values)."""
    if not isinstance(record, dict):
        return ['record fields']
    version2 = record.get('schema') == SCHEMA_V2
    keys = RECORD_KEYS_V2 if version2 else RECORD_KEYS
    if set(record) != set(keys):
        errors = ['record fields: missing %s; unexpected %s' % (sorted(set(keys) - set(record)), sorted(set(record) - set(keys)))]
        if version2 and 'dual_resident_images' not in record:
            errors.insert(0, 'dual_resident_images: required for version 2')
        return errors
    errors = []
    if record['schema'] != SCHEMA and not version2:
        errors.append('schema')
    if not _exact_keys(record['os'], OS_KEYS) or not all(isinstance(record['os'][k], str) for k in OS_KEYS) \
            or not SHA256.fullmatch(record['os']['system_version_plist_sha256']):
        errors.append('os')
    if not _exact_keys(record['host_integrity'], INTEGRITY_KEYS) or not all(isinstance(v, bool) for v in record['host_integrity'].values()):
        errors.append('host_integrity')
    arch_valid = isinstance(record['arch'], str) and bool(record['arch'])  # DEF-0031: validated before filename use
    if not arch_valid:
        errors.append('arch')
    if not _path(record['cache_dir']):
        errors.append('cache_dir')
    cache = record['cache']
    if not _exact_keys(cache, ('main_uuid', 'subcaches', *CACHE_INT_KEYS)) or not all(_int(cache[k]) and cache[k] >= 0 for k in CACHE_INT_KEYS) \
            or not isinstance(cache['main_uuid'], str) or not cache['main_uuid'] or not isinstance(cache['subcaches'], list) \
            or not all(_exact_keys(s, ('suffix', 'uuid')) and isinstance(s['suffix'], str) and isinstance(s['uuid'], str)
                       and s['suffix'] and s['uuid'] and '/' not in s['suffix'] for s in cache['subcaches']):
        return errors + ['cache']
    if arch_valid:
        errors += _file_errors(record)
    alignment = record['slide_alignment']
    if not _int(alignment) or alignment <= 0 or alignment & (alignment - 1):
        errors.append('slide_alignment')
    source = record['capture_source']
    if not _exact_keys(source, CAPTURE_KEYS) or not all(isinstance(v, str) for v in source.values()):
        errors.append('capture_source')
    elif (source['base_address'], source['top_address']) != STUDIED_ADDRESS_SEMANTICS:
        errors.append('capture_source: address semantics not studied')
    image_errors = _image_errors(record)
    errors += image_errors
    if version2:
        # Follow-up 10: the dual subset can only be checked against valid canonical image identities.
        errors += _dual_errors(record) if not image_errors else ['dual_resident_images: not validated because images are invalid']
    return errors


def _dual_errors(record):
    images, dual = record['images'], record['dual_resident_images']
    canonical = {i['install_path'] for i in images}  # images already validated by _image_errors
    if not isinstance(dual, dict) or not all(isinstance(path, str) and path in canonical for path in dual) \
            or not all(_exact_keys(v, DUAL_KEYS) and _int(v['bytes']) and v['bytes'] > 0
                       and isinstance(v['sha256'], str) and SHA256.fullmatch(v['sha256']) for v in dual.values()):
        return ['dual_resident_images: explicit canonical image subset with positive bytes and lowercase sha256 required']
    return []


def _file_errors(record):
    files, arch, cache = record['cache_files'], record['arch'], record['cache']
    main = 'dyld_shared_cache_' + arch
    required = {main} | {main + s['suffix'] for s in cache['subcaches']}
    if not isinstance(files, dict) or not files or not required <= set(files):
        return ['cache_files: incomplete SHA256 manifest']
    for name, entry in files.items():
        if not isinstance(name, str) or '/' in name or not _exact_keys(entry, ('bytes', 'sha256')) \
                or not _int(entry['bytes']) or entry['bytes'] <= 0 or not isinstance(entry['sha256'], str) or not SHA256.fullmatch(entry['sha256']):
            return ['cache_files: missing or malformed digest']
    return []


def _image_errors(record):
    images, aliases, cache = record['images'], record['aliases'], record['cache']
    if not isinstance(images, list) or not all(_exact_keys(i, IMAGE_KEYS) for i in images):
        return ['images: explicit list of exact identities required']
    errors = []
    start, end = cache['shared_region_start'], cache['shared_region_start'] + cache['shared_region_size']
    for image in images:
        if not _path(image['install_path']) or not isinstance(image['image_uuid'], str) or not image['image_uuid'] \
                or not _int(image['unslid_load_address']) or not _int(image['text_size']) or image['text_size'] <= 0 \
                or not start <= image['unslid_load_address'] < image['unslid_load_address'] + image['text_size'] <= end:
            errors.append('images: invalid identity')
            break
    paths = [i['install_path'] for i in images]
    if all(isinstance(path, str) for path in paths) and len(set(paths)) != len(paths):  # DEF-0031: hash strings only
        errors.append('images: duplicate install_path')
    if len(images) >= cache['images_count']:
        errors.append('images: entire cache image set cannot be enrolled')
    if not isinstance(aliases, dict) or not all(_path(a) and a not in paths and t in paths for a, t in aliases.items()):
        errors.append('aliases: explicit reviewed alias to a reviewed image required')
    mandatory = record['mandatory_images']
    if not isinstance(mandatory, list) or not mandatory or len(set(map(str, mandatory))) != len(mandatory) \
            or not all(isinstance(m, str) and m in paths for m in mandatory) or not set(REQUIRED_MANDATORY_IMAGES) <= set(mandatory):
        errors.append('mandatory_images: non-empty canonical subset of images including %s required' % ', '.join(REQUIRED_MANDATORY_IMAGES))
    return errors


def check_host(evidence):
    """Compare the reviewed record with both independently read host records. Returns setting strings."""
    record = evidence.expected
    errors = _record_errors(record)
    if errors:
        return ['system cache record: ' + e for e in errors]
    paths = [i['install_path'] for i in record['images']] + list(record['aliases'])
    identities = {i['install_path']: {k: i[k] for k in IDENTITY_KEYS} for i in record['images']}
    version2 = record['schema'] == SCHEMA_V2
    dual = record['dual_resident_images'] if version2 else {}
    boots = set()
    for side, host in (('pre-capture', evidence.pre_host), ('post-capture', evidence.post_host)):
        if isinstance(host, dict) and version2 and 'dual_resident_files' not in host:
            errors.append('%s host: dual_resident_files missing for a version 2 record' % side)
        if isinstance(host, dict) and not version2 and 'dual_resident_files' in host:
            errors.append('%s host: dual_resident_files present for a version 1 record' % side)
        if not _exact_keys(host, HOST_KEYS_V2 if version2 else HOST_KEYS):
            if not isinstance(host, dict) or set(host) - {'dual_resident_files'} != set(HOST_KEYS):
                errors.append('%s host record incomplete' % side)
            continue
        for key in HOST_IDENTITY_KEYS:
            if not _same(host[key], record[key]):
                errors.append('%s host identity changed: %s' % (side, key))
        if not _same(host['cache_image_identities'], identities):
            errors.append('%s host identity changed: cache_image_identities' % side)
        if version2 and not _same(host['dual_resident_files'], dual):
            errors.append('%s host: dual_resident_files differ from the reviewed dual_resident_images' % side)
        status = host['image_path_status']
        if not isinstance(status, dict) or set(status) != set(paths):
            errors.append('%s host record incomplete: image_path_status' % side)
        else:
            for path in sorted(paths):
                if path in dual:
                    if status[path] != 'regular':
                        errors.append('%s host: dual-resident image_path_status not regular: %s' % (side, path))
                elif status[path] != 'absent':
                    errors.append('%s host: reviewed cache path not absent as a file: %s' % (side, path))
        boot = host['boot_session_uuid']
        if not isinstance(boot, str) or not boot:
            errors.append('%s host record incomplete: boot session' % side)
        boots.add(boot)
    if len(boots) > 1:
        errors.append('boot session differs between pre-capture and post-capture host')
    return errors


def lookup(evidence, name):
    """Canonical reviewed install path for an observed name, or None. Only exact paths and reviewed aliases."""
    record = evidence.expected
    paths = {i['install_path'] for i in record['images']}
    if name in paths:
        return name
    return record['aliases'].get(name)


def check_capture(evidence, observation, entries):
    """Per-JVM checks for observed cache entries [(canonical_path, observed_entry)]. Returns setting strings."""
    record = evidence.expected
    errors = []
    capture = observation.get('capture') or {}
    if not _same(capture.get('native_load_source'), record['capture_source']):
        errors.append('native capture source differs from the reviewed capture source')
    boot = observation.get('boot_session_uuid')
    if not isinstance(boot, str) or boot != evidence.pre_host.get('boot_session_uuid') or boot != evidence.post_host.get('boot_session_uuid'):
        errors.append('boot session of captured JVM differs from host records')
    images = {i['install_path']: i for i in record['images']}
    cache, alignment = record['cache'], record['slide_alignment']
    slides = set()
    for path, entry in entries:
        base, top = entry.get('base_address'), entry.get('top_address')
        if not _int(base):
            errors.append('system cache image without integer base_address: %s' % path)
            continue
        if not (_int(top) and top == 0):
            errors.append('system cache image top_address differs from reviewed semantics: %s' % path)
        slide = base - images[path]['unslid_load_address']
        if slide < 0 or slide % alignment or not (slide < cache['max_slide'] if cache['max_slide'] else slide == 0):
            errors.append('system cache image slide negative, unaligned or out of bounds: %s' % path)
        slides.add(slide)
    if len(slides) > 1:
        errors.append('system cache images use different slides in one JVM')
    observed = {path for path, _ in entries}  # canonical paths, after reviewed-alias lookup
    for path in record['mandatory_images']:
        if path not in observed:
            errors.append('incomplete capture: mandatory system cache image missing: %s' % path)
    return errors
