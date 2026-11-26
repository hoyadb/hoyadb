#!/usr/bin/env python3
"""TC-0038 SR-30 absence/fork evidence on synthetic JARs. No JAR code is loaded.

Run: python3 -I packages/react-native/tools/security/test_symbol_guard.py
     python3 -I -O packages/react-native/tools/security/test_symbol_guard.py
"""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import classfile  # noqa: E402
from fixtures import class_bytes, jar  # noqa: E402
import symbol_guard as S  # noqa: E402

KAPT = 'org/jetbrains/kotlin/kapt/base/incremental/JavaClassCacheManager.class'
LOG4J_AFFECTED = ['org/apache/log4j/net/SocketServer.class', 'org/apache/log4j/net/JMSAppender.class',
                  'org/apache/log4j/jdbc/JDBCAppender.class', 'org/apache/log4j/chainsaw/Main.class', 'org/apache/log4j/net/JMSSink.class']
RELOC = 'org/jetbrains/kotlin/'
LANG3 = 'org/jetbrains/kotlin/org/apache/commons/lang3/ClassUtils'
PROTO = 'org/jetbrains/kotlin/protobuf/CodedInputStream'


class Temp(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='hoyadb-sr30-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)


class ClassFileParser(Temp):
    def test_constant_pool_methodrefs_and_methods(self):
        data = class_bytes('a/B', methods=[('run', '()V')], invokes=[('c/D', 'go', '(I)V')], strings=['x.y'])
        info = classfile.parse(data)
        self.assertEqual(info.name, 'a/B')
        self.assertIn(('c/D', 'go', '(I)V'), info.methodrefs)
        self.assertIn(('run', '()V'), info.methods)
        self.assertIn('x.y', info.strings)

    def test_malformed_class_rejects_explicitly(self):
        with self.assertRaises(ValueError):
            classfile.parse(b'\xca\xfe\xba\xbe\x00')
        with self.assertRaises(ValueError):
            classfile.parse(b'not a class file at all')


class AbsenceClaims(Temp):
    def test_kapt_cache_absent_then_synthetic_presence_rejects(self):
        clean = jar(self.root / 'clean.jar', {'org/jetbrains/kotlin/kapt3/diagnostic/KaptError.class': class_bytes('org/jetbrains/kotlin/kapt3/diagnostic/KaptError')})
        self.assertEqual(S.present([clean], [KAPT]), {})
        bad = jar(self.root / 'bad.jar', {KAPT: class_bytes(KAPT[:-6])})
        self.assertEqual(S.present([clean, bad], [KAPT]), {KAPT: [str(bad)]})

    def test_relocated_symbol_is_still_detected(self):
        relocated = jar(self.root / 'reloc.jar', {RELOC + KAPT: class_bytes((RELOC + KAPT)[:-6])})
        self.assertEqual(S.present([relocated], [KAPT], relocation_prefixes=[RELOC]), {KAPT: [str(relocated)]})

    def test_log4j_affected_classes_absent_vs_present(self):
        only_safe = jar(self.root / 'l4.jar', {RELOC + 'org/apache/log4j/Level.class': class_bytes(RELOC + 'org/apache/log4j/Level')})
        self.assertEqual(S.present([only_safe], LOG4J_AFFECTED, relocation_prefixes=[RELOC]), {})
        control = jar(self.root / 'l4bad.jar', {RELOC + LOG4J_AFFECTED[1]: class_bytes((RELOC + LOG4J_AFFECTED[1])[:-6])})
        self.assertEqual(list(S.present([control], LOG4J_AFFECTED, relocation_prefixes=[RELOC])), [LOG4J_AFFECTED[1]])


class ForkAndCallPath(Temp):
    def test_protobuf_patch_presence_is_method_level(self):
        unpatched = jar(self.root / 'p1.jar', {PROTO + '.class': class_bytes(PROTO, methods=[('skipMessage', '()V')])})
        patched = jar(self.root / 'p2.jar', {PROTO + '.class': class_bytes(PROTO, methods=[('skipMessage', '()V'), ('checkRecursionLimit', '()V')])})
        self.assertFalse(S.declares(unpatched, PROTO, 'checkRecursionLimit'))
        self.assertTrue(S.declares(patched, PROTO, 'checkRecursionLimit'))
        with self.assertRaises(SystemExit):
            S.declares(unpatched, PROTO + 'Missing', 'checkRecursionLimit')

    def test_external_invocations_exclude_owner_and_detect_callers(self):
        owner = class_bytes(LANG3, methods=[('getClass', '(Ljava/lang/String;)Ljava/lang/Class;')],
                            invokes=[(LANG3, 'getClass', '(Ljava/lang/ClassLoader;Ljava/lang/String;Z)Ljava/lang/Class;')])
        safe = class_bytes('org/jetbrains/kotlin/gradle/A', invokes=[(LANG3, 'isAssignable', '(Ljava/lang/Class;Ljava/lang/Class;)Z')])
        clean = jar(self.root / 'kgp.jar', {LANG3 + '.class': owner, 'org/jetbrains/kotlin/gradle/A.class': safe})
        self.assertEqual(S.external_invocations([clean], LANG3, 'getClass'), [])
        caller = class_bytes('org/jetbrains/kotlin/gradle/B', invokes=[(LANG3, 'getClass', '(Ljava/lang/String;)Ljava/lang/Class;')])
        dirty = jar(self.root / 'kgp2.jar', {LANG3 + '.class': owner, 'org/jetbrains/kotlin/gradle/B.class': caller})
        self.assertEqual(S.external_invocations([dirty], LANG3, 'getClass'), [(str(dirty), 'org/jetbrains/kotlin/gradle/B')])

    def test_reflective_string_reference_counts_as_external_reference(self):
        reflective = class_bytes('x/R', strings=[LANG3.replace('/', '.')])
        archive = jar(self.root / 'r.jar', {'x/R.class': reflective})
        self.assertEqual(S.string_references([archive], LANG3), [(str(archive), 'x/R')])

    def test_runtime_flavor_users(self):
        lite = class_bytes('org/jetbrains/kotlin/metadata/P', class_refs=['org/jetbrains/kotlin/protobuf/GeneratedMessageLite'])
        full = class_bytes('org/jetbrains/kotlin/x/F', class_refs=['org/jetbrains/kotlin/protobuf/UnknownFieldSet'])
        archive = jar(self.root / 'c.jar', {'org/jetbrains/kotlin/metadata/P.class': lite})
        self.assertEqual(S.type_users([archive], 'org/jetbrains/kotlin/protobuf/UnknownFieldSet', exclude_prefix='org/jetbrains/kotlin/protobuf/'), [])
        archive2 = jar(self.root / 'c2.jar', {'org/jetbrains/kotlin/metadata/P.class': lite, 'org/jetbrains/kotlin/x/F.class': full})
        self.assertEqual(S.type_users([archive2], 'org/jetbrains/kotlin/protobuf/UnknownFieldSet', exclude_prefix='org/jetbrains/kotlin/protobuf/'),
                         [(str(archive2), 'org/jetbrains/kotlin/x/F')])


if __name__ == '__main__':
    unittest.main()
