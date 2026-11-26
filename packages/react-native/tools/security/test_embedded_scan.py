#!/usr/bin/env python3
"""TC-0038 SR-31 permanent embedded/shaded component gate on synthetic JARs. No network, no JAR code.

Run: python3 -I packages/react-native/tools/security/test_embedded_scan.py
     python3 -I -O packages/react-native/tools/security/test_embedded_scan.py
"""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import embedded_scan as E  # noqa: E402
from fixtures import class_bytes, jar, pom_properties  # noqa: E402

LANG3 = 'org.apache.commons:commons-lang3'
GHSA = 'GHSA-j288-q9x7-2f5v'


class Gate(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='hoyadb-sr31-')
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.kgp = jar(root / 'kotlin-gradle-plugin-2.1.20-gradle85.jar', {
            'META-INF/maven/org.jetbrains.kotlin/kotlin-gradle-plugin/pom.properties': pom_properties('org.jetbrains.kotlin', 'kotlin-gradle-plugin', '2.1.20'),
            'META-INF/maven/org.apache.commons/commons-lang3/pom.properties': pom_properties('org.apache.commons', 'commons-lang3', '3.16.0'),
            'org/jetbrains/kotlin/org/apache/commons/lang3/ClassUtils.class': class_bytes('org/jetbrains/kotlin/org/apache/commons/lang3/ClassUtils'),
            'org/jetbrains/kotlin/gradle/Plugin.class': class_bytes('org/jetbrains/kotlin/gradle/Plugin'),
        })
        self.compiler = jar(root / 'kotlin-compiler-embeddable-2.1.20.jar', {
            'org/jetbrains/kotlin/net/jpountz/lz4/LZ4Factory.class': class_bytes('org/jetbrains/kotlin/net/jpountz/lz4/LZ4Factory'),
        })
        self.namespaces = {'org/jetbrains/kotlin/org/apache/commons/lang3/': LANG3, 'org/jetbrains/kotlin/net/jpountz/': 'org.lz4:lz4-java'}
        self.declared = {'org.lz4:lz4-java': {'version': '1.7.1', 'source': 'kotlin v2.1.20 gradle/versions.properties'}}

    def inventory(self, jar_path, own, declared=None):
        return E.inventory(jar_path, own_coordinate=own, relocation_prefixes=['org/jetbrains/kotlin/'],
                           namespace_map=self.namespaces, source_declared=declared or {})

    def test_known_shaded_lang3_is_detected_byte_bound_and_separate(self):
        found = self.inventory(self.kgp, 'org.jetbrains.kotlin:kotlin-gradle-plugin:2.1.20')
        lang3 = [c for c in found if c['coordinate'] == LANG3 + ':3.16.0']
        self.assertEqual(len(lang3), 1)
        self.assertEqual(lang3[0]['confidence'], 'byte_bound_pom_properties')
        self.assertEqual(lang3[0]['container'], 'kotlin-gradle-plugin-2.1.20-gradle85.jar')
        self.assertNotIn('org.jetbrains.kotlin:kotlin-gradle-plugin:2.1.20', [c['coordinate'] for c in found])

    def test_source_declared_is_distinct_from_byte_bound(self):
        found = self.inventory(self.compiler, 'org.jetbrains.kotlin:kotlin-compiler-embeddable:2.1.20', self.declared)
        lz4 = [c for c in found if c['coordinate'].startswith('org.lz4:lz4-java')]
        self.assertEqual([c['confidence'] for c in lz4], ['source_declared'])

    def test_relocated_namespace_without_identity_is_unknown_not_clean(self):
        found = self.inventory(self.compiler, 'org.jetbrains.kotlin:kotlin-compiler-embeddable:2.1.20')
        lz4 = [c for c in found if c['ga'] == 'org.lz4:lz4-java']
        self.assertEqual(lz4[0]['version'], None)
        result = E.evaluate(found, advisories={}, triage={})
        self.assertIn('unknown version', ' '.join(r['reason'] for r in result['rejections']))

    def test_unmapped_relocated_third_party_namespace_rejects(self):
        odd = jar(Path(self.temp.name) / 'odd.jar', {
            'org/jetbrains/kotlin/com/example/shaded/Thing.class': class_bytes('org/jetbrains/kotlin/com/example/shaded/Thing')})
        found = self.inventory(odd, 'org.jetbrains.kotlin:odd:1')
        self.assertEqual([c['confidence'] for c in found], ['unmapped_namespace'])
        result = E.evaluate(found, advisories={}, triage={})
        self.assertIn('unmapped relocated namespace', ' '.join(r['reason'] for r in result['rejections']))

    # Follow-up 4 (review issue 3): flat two-segment packages and every relocated class are accounted.
    def test_flat_and_nested_relocated_packages_are_all_accounted(self):
        P = 'org/jetbrains/kotlin/'
        names = [P + 'javax/inject/Inject.class', P + 'javax/inject/Named.class', P + 'org/jdom/Attribute.class',
                 P + 'gnu/trove/Thing.class', P + 'com/example/shaded/deep/Thing.class', P + 'com/example/shaded/Other.class']
        odd = jar(Path(self.temp.name) / 'flat.jar', {n: class_bytes(n[:-6]) for n in names})
        found = self.inventory(odd, 'org.jetbrains.kotlin:flat:1')
        keys = {c['evidence']: c['classes'] for c in found}
        self.assertEqual(keys, {P + 'javax/inject/': 2, P + 'org/jdom/': 1, P + 'gnu/trove/': 1, P + 'com/example/shaded/': 2})
        self.assertEqual(sum(keys.values()), len(names))
        self.assertTrue(all(c['confidence'] == 'unmapped_namespace' and c['version'] is None for c in found))

    def test_longest_prefix_covers_unrelocated_and_alternate_relocation_roots(self):
        names = ['io/vavr/control/Try.class', 'org/jetbrains/org/objectweb/asm/ClassReader.class',
                 'org/jetbrains/kotlin/load/java/X.class', 'org/jetbrains/kotlin/com/google/common/base/Y.class',
                 'kotlin/collections/Z.class']
        archive = jar(Path(self.temp.name) / 'multi.jar', {n: class_bytes(n[:-6]) for n in names})
        found = E.inventory(archive, own_coordinate='x:multi:1', relocation_prefixes=['', 'org/jetbrains/', 'org/jetbrains/kotlin/'],
                            namespace_map={}, source_declared={})
        self.assertEqual({c['evidence']: c['classes'] for c in found},
                         {'io/vavr/control/': 1, 'org/jetbrains/org/objectweb/asm/': 1, 'org/jetbrains/kotlin/com/google/common/': 1})

    def test_cli_unreadable_or_missing_container_is_rejected_not_open(self):
        bad = Path(self.temp.name) / 'bad.jar'
        bad.write_bytes(b'not a zip archive')
        for path in [bad, Path(self.temp.name) / 'missing.jar']:
            out = Path(self.temp.name) / ('report-' + path.name + '.json')
            plan = Path(self.temp.name) / ('plan-' + path.name + '.json')
            plan.write_text(json.dumps({'jars': [{'path': str(path), 'own_coordinate': 'x:y:1'}], 'advisories': {}, 'triage': {}}))
            self.assertEqual(E.main(['--plan', str(plan), '--out', str(out)]), 2)
            report = json.loads(out.read_text())
            self.assertEqual(report['status'], 'rejected')
            self.assertIn('unreadable container', ' '.join(r['reason'] for r in report['rejections']))

    def test_triaged_known_advisory_is_reported_open_not_clean(self):
        found = self.inventory(self.kgp, 'org.jetbrains.kotlin:kotlin-gradle-plugin:2.1.20')
        advisories = {c['coordinate']: [] for c in found}
        advisories[LANG3 + ':3.16.0'] = [GHSA]
        triage = {LANG3 + ':3.16.0': {GHSA: {'disposition': 'open', 'evidence': 'lang3-getclass-entrypoints.txt'}}}
        result = E.evaluate(found, advisories, triage)
        self.assertEqual(result['rejections'], [])
        self.assertEqual(result['status'], 'open')
        self.assertEqual([(f['coordinate'], f['advisory']) for f in result['findings']], [(LANG3 + ':3.16.0', GHSA)])

    def test_untriaged_advisory_component_or_failed_scan_rejects(self):
        found = self.inventory(self.kgp, 'org.jetbrains.kotlin:kotlin-gradle-plugin:2.1.20')
        untriaged = E.evaluate(found, {LANG3 + ':3.16.0': [GHSA]}, {})
        self.assertIn('untriaged advisory', ' '.join(r['reason'] for r in untriaged['rejections']))
        missing_scan = E.evaluate(found, {}, {})
        self.assertIn('advisory scan unavailable', ' '.join(r['reason'] for r in missing_scan['rejections']))
        failed = E.evaluate(found, {LANG3 + ':3.16.0': None}, {})
        self.assertIn('advisory scan unavailable', ' '.join(r['reason'] for r in failed['rejections']))

    def test_not_applicable_requires_evidence_reference(self):
        found = self.inventory(self.kgp, 'org.jetbrains.kotlin:kotlin-gradle-plugin:2.1.20')
        result = E.evaluate(found, {LANG3 + ':3.16.0': [GHSA]}, {LANG3 + ':3.16.0': {GHSA: {'disposition': 'not_applicable'}}})
        self.assertIn('evidence', ' '.join(r['reason'] for r in result['rejections']))

    def test_cli_writes_json_and_exit_codes(self):
        out = Path(self.temp.name) / 'report.json'
        plan = Path(self.temp.name) / 'plan.json'
        plan.write_text(json.dumps({'jars': [{'path': str(self.kgp), 'own_coordinate': 'org.jetbrains.kotlin:kotlin-gradle-plugin:2.1.20'}],
                                    'relocation_prefixes': ['org/jetbrains/kotlin/'], 'namespace_map': self.namespaces,
                                    'source_declared': {}, 'advisories': {}, 'triage': {}}))
        code = E.main(['--plan', str(plan), '--out', str(out)])
        self.assertEqual(code, 2)
        self.assertEqual(json.loads(out.read_text())['status'], 'rejected')


if __name__ == '__main__':
    unittest.main()
