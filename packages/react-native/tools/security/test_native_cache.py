#!/usr/bin/env python3
"""TC-0038 SR-22a/b macOS system shared-cache identities. Pure and synthetic: no JVM, dyld, network or subprocess.

Run: python3 -I packages/react-native/tools/security/test_native_cache.py
     python3 -I -O packages/react-native/tools/security/test_native_cache.py
"""
import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import kotlin_guard as G  # noqa: E402
import native_cache as N  # noqa: E402

ARCH = 'x86_64h'
MAIN = 'dyld_shared_cache_' + ARCH
SECRET = 'unrelated-secret-value-7f3a'
BOOT = '00EBDCD7-544E-4B3D-BF87-B30D07098B2D'
SLIDE = 0x5000
LIBSYSTEM = '/usr/lib/libSystem.B.dylib'
LIBZ = '/usr/lib/libz.1.dylib'
SECURITY = '/System/Library/Frameworks/Security.framework/Versions/A/Security'
IMAGES = [
    {'install_path': LIBSYSTEM, 'image_uuid': '59F4B806-CE61-3332-9C81-B0017DD51338', 'unslid_load_address': 0x7ff812e9d000, 'text_size': 6956},
    {'install_path': LIBZ, 'image_uuid': '1937863A-A3D1-3A4D-AF24-5492992693BF', 'unslid_load_address': 0x7ff812db2000, 'text_size': 79340},
    {'install_path': SECURITY, 'image_uuid': 'B2530481-AEDF-36E2-847D-77D5545C91FE', 'unslid_load_address': 0x7ff806220000, 'text_size': 4190288},
]
ALIAS = '/System/Library/Frameworks/Security.framework/Security'
LIBSYSTEM_ALIAS = '/usr/lib/libSystem.dylib'


def expected():
    files = {MAIN: {'bytes': 888602624, 'sha256': '1' * 64}, MAIN + '.01': {'bytes': 805208064, 'sha256': '2' * 64},
             MAIN + '.map': {'bytes': 1156006, 'sha256': '3' * 64}}
    return {
        'schema': N.SCHEMA,
        'os': {'product_version': '26.7.1', 'product_build_version': '25G241', 'build_id': '66D2C708-B87D-11F1-B88D-BB1EF7271656',
               'product_version_extra': '', 'system_version_plist_sha256': '7' * 64},
        'host_integrity': {'sip_enabled': True, 'authenticated_root': True, 'system_volume_sealed': True},
        'arch': ARCH,
        'cache_dir': '/System/Volumes/Preboot/Cryptexes/OS/System/Library/dyld',
        'cache': {'main_uuid': '31374756-89B4-34E8-A34F-56C5D57216A1', 'subcaches': [{'suffix': '.01', 'uuid': '98E79F0E-FCC8-31D5-885B-12E9A5F8A269'}],
                  'cache_type': 0, 'platform': 1, 'os_version_field': 0x1a0700, 'shared_region_start': 0x7ff800000000,
                  'shared_region_size': 0x700000000, 'max_slide': 0x1941c000, 'images_count': 3625},
        'cache_files': files,
        'slide_alignment': 4096,
        'capture_source': {'jdk': 'Temurin-17.0.1+12', 'event': 'jdk.NativeLibrary', 'base_address': 'mach_header', 'top_address': 'zero'},
        'images': copy.deepcopy(IMAGES),
        'aliases': {},
        'mandatory_images': [LIBSYSTEM],
    }


def host(record=None):
    """An independently read host record that agrees with the reviewed record."""
    record = record or expected()
    paths = [i['install_path'] for i in record['images']] + list(record['aliases'])
    return {key: copy.deepcopy(record[key]) for key in N.HOST_IDENTITY_KEYS} | {
        'boot_session_uuid': BOOT,
        'image_path_status': {p: 'absent' for p in paths},
        'cache_image_identities': {i['install_path']: {k: i[k] for k in ('image_uuid', 'unslid_load_address', 'text_size')}
                                   for i in record['images']},
    }


def evidence(record=None, pre=None, post=None):
    record = record or expected()
    return N.SystemCacheEvidence(expected=record, pre_host=pre or host(record), post_host=post or host(record))


def cached(path, slide=SLIDE, **override):
    image = next(i for i in IMAGES if i['install_path'] == path)
    entry = {'name': path, 'base_address': image['unslid_load_address'] + slide, 'top_address': 0}
    entry.update(override)
    return entry


def jvm(libs, **override):
    base = {'role': 'kotlin-compiler', 'execution_strategy': 'in-process', 'input_arguments': ['-Xmx2g'],
            'system_properties': {'kotlin.incremental': 'false'}, 'environment': {'HOME_TOKEN': SECRET},
            'capture': {'class_load': True, 'native_load': True, 'native_load_source': expected()['capture_source']},
            'boot_session_uuid': BOOT, 'classes_loaded': [], 'native_libraries_loaded': libs}
    base.update(override)
    return base


PAYLOADS = {'libjansi.jnilib': 'b' * 64}
BASELINE = {'/jdk/lib/libjava.dylib': 'd' * 64}
FILE_LIBS = [{'name': '/jdk/lib/libjava.dylib', 'sha256': 'd' * 64}, {'name': 'libjansi.jnilib', 'sha256': 'b' * 64}]


def check(observations, system_cache=None):
    return G.check_observed_jvms(observations, ['kotlin-compiler'], native_inventory=PAYLOADS, native_baseline=BASELINE,
                                 system_cache=system_cache)


class SystemCacheIdentity(unittest.TestCase):
    def rejected(self, result, text):
        self.assertFalse(result.ok)
        joined = ' | '.join(v.setting for v in result.violations)
        self.assertIn(text, joined)
        self.assertNotIn(SECRET, repr(result))

    def test_sr22a_reviewed_cache_images_close_and_report_three_bins(self):
        libs = FILE_LIBS + [cached(LIBSYSTEM), cached(LIBZ), cached(SECURITY)]
        result = check([jvm(libs)], evidence())
        self.assertTrue(result.ok, result.violations)
        self.assertEqual(result.native_loads, [('kotlin-compiler', 'libjansi.jnilib')])
        self.assertEqual(result.baseline_loads, [('kotlin-compiler', '/jdk/lib/libjava.dylib')])
        self.assertEqual(result.system_cache_loads, [('kotlin-compiler', LIBSYSTEM), ('kotlin-compiler', LIBZ), ('kotlin-compiler', SECURITY)])
        self.assertEqual(result.discovered_images, [])

    # Follow-up 8 (review): a valid cache record must not certify a JVM without the mandatory cache images.
    def test_sr22a_zero_cache_images_is_incomplete(self):
        self.rejected(check([jvm(FILE_LIBS)], evidence()), 'mandatory system cache image missing: ' + LIBSYSTEM)

    def test_sr22a_every_jvm_must_report_mandatory_images(self):
        first = jvm(FILE_LIBS + [cached(LIBSYSTEM)])
        second = jvm(FILE_LIBS, role='kotlin-daemon')
        result = G.check_observed_jvms([first, second], ['kotlin-compiler', 'kotlin-daemon'], native_inventory=PAYLOADS,
                                       native_baseline=BASELINE, system_cache=evidence())
        self.rejected(result, 'mandatory system cache image missing: ' + LIBSYSTEM)
        self.assertEqual({v.source for v in result.violations}, {'kotlin-daemon'})

    def test_sr22a_non_mandatory_cache_image_alone_is_incomplete(self):
        self.rejected(check([jvm([cached(LIBZ)])], evidence()), 'mandatory system cache image missing: ' + LIBSYSTEM)

    def test_sr22a_mandatory_subset_must_be_explicit_canonical_and_include_libsystem(self):
        changes = {
            'absent': lambda r: r.pop('mandatory_images'),
            'empty': lambda r: r.update(mandatory_images=[]),
            'all': lambda r: r.update(mandatory_images='all'),
            'non-list': lambda r: r.update(mandatory_images={LIBSYSTEM: True}),
            'unregistered': lambda r: r.update(mandatory_images=[LIBSYSTEM, '/usr/lib/libobjc.A.dylib']),
            'alias entry': lambda r: r.update(aliases={LIBSYSTEM_ALIAS: LIBSYSTEM}, mandatory_images=[LIBSYSTEM_ALIAS]),
            'duplicate': lambda r: r.update(mandatory_images=[LIBSYSTEM, LIBSYSTEM]),
            'without libSystem': lambda r: r.update(mandatory_images=[LIBZ]),
            'wildcard': lambda r: r.update(mandatory_images=['/usr/lib/*']),
        }
        for label, change in changes.items():
            record = expected()
            change(record)
            with self.subTest(label=label):
                result = check([jvm([cached(LIBSYSTEM), cached(LIBZ)])], evidence(record, pre=host(), post=host()))
                self.assertFalse(result.ok)
                self.assertTrue(any('mandatory_images' in v.setting for v in result.violations), result.violations)
        record = expected()
        record['mandatory_images'] = [LIBSYSTEM, LIBZ]
        self.assertTrue(check([jvm([cached(LIBSYSTEM), cached(LIBZ)])], evidence(record)).ok)
        self.rejected(check([jvm([cached(LIBSYSTEM)])], evidence(record)), 'mandatory system cache image missing: ' + LIBZ)

    def test_sr22a_any_one_of_several_jvms_missing_a_mandatory_image_rejects(self):
        roles = ['kotlin-compiler', 'kotlin-daemon', 'kapt']
        for missing in roles:
            observations = [jvm([cached(LIBZ)] + ([] if role == missing else [cached(LIBSYSTEM)]), role=role) for role in roles]
            with self.subTest(missing=missing):
                result = G.check_observed_jvms(observations, roles, system_cache=evidence())
                self.rejected(result, 'mandatory system cache image missing: ' + LIBSYSTEM)
                self.assertEqual({v.source for v in result.violations}, {missing})

    def test_sr22a_unreviewed_alias_cannot_satisfy_mandatory_image(self):
        result = check([jvm([cached(LIBSYSTEM, name=LIBSYSTEM_ALIAS)])], evidence())
        self.rejected(result, 'mandatory system cache image missing: ' + LIBSYSTEM)
        self.assertEqual(result.discovered_images, [('kotlin-compiler', LIBSYSTEM_ALIAS)])

    def test_sr22a_reviewed_alias_satisfies_canonical_mandatory_image(self):
        record = expected()
        record['aliases'] = {LIBSYSTEM_ALIAS: LIBSYSTEM}
        result = check([jvm([cached(LIBSYSTEM, name=LIBSYSTEM_ALIAS)])], evidence(record, pre=host(record), post=host(record)))
        self.assertTrue(result.ok, result.violations)
        self.assertEqual(result.system_cache_loads, [('kotlin-compiler', LIBSYSTEM)])

    def test_sr22a_repeated_chunk_event_for_same_image_counts_once(self):
        result = check([jvm([cached(LIBSYSTEM), cached(LIBZ), cached(LIBZ)])], evidence())
        self.assertTrue(result.ok, result.violations)
        self.assertEqual(result.system_cache_loads, [('kotlin-compiler', LIBSYSTEM), ('kotlin-compiler', LIBZ)])

    def test_sr22a_missing_digest_is_never_a_general_exemption(self):
        self.rejected(check([jvm([cached(LIBZ)])]), 'missing digest')
        self.rejected(check([jvm([{'name': '/jdk/lib/libjava.dylib', 'sha256': None}])], evidence()), 'missing digest')
        self.rejected(check([jvm([{'name': '/opt/unknown.dylib', 'base_address': 0x1000, 'top_address': 0}])], evidence()), 'missing digest')

    def test_sr22a_regular_file_cannot_enter_through_cache_class(self):
        self.rejected(check([jvm([cached(LIBZ, sha256='e' * 64)])], evidence()), 'file-backed')
        for side in ('pre', 'post'):
            changed = host()
            changed['image_path_status'][LIBZ] = 'regular_file'
            self.rejected(check([jvm([cached(LIBSYSTEM)])], evidence(**{side: changed})), 'not absent as a file')

    def test_sr22a_each_entry_classified_exactly_once(self):
        both = G.check_observed_jvms([jvm([{'name': 'libjansi.jnilib', 'sha256': 'b' * 64}])], ['kotlin-compiler'],
                                     native_inventory=PAYLOADS, native_baseline={'libjansi.jnilib': 'b' * 64}, system_cache=evidence())
        self.rejected(both, 'ambiguous native classification')

    def test_sr22b_any_host_identity_change_rejects_in_pre_or_post(self):
        changes = {
            'os.product_build_version': lambda h: h['os'].update(product_build_version='25G242'),
            'os.build_id': lambda h: h['os'].update(build_id='0' * 8),
            'os.product_version_extra': lambda h: h['os'].update(product_version_extra='(a)'),
            'os.system_version_plist_sha256': lambda h: h['os'].update(system_version_plist_sha256='8' * 64),
            'host_integrity': lambda h: h['host_integrity'].update(system_volume_sealed=False),
            'arch': lambda h: h.update(arch='arm64e'),
            'cache_dir': lambda h: h.update(cache_dir='/tmp/dyld'),
            'cache.main_uuid': lambda h: h['cache'].update(main_uuid='0' * 32),
            'cache.subcache uuid': lambda h: h['cache']['subcaches'][0].update(uuid='0' * 32),
            'cache.extra subcache': lambda h: h['cache']['subcaches'].append({'suffix': '.02', 'uuid': '1' * 32}),
            'cache.max_slide': lambda h: h['cache'].update(max_slide=0x2000),
            'cache.images_count': lambda h: h['cache'].update(images_count=3626),
            'cache_files sha256': lambda h: h['cache_files'][MAIN + '.01'].update(sha256='9' * 64),
            'cache_files extra file': lambda h: h['cache_files'].update({MAIN + '.02': {'bytes': 1, 'sha256': '4' * 64}}),
            'cache_files missing file': lambda h: h['cache_files'].pop(MAIN + '.map'),
            'image identity uuid': lambda h: h['cache_image_identities'][LIBZ].update(image_uuid='0' * 32),
            'image identity address': lambda h: h['cache_image_identities'][LIBZ].update(unslid_load_address=0x7ff812db3000),
        }
        for label, change in changes.items():
            for side in ('pre', 'post'):
                changed = host()
                change(changed)
                with self.subTest(label=label, side=side):
                    self.rejected(check([jvm([cached(LIBZ)])], evidence(**{side: changed})), 'host identity')

    def test_sr22b_host_comparison_is_type_strict(self):
        for change in (lambda h: h['host_integrity'].update(sip_enabled=1), lambda h: h['cache'].update(cache_type=False),
                       lambda h: h['cache_image_identities'][LIBZ].update(text_size=79340.0)):
            changed = host()
            change(changed)
            with self.subTest():
                self.rejected(check([jvm([cached(LIBZ)])], evidence(pre=changed)), 'host identity')

    def test_sr22b_incomplete_host_or_record_rejects(self):
        for side in ('pre', 'post'):
            for key in ('os', 'cache_files', 'boot_session_uuid', 'image_path_status', 'cache_image_identities', 'host_integrity'):
                changed = host()
                del changed[key]
                with self.subTest(side=side, key=key):
                    self.assertFalse(check([jvm([cached(LIBZ)])], evidence(**{side: changed})).ok)

    def test_sr22b_uuid_only_identity_cannot_replace_complete_hashes(self):
        for change in (lambda r: r.pop('cache_files'), lambda r: r.update(cache_files={}),
                       lambda r: r['cache_files'][MAIN].update(sha256=None), lambda r: r['cache_files'][MAIN].update(sha256='ABC'),
                       lambda r: r['cache_files'].pop(MAIN + '.01'), lambda r: r['cache_files'][MAIN].pop('bytes')):
            record = expected()
            change(record)
            with self.subTest(record=record.get('cache_files')):
                self.rejected(check([jvm([cached(LIBZ)])], evidence(record, pre=host(), post=host())), 'cache_files')

    def test_sr22b_boot_session_binds_pre_capture_and_post(self):
        changed = host()
        changed['boot_session_uuid'] = '11111111-2222-3333-4444-555555555555'
        for side in ('pre', 'post'):
            self.rejected(check([jvm([cached(LIBZ)])], evidence(**{side: changed})), 'boot session')
        self.rejected(check([jvm([cached(LIBZ)], boot_session_uuid='11111111-2222-3333-4444-555555555555')], evidence()), 'boot session')
        observation = jvm([cached(LIBZ)])
        del observation['boot_session_uuid']
        self.rejected(check([observation], evidence()), 'boot session')

    def test_sr22b_slide_must_be_common_bounded_and_aligned(self):
        cases = {
            'mixed': [cached(LIBZ), cached(LIBSYSTEM, slide=SLIDE + 0x1000)],
            # Follow-up 8: libSystem shares each bad slide, so only the slide rule can reject.
            'negative': [cached(LIBSYSTEM, slide=-0x1000), cached(LIBZ, slide=-0x1000)],
            'unaligned': [cached(LIBSYSTEM, slide=0x5800), cached(LIBZ, slide=0x5800)],
            'at max_slide': [cached(LIBSYSTEM, slide=0x1941c000), cached(LIBZ, slide=0x1941c000)],
            'beyond max_slide': [cached(LIBSYSTEM, slide=0x20000000), cached(LIBZ, slide=0x20000000)],
        }
        for label, libs in cases.items():
            with self.subTest(label=label):
                self.rejected(check([jvm(libs)], evidence()), 'slide')
        for slide in (0, 0x1941b000):
            result = check([jvm([cached(LIBSYSTEM, slide=slide), cached(LIBZ, slide=slide)])], evidence())
            self.assertTrue(result.ok, result.violations)

    def test_sr22b_slide_is_per_jvm(self):
        first = jvm([cached(LIBSYSTEM), cached(LIBZ)])
        second = jvm([cached(LIBSYSTEM, slide=0x9000), cached(LIBZ, slide=0x9000)], role='kotlin-daemon')
        result = G.check_observed_jvms([first, second], ['kotlin-compiler', 'kotlin-daemon'], system_cache=evidence())
        self.assertTrue(result.ok, result.violations)

    def test_sr22b_alignment_is_the_reviewed_architecture_value(self):
        record = expected()
        del record['slide_alignment']
        self.rejected(check([jvm([cached(LIBZ)])], evidence(record, pre=host(), post=host())), 'slide_alignment')
        for bad in (0, 3000, True, '4096'):
            record = expected()
            record['slide_alignment'] = bad
            with self.subTest(bad=bad):
                self.rejected(check([jvm([cached(LIBZ)])], evidence(record)), 'slide_alignment')
        record = expected()
        record['slide_alignment'] = 0x4000
        self.rejected(check([jvm([cached(LIBZ)])], evidence(record)), 'slide')

    def test_sr22b_address_semantics_are_pinned(self):
        self.rejected(check([jvm([cached(LIBZ, top_address=0x7ff812dc5000 + SLIDE)])], evidence()), 'top_address')
        missing = cached(LIBZ)
        del missing['base_address']
        self.rejected(check([jvm([missing])], evidence()), 'base_address')
        self.rejected(check([jvm([cached(LIBZ, base_address='0x7ff812db7000')])], evidence()), 'base_address')
        for top in (False, 0.0, None):
            with self.subTest(top=top):
                self.rejected(check([jvm([cached(LIBZ, top_address=top)])], evidence()), 'top_address')
        other = dict(expected()['capture_source'], top_address='module_end')
        capture = {'class_load': True, 'native_load': True, 'native_load_source': other}
        self.rejected(check([jvm([cached(LIBZ)], capture=capture)], evidence()), 'capture source')
        capture = {'class_load': True, 'native_load': True}
        self.rejected(check([jvm([cached(LIBZ)], capture=capture)], evidence()), 'capture source')

    def test_sr22b_prefix_wildcard_and_whole_cache_enrollment_reject(self):
        for path in ('/usr/lib/*', '/usr/lib/', '/usr/lib/libz.?.dylib', '/usr/lib/[a-z]*', 'usr/lib/libz.1.dylib', ''):
            record = expected()
            record['images'].append({'install_path': path, 'image_uuid': '0' * 32, 'unslid_load_address': 0x7ff812000000, 'text_size': 1})
            with self.subTest(path=path):
                self.rejected(check([jvm([cached(LIBZ)])], evidence(record)), 'images')
        record = expected()
        record['images'] = 'all'
        self.rejected(check([jvm([cached(LIBZ)])], evidence(record, pre=host(), post=host())), 'images')
        record = expected()
        record['cache']['images_count'] = len(record['images'])
        self.rejected(check([jvm([cached(LIBZ)])], evidence(record, pre=host(record), post=host(record))), 'entire cache image set')
        record = expected()
        record['images'].append(dict(record['images'][0]))
        self.rejected(check([jvm([cached(LIBZ)])], evidence(record)), 'duplicate')

    def test_sr22b_aliases_need_explicit_review(self):
        self.rejected(check([jvm([cached(SECURITY, name=ALIAS)])], evidence()), 'missing digest')
        record = expected()
        record['aliases'] = {ALIAS: SECURITY}
        result = check([jvm([cached(LIBSYSTEM), cached(SECURITY, name=ALIAS)])], evidence(record, pre=host(record), post=host(record)))
        self.assertTrue(result.ok, result.violations)
        self.assertEqual(result.system_cache_loads, [('kotlin-compiler', LIBSYSTEM), ('kotlin-compiler', SECURITY)])
        for bad in ({ALIAS: '/usr/lib/unreviewed.dylib'}, {'/System/Library/Frameworks/*': SECURITY}, {LIBZ: SECURITY}):
            record = expected()
            record['aliases'] = bad
            with self.subTest(bad=bad):
                self.rejected(check([jvm([cached(LIBZ)])], evidence(record, pre=host(record), post=host(record))), 'aliases')

    def test_sr22b_discovery_cannot_enroll_itself(self):
        record = expected()
        frozen = copy.deepcopy(record)
        unknown = {'name': '/usr/lib/libobjc.A.dylib', 'base_address': 0x7ff802a22000 + SLIDE, 'top_address': 0}
        result = check([jvm([cached(LIBZ), unknown])], evidence(record))
        self.rejected(result, 'missing digest')
        self.assertEqual(result.discovered_images, [('kotlin-compiler', '/usr/lib/libobjc.A.dylib')])
        self.assertEqual(record, frozen)
        self.assertFalse(check([jvm([cached(LIBZ), unknown])], evidence(record)).ok)

    def test_sr22b_record_schema_and_types_are_exact(self):
        for change in (lambda r: r.update(schema='other/1'), lambda r: r.pop('capture_source'), lambda r: r['os'].pop('build_id'),
                       lambda r: r['cache'].pop('max_slide'), lambda r: r['cache'].update(max_slide='0x1941c000'),
                       lambda r: r['images'][0].update(unslid_load_address=None), lambda r: r['images'][0].pop('text_size'),
                       lambda r: r['images'][0].update(extra=1), lambda r: r['cache'].update(main_uuid=''),
                       lambda r: r['cache']['subcaches'][0].update(uuid=''), lambda r: r['cache']['subcaches'][0].update(suffix='/.01')):
            record = expected()
            change(record)
            with self.subTest():
                self.assertFalse(check([jvm([cached(LIBZ)])], evidence(record, pre=host(), post=host())).ok)


SCHEMA_V2 = 'hoyadb.system-shared-cache/2'  # literal: the RED must come from the guard's result, not a missing constant
KERNEL = '/usr/lib/system/libsystem_kernel.dylib'
KERNEL_IMAGE = {'install_path': KERNEL, 'image_uuid': 'C7A3B3F4-1D35-3B5C-9B5F-6E2C51A7D0E1', 'unslid_load_address': 0x7ff812d00000,
                'text_size': 245760}
KERNEL_DISK = {'bytes': 1346496, 'sha256': '34a1db9713a098517185fe74b0be8889c39112955cd5462b144df9198dc5899d'}
KERNEL_ALIAS = '/usr/lib/system/libsystem_kernel_alias.dylib'


def expected_v2(dual=None):
    """SR-22d: a synthetic version-2 record with one explicit dual-resident canonical image."""
    record = expected()
    record['schema'] = SCHEMA_V2
    record['images'].append(dict(KERNEL_IMAGE))
    record['dual_resident_images'] = copy.deepcopy({KERNEL: KERNEL_DISK} if dual is None else dual)
    return record


def host_v2(record):
    """Independently read version-2 host: dual paths are regular files with re-read size and hash."""
    observed = host(record)
    for path in record['dual_resident_images']:
        observed['image_path_status'][path] = 'regular'
    observed['dual_resident_files'] = copy.deepcopy(record['dual_resident_images'])
    return observed


def evidence_v2(record=None, pre=None, post=None):
    record = record or expected_v2()
    return N.SystemCacheEvidence(expected=record, pre_host=pre or host_v2(record), post_host=post or host_v2(record))


def cached_image(image, slide=SLIDE, **override):
    entry = {'name': image['install_path'], 'base_address': image['unslid_load_address'] + slide, 'top_address': 0}
    entry.update(override)
    return entry


class DualResident(unittest.TestCase):
    """SR-22d version-2 successor. Version-1 behaviour is characterized, not re-labelled."""

    def violations(self, result):
        return [v.setting for v in result.violations]

    def rejected_with(self, result, identity):
        self.assertFalse(result.ok)
        self.assertTrue(any(identity in s for s in self.violations(result)), (identity, self.violations(result)))
        self.assertNotIn(SECRET, repr(result))

    def test_sr22d_v2_dual_resident_image_classifies_once_as_system_cache(self):
        result = check([jvm(FILE_LIBS + [cached(LIBSYSTEM), cached_image(KERNEL_IMAGE)])], evidence_v2())
        self.assertTrue(result.ok, result.violations)
        self.assertEqual(result.system_cache_loads, [('kotlin-compiler', LIBSYSTEM), ('kotlin-compiler', KERNEL)])
        self.assertEqual(result.baseline_loads, [('kotlin-compiler', '/jdk/lib/libjava.dylib')])

    def test_sr22d_v2_reviewed_alias_resolves_to_canonical_dual_image(self):
        record = expected_v2()
        record['aliases'] = {KERNEL_ALIAS: KERNEL}
        result = check([jvm([cached(LIBSYSTEM), cached_image(KERNEL_IMAGE, name=KERNEL_ALIAS)])], evidence_v2(record))
        self.assertTrue(result.ok, result.violations)
        self.assertEqual(result.system_cache_loads, [('kotlin-compiler', LIBSYSTEM), ('kotlin-compiler', KERNEL)])

    def test_sr22d_v2_dual_image_can_be_mandatory(self):
        record = expected_v2()
        record['mandatory_images'] = [LIBSYSTEM, KERNEL]
        self.assertTrue(check([jvm([cached(LIBSYSTEM), cached_image(KERNEL_IMAGE)])], evidence_v2(record)).ok)
        self.rejected_with(check([jvm([cached(LIBSYSTEM)])], evidence_v2(record)), 'mandatory system cache image missing: ' + KERNEL)

    def test_sr22d_v2_empty_dual_set_grants_no_exception(self):
        record = expected_v2(dual={})
        self.assertTrue(check([jvm([cached(LIBSYSTEM), cached_image(KERNEL_IMAGE)])], evidence_v2(record)).ok)
        for side in ('pre', 'post'):
            changed = host_v2(record)
            changed['image_path_status'][KERNEL] = 'regular'
            with self.subTest(side=side):
                self.rejected_with(check([jvm([cached(LIBSYSTEM)])], evidence_v2(record, **{side: changed})),
                                   'reviewed cache path not absent as a file: ' + KERNEL)

    def test_sr22d_v1_characterization_rejects_real_file_and_v2_fields(self):
        record = expected()
        record['images'].append(dict(KERNEL_IMAGE))
        observed = host(record)
        observed['image_path_status'][KERNEL] = 'regular'
        self.rejected_with(check([jvm([cached(LIBSYSTEM)])], evidence(record, pre=observed, post=host(record))),
                           'pre-capture host: reviewed cache path not absent as a file: ' + KERNEL)
        record['dual_resident_images'] = {KERNEL: dict(KERNEL_DISK)}
        self.rejected_with(check([jvm([cached(LIBSYSTEM)])], evidence(record, pre=host(), post=host())), 'dual_resident_images')

    def test_sr22d_v1_host_carrying_dual_resident_files_rejects(self):
        for side in ('pre', 'post'):
            changed = host()
            changed['dual_resident_files'] = {}
            with self.subTest(side=side):
                self.rejected_with(check([jvm([cached(LIBSYSTEM)])], evidence(**{side: changed})),
                                   '%s-capture host: dual_resident_files' % side)

    def test_sr22d_v2_host_missing_dual_resident_files_rejects(self):
        for side in ('pre', 'post'):
            changed = host_v2(expected_v2())
            del changed['dual_resident_files']
            with self.subTest(side=side):
                self.rejected_with(check([jvm([cached(LIBSYSTEM)])], evidence_v2(**{side: changed})),
                                   '%s-capture host: dual_resident_files' % side)

    def test_sr22d_pre_and_post_each_match_record_even_when_wrong_values_agree(self):
        wrong = host_v2(expected_v2())
        wrong['dual_resident_files'][KERNEL]['sha256'] = '0' * 64
        result = check([jvm([cached(LIBSYSTEM)])], evidence_v2(pre=wrong, post=copy.deepcopy(wrong)))
        self.rejected_with(result, 'pre-capture host: dual_resident_files')
        self.rejected_with(result, 'post-capture host: dual_resident_files')

    def test_sr22d_malformed_or_unregistered_subset_entries_reject(self):
        cases = {
            'absent': lambda r: r.pop('dual_resident_images'),
            'non-dict': lambda r: r.update(dual_resident_images=[KERNEL]),
            'unregistered': lambda r: r.update(dual_resident_images={'/usr/lib/system/libsystem_c.dylib': dict(KERNEL_DISK)}),
            'alias': lambda r: r.update(aliases={KERNEL_ALIAS: KERNEL}, dual_resident_images={KERNEL_ALIAS: dict(KERNEL_DISK)}),
            'wildcard': lambda r: r.update(dual_resident_images={'/usr/lib/system/*': dict(KERNEL_DISK)}),
            'prefix': lambda r: r.update(dual_resident_images={'/usr/lib/system/': dict(KERNEL_DISK)}),
            'bytes bool': lambda r: r['dual_resident_images'][KERNEL].update(bytes=True),
            'bytes zero': lambda r: r['dual_resident_images'][KERNEL].update(bytes=0),
            'bytes negative': lambda r: r['dual_resident_images'][KERNEL].update(bytes=-1),
            'bytes float': lambda r: r['dual_resident_images'][KERNEL].update(bytes=1346496.0),
            'bytes string': lambda r: r['dual_resident_images'][KERNEL].update(bytes='1346496'),
            'sha uppercase': lambda r: r['dual_resident_images'][KERNEL].update(sha256=KERNEL_DISK['sha256'].upper()),
            'sha short': lambda r: r['dual_resident_images'][KERNEL].update(sha256='34a1db97'),
            'sha missing': lambda r: r['dual_resident_images'][KERNEL].pop('sha256'),
            'extra value field': lambda r: r['dual_resident_images'][KERNEL].update(uuid='C7A3B3F4'),
        }
        for label, change in cases.items():
            record = expected_v2()
            change(record)
            with self.subTest(label=label):
                result = check([jvm([cached(LIBSYSTEM), cached_image(KERNEL_IMAGE)])],
                               evidence_v2(record, pre=host_v2(expected_v2()), post=host_v2(expected_v2())))
                self.rejected_with(result, 'system cache record: dual_resident_images')

    # Follow-up 10 (root finding): an invalid image identity must be a record rejection, never an exception.
    def test_sr22d_missing_image_identity_rejects_instead_of_raising(self):
        for key in ('install_path', 'image_uuid', 'unslid_load_address', 'text_size'):
            for index in (0, 3):
                record = expected_v2()
                record['images'][index].pop(key)
                with self.subTest(key=key, index=index):
                    try:
                        direct = N.check_host(N.SystemCacheEvidence(record, {}, {}))
                        result = check([jvm([cached(LIBSYSTEM)])], evidence_v2(record, pre=host_v2(expected_v2()), post=host_v2(expected_v2())))
                    except Exception as error:  # the defect: an exception instead of a rejection
                        self.fail('raised %s instead of rejecting' % type(error).__name__)
                    self.assertTrue(any(e.startswith('system cache record: images') for e in direct), direct)
                    self.rejected_with(result, 'system cache record: images')

    def test_sr22d_changed_missing_extra_or_incomplete_host_file_evidence_rejects(self):
        changes = {
            'sha changed': lambda h: h['dual_resident_files'][KERNEL].update(sha256='1' * 64),
            'bytes changed': lambda h: h['dual_resident_files'][KERNEL].update(bytes=1346497),
            'bytes type': lambda h: h['dual_resident_files'][KERNEL].update(bytes=1346496.0),
            'missing entry': lambda h: h['dual_resident_files'].pop(KERNEL),
            'extra entry': lambda h: h['dual_resident_files'].update({LIBZ: dict(KERNEL_DISK)}),
            'incomplete entry': lambda h: h['dual_resident_files'][KERNEL].pop('bytes'),
            'non-dict': lambda h: h.update(dual_resident_files=None),
        }
        for label, change in changes.items():
            for side in ('pre', 'post'):
                changed = host_v2(expected_v2())
                change(changed)
                with self.subTest(label=label, side=side):
                    self.rejected_with(check([jvm([cached(LIBSYSTEM), cached_image(KERNEL_IMAGE)])], evidence_v2(**{side: changed})),
                                       '%s-capture host: dual_resident_files' % side)

    def test_sr22d_wrong_path_kinds_reject_per_side(self):
        for kind in ('symlink', 'absent', 'other', 'regular_file'):
            for side in ('pre', 'post'):
                changed = host_v2(expected_v2())
                changed['image_path_status'][KERNEL] = kind
                with self.subTest(kind=kind, side=side):
                    self.rejected_with(check([jvm([cached(LIBSYSTEM)])], evidence_v2(**{side: changed})),
                                       '%s-capture host: dual-resident image_path_status' % side)
        changed = host_v2(expected_v2())
        changed['image_path_status'][LIBZ] = 'regular'
        self.rejected_with(check([jvm([cached(LIBSYSTEM)])], evidence_v2(pre=changed)), 'reviewed cache path not absent as a file: ' + LIBZ)
        record = expected_v2()
        record['aliases'] = {KERNEL_ALIAS: KERNEL}
        changed = host_v2(record)
        changed['image_path_status'][KERNEL_ALIAS] = 'regular'
        self.rejected_with(check([jvm([cached(LIBSYSTEM)])], evidence_v2(record, pre=changed)),
                           'reviewed cache path not absent as a file: ' + KERNEL_ALIAS)

    def test_sr22d_digest_bearing_cache_claim_rejects_even_with_disk_hash(self):
        claim = cached_image(KERNEL_IMAGE, sha256=KERNEL_DISK['sha256'])
        self.rejected_with(check([jvm([cached(LIBSYSTEM), claim])], evidence_v2()), 'file-backed library claims a reviewed system cache path: ' + KERNEL)

    def test_sr22d_capture_rules_still_apply_to_dual_images(self):
        self.rejected_with(check([jvm([cached(LIBSYSTEM), cached_image(KERNEL_IMAGE, slide=SLIDE + 0x1000)])], evidence_v2()), 'different slides')
        self.rejected_with(check([jvm([cached(LIBSYSTEM), cached_image(KERNEL_IMAGE, slide=0x5800)])], evidence_v2()), 'slide')
        self.rejected_with(check([jvm([cached_image(KERNEL_IMAGE)])], evidence_v2()), 'mandatory system cache image missing: ' + LIBSYSTEM)
        self.rejected_with(check([jvm([cached(LIBSYSTEM), cached_image(KERNEL_IMAGE, top_address=1)])], evidence_v2()), 'top_address')

    def test_sr22d_unknown_or_candidate_schema_and_uuid_only_still_reject(self):
        for schema in ('hoyadb.system-shared-cache/3', 'hoyadb.system-shared-cache-candidate/1'):
            record = expected_v2()
            record['schema'] = schema
            with self.subTest(schema=schema):
                self.rejected_with(check([jvm([cached(LIBSYSTEM)])], evidence_v2(record)), 'system cache record')
        record = expected_v2()
        record.pop('cache_files')
        self.rejected_with(check([jvm([cached(LIBSYSTEM)])], evidence_v2(record, pre=host_v2(expected_v2()), post=host_v2(expected_v2()))),
                           'cache_files')


SENTINEL = 'malformed-value-sentinel-91c4'
NON_STRING_JSON = [None, True, 0, 1.5, [SENTINEL], {SENTINEL: SENTINEL}]


class InputTypes(unittest.TestCase):
    """DEF-0031 UP01-UP05: malformed field types return structured rejections; exceptions are test failures."""

    def call(self, function, *args, **kwargs):
        try:
            return function(*args, **kwargs)
        except Exception as error:  # the defect: an escaping exception instead of a structured rejection
            self.fail('raised %s instead of rejecting' % type(error).__name__)

    def records(self):
        return (('v1', expected, host, evidence), ('v2', expected_v2, host_v2, evidence_v2))

    def test_up01_unhashable_install_path_does_not_escape_validation(self):
        for label, make, make_host, make_evidence in self.records():
            for value in ([], {}):
                record = make()
                record['images'][0]['install_path'] = value
                with self.subTest(schema=label, value=type(value).__name__):
                    direct = self.call(N.check_host, N.SystemCacheEvidence(record, {}, {}))
                    self.assertTrue(any(e.startswith('system cache record: images') for e in direct), direct)
                    result = self.call(check, [jvm([cached(LIBSYSTEM)])], make_evidence(record, pre=make_host(make()), post=make_host(make())))
                    self.assertFalse(result.ok)
                    self.assertTrue(any(v.setting.startswith('system cache record: images') for v in result.violations), result.violations)

    def test_up02_every_non_string_install_path_rejects_without_coercion(self):
        for label, make, make_host, make_evidence in self.records():
            for value in NON_STRING_JSON:
                record = make()
                record['images'][0]['install_path'] = value
                with self.subTest(schema=label, value=repr(value)):
                    direct = self.call(N.check_host, N.SystemCacheEvidence(record, {}, {}))
                    self.assertIn('system cache record: images: invalid identity', direct)
                    self.assertNotIn(SENTINEL, repr(direct))

    def test_up03_valid_duplicate_and_unsafe_path_controls_keep_their_meaning(self):
        self.assertTrue(self.call(check, [jvm([cached(LIBSYSTEM), cached(LIBZ)])], evidence()).ok)
        self.assertTrue(self.call(check, [jvm([cached(LIBSYSTEM), cached_image(KERNEL_IMAGE)])], evidence_v2()).ok)
        for label, make, _, _ in self.records():
            record = make()
            record['images'].append(dict(record['images'][0]))
            with self.subTest(schema=label, case='duplicate'):
                self.assertIn('system cache record: images: duplicate install_path', self.call(N.check_host, N.SystemCacheEvidence(record, {}, {})))
            for path in ('', 'usr/lib/libz.1.dylib', '/usr/lib/*', '/usr/lib/../libz.1.dylib'):
                record = make()
                record['images'][1]['install_path'] = path
                with self.subTest(schema=label, path=path):
                    self.assertIn('system cache record: images: invalid identity', self.call(N.check_host, N.SystemCacheEvidence(record, {}, {})))

    def test_up04_invalid_architecture_never_reaches_filename_construction(self):
        for label, make, make_host, make_evidence in self.records():
            for value in [''] + NON_STRING_JSON:
                record = make()
                record['arch'] = value
                with self.subTest(schema=label, value=repr(value)):
                    direct = self.call(N.check_host, N.SystemCacheEvidence(record, {}, {}))
                    self.assertIn('system cache record: arch', direct)
                    self.assertFalse(any('cache_files' in e for e in direct), direct)
                    self.assertNotIn(SENTINEL, repr(direct))
                    result = self.call(check, [jvm([cached(LIBSYSTEM)])], make_evidence(record, pre=make_host(make()), post=make_host(make())))
                    self.assertFalse(result.ok)
                    self.assertIn('system cache record: arch', [v.setting for v in result.violations])

    def test_up05_invalid_library_entries_and_names_are_role_specific_and_never_echoed_or_binned(self):
        entries = [('not an object', bad) for bad in (None, SENTINEL, [SENTINEL], 7, True)]
        for name in ('missing', '') + tuple(NON_STRING_JSON):
            for digest in (None, 'b' * 64):
                for base in (False, True):
                    entry = {} if name == 'missing' else {'name': name}
                    if digest:
                        entry['sha256'] = digest
                    if base:
                        entry.update(base_address=0x7ff812db2000 + SLIDE, top_address=0)
                    entries.append(('name', entry))
        for with_cache in (False, True):
            valid = FILE_LIBS + ([cached(LIBSYSTEM)] if with_cache else [])
            for kind, bad in entries:
                with self.subTest(cache=with_cache, kind=kind, entry=repr(bad)):
                    result = self.call(check, [jvm(valid + [bad])], evidence() if with_cache else None)
                    self.assertFalse(result.ok)
                    identity = ('incomplete capture: native library entry is not an object' if kind == 'not an object'
                                else 'incomplete capture: native library name missing or not a non-empty string')
                    self.assertIn(('kotlin-compiler', identity), [(v.source, v.setting) for v in result.violations])
                    self.assertNotIn(SENTINEL, repr(result))
                    self.assertEqual(result.discovered_images, [])
                    self.assertEqual(result.native_loads, [('kotlin-compiler', 'libjansi.jnilib')])
                    self.assertEqual(result.baseline_loads, [('kotlin-compiler', '/jdk/lib/libjava.dylib')])
                    self.assertEqual(result.system_cache_loads, [('kotlin-compiler', LIBSYSTEM)] if with_cache else [])


class LoaderEnvironment(unittest.TestCase):
    def test_sr22c_dyld_variables_reject_before_launch_by_name_only(self):
        for name in ('DYLD_INSERT_LIBRARIES', 'DYLD_LIBRARY_PATH', 'DYLD_SHARED_REGION', 'DYLD_SHARED_CACHE_DIR', 'DYLD_FRAMEWORK_PATH',
                     'DYLD_PRINT_LIBRARIES'):
            result = G.check_launch(G.secure_launch_args(), {name: '/tmp/' + SECRET}, {})
            with self.subTest(name=name):
                self.assertFalse(result.ok)
                self.assertIn(('env:' + name, 'DYLD_ loader environment'), [(v.source, v.setting) for v in result.violations])
                self.assertNotIn(SECRET, repr(result))

    def test_sr22c_effective_environment_is_mandatory_and_checked(self):
        result = check([jvm([], environment={'DYLD_INSERT_LIBRARIES': '/tmp/' + SECRET})])
        self.assertFalse(result.ok)
        self.assertIn(('kotlin-compiler', 'DYLD_ loader environment: DYLD_INSERT_LIBRARIES'),
                      [(v.source, v.setting) for v in result.violations])
        self.assertNotIn(SECRET, repr(result))
        for override in ({'environment': None}, {'environment': ['DYLD_X=1']}):
            self.assertFalse(check([jvm([], **override)]).ok)
        observation = jvm([])
        del observation['environment']
        result = check([observation])
        self.assertFalse(result.ok)
        self.assertIn('incomplete capture: effective environment missing', [v.setting for v in result.violations])


if __name__ == '__main__':
    unittest.main()
