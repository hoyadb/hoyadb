#!/usr/bin/env python3
"""TC-0038 SR-20/SR-21/SR-22 launch and observed-JVM guards. Pure: no Gradle, JVM, network or subprocess.

Run: python3 -I packages/react-native/tools/security/test_kotlin_guard.py
     python3 -I -O packages/react-native/tools/security/test_kotlin_guard.py
"""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import kotlin_guard as G  # noqa: E402

SECRET = 'unrelated-secret-value-7f3a'
OPTION_ENV = ['GRADLE_OPTS', 'JAVA_OPTS', 'JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS', 'JDK_JAVA_OPTIONS']


def secure(argv=(), env=None, properties=None):
    env = dict({'HOME_TOKEN': SECRET}, **(env or {}))
    props = {'consumer/gradle.properties': 'org.gradle.caching=false\n', 'included/gradle.properties': ''}
    props.update(properties or {})
    return G.check_launch([*G.secure_launch_args(), *argv], env, props)


class LaunchGuard(unittest.TestCase):
    def rejected(self, result, source, setting):
        self.assertFalse(result.ok)
        self.assertIn((source, setting), [(v.source, v.setting) for v in result.violations])
        self.assertNotIn(SECRET, repr(result))

    def test_default_secure_configuration_passes_and_pins_both_builds(self):
        args = G.secure_launch_args()
        self.assertIn('-Pkotlin.incremental=false', args)
        self.assertIn('-Pkotlin.compiler.execution.strategy=in-process', args)
        result = secure()
        self.assertTrue(result.ok, result.violations)

    def test_missing_secure_incremental_argument_rejects(self):
        result = G.check_launch([], {}, {})
        self.rejected(result, 'launch', 'kotlin.incremental')

    def test_project_property_files_enable_incremental(self):
        for name in ['consumer/gradle.properties', 'included/gradle.properties']:
            self.rejected(secure(properties={name: 'kotlin.incremental=true\n'}), name, 'kotlin.incremental')

    def test_explicit_launch_arguments_cannot_reenable(self):
        self.rejected(secure(['-Pkotlin.incremental=true']), 'launch', 'kotlin.incremental')
        self.rejected(secure(['--project-prop', 'kotlin.incremental=true']), 'launch', 'kotlin.incremental')
        self.rejected(secure(['-Dorg.gradle.project.kotlin.incremental=true']), 'launch', 'kotlin.incremental')
        self.rejected(secure(['-Didea.use.native.compression=true']), 'launch', 'idea.use.native.compression')
        self.rejected(secure(['-Pkotlin.compiler.execution.strategy=daemon']), 'launch', 'kotlin.compiler.execution.strategy')

    def test_prefixed_gradle_environment_property(self):
        self.rejected(secure(env={'ORG_GRADLE_PROJECT_kotlin.incremental': 'true'}), 'env:ORG_GRADLE_PROJECT_kotlin.incremental', 'kotlin.incremental')
        self.rejected(secure(env={'ORG_GRADLE_PROJECT_kotlin.compiler.execution.strategy': 'daemon'}),
                      'env:ORG_GRADLE_PROJECT_kotlin.compiler.execution.strategy', 'kotlin.compiler.execution.strategy')

    def test_each_inherited_jvm_option_source_has_its_own_control(self):
        for name in OPTION_ENV:
            self.rejected(secure(env={name: f'-Xmx1g -Didea.use.native.compression=TRUE -Dx={SECRET}'}), 'env:' + name, 'idea.use.native.compression')
            self.rejected(secure(env={name: '-Dorg.gradle.project.kotlin.incremental=true'}), 'env:' + name, 'kotlin.incremental')
        self.rejected(secure(properties={'consumer/gradle.properties': 'org.gradle.jvmargs=-Xmx2g -Didea.use.native.compression=true\n'}),
                      'consumer/gradle.properties:org.gradle.jvmargs', 'idea.use.native.compression')
        self.rejected(secure(properties={'included/gradle.properties': 'kotlin.daemon.jvmargs=-Didea.use.native.compression=true\n'}),
                      'included/gradle.properties:kotlin.daemon.jvmargs', 'idea.use.native.compression')
        self.rejected(secure(['-Dorg.gradle.jvmargs=-Didea.use.native.compression=true']), 'launch:org.gradle.jvmargs', 'idea.use.native.compression')
        self.rejected(secure(['-Pkotlin.daemon.jvmargs=-Didea.use.native.compression=true']), 'launch:kotlin.daemon.jvmargs', 'idea.use.native.compression')

    def test_system_property_forwarding_and_nested_daemon_options(self):
        self.rejected(secure(properties={'consumer/gradle.properties': 'systemProp.idea.use.native.compression=true\n'}),
                      'consumer/gradle.properties', 'idea.use.native.compression')
        self.rejected(secure(env={'GRADLE_OPTS': '-Dkotlin.daemon.jvm.options=Xmx1g,Didea.use.native.compression=true'}),
                      'env:GRADLE_OPTS:kotlin.daemon.jvm.options', 'idea.use.native.compression')
        self.rejected(secure(properties={'consumer/gradle.properties': 'org.gradle.jvmargs=-Dkotlin.daemon.jvm.options="-Didea.use.native.compression=true"\n'}),
                      'consumer/gradle.properties:org.gradle.jvmargs:kotlin.daemon.jvm.options', 'idea.use.native.compression')
        self.rejected(secure(env={'ORG_GRADLE_PROJECT_kotlin.daemon.jvmargs': '-Didea.use.native.compression=true'}),
                      'env:ORG_GRADLE_PROJECT_kotlin.daemon.jvmargs', 'idea.use.native.compression')

    # Follow-up 2 (root finding): public java.util.Properties.load(Reader) contract semantics.
    def test_properties_unicode_escaped_key_and_value(self):
        self.rejected(secure(properties={'consumer/gradle.properties': 'systemProp.\\u0069dea.use.native.compression=true\n'}),
                      'consumer/gradle.properties', 'idea.use.native.compression')
        self.rejected(secure(properties={'included/gradle.properties': 'kotlin.incremental=\\u0074rue\n'}),
                      'included/gradle.properties', 'kotlin.incremental')
        self.rejected(secure(properties={'consumer/gradle.properties': 'org.gradle.jvmargs=-D\\u0069dea.use.native.compression=true\n'}),
                      'consumer/gradle.properties:org.gradle.jvmargs', 'idea.use.native.compression')

    def test_properties_whitespace_and_colon_separators(self):
        for line in ['kotlin.incremental true', 'kotlin.incremental\ttrue', 'kotlin.incremental : true', 'kotlin.incremental:true',
                     '\fkotlin.incremental = true', '   kotlin.incremental=true']:
            self.rejected(secure(properties={'consumer/gradle.properties': line + '\n'}), 'consumer/gradle.properties', 'kotlin.incremental')

    def test_properties_continued_key_and_value(self):
        self.rejected(secure(properties={'consumer/gradle.properties': 'kotlin.incre\\\n    mental=true\n'}),
                      'consumer/gradle.properties', 'kotlin.incremental')
        self.rejected(secure(properties={'consumer/gradle.properties': 'org.gradle.jvmargs=-Xmx1g \\\r\n   -Didea.use.native.compression=true\n'}),
                      'consumer/gradle.properties:org.gradle.jvmargs', 'idea.use.native.compression')
        self.rejected(secure(properties={'consumer/gradle.properties': 'systemProp.idea.use.native.compression=tr\\\n\t ue'}),
                      'consumer/gradle.properties', 'idea.use.native.compression')

    def test_properties_escaped_backslash_is_not_a_continuation(self):
        # An even number of trailing backslashes is a literal backslash, not a continuation.
        result = secure(properties={'consumer/gradle.properties': 'unrelated=path\\\\\nkotlin.incremental=true\n'})
        self.rejected(result, 'consumer/gradle.properties', 'kotlin.incremental')

    def test_properties_comments_and_last_value_wins_like_java(self):
        self.assertTrue(secure(properties={'consumer/gradle.properties': '  ! kotlin.incremental=true\n# kotlin.incremental=true\n'}).ok)
        self.assertTrue(secure(properties={'consumer/gradle.properties': 'kotlin.incremental=true\nkotlin.incremental=false\n'}).ok)

    def test_properties_malformed_unicode_escape_fails_closed(self):
        self.rejected(secure(properties={'consumer/gradle.properties': 'kotlin.incremental=\\u00zz\n'}),
                      'consumer/gradle.properties', 'malformed properties')

    def test_properties_bytes_decode_like_java_iso_8859_1_and_fail_closed_on_utf8(self):
        self.rejected(secure(properties={'consumer/gradle.properties': b'kotlin.incremental=true\n'}), 'consumer/gradle.properties', 'kotlin.incremental')
        data = 'note=caf\u00e9\nsystemProp.idea.use.native.compression=true\n'.encode('utf-8')
        self.rejected(secure(properties={'consumer/gradle.properties': data}), 'consumer/gradle.properties', 'idea.use.native.compression')

    # Follow-up 5: the guard must use the independently written java_properties module (root decision A).
    def test_guard_uses_independent_properties_module(self):
        import java_properties
        from unittest import mock
        self.assertIs(G.MalformedProperties, java_properties.MalformedProperties)
        for translated in ('_logical_lines', '_convert', '_properties'):
            self.assertFalse(hasattr(G, translated), translated)
        with mock.patch.object(java_properties, 'load_reader', wraps=java_properties.load_reader) as spy:
            result = secure(properties={'consumer/gradle.properties': 'kotlin.incremental true\n'})
        self.assertTrue(spy.called)
        self.assertFalse(result.ok)

    def test_false_or_absent_native_compression_is_not_enabled(self):
        self.assertTrue(secure(env={'JAVA_OPTS': '-Didea.use.native.compression=false'}).ok)

    def test_reports_only_names(self):
        result = secure(env={'JAVA_TOOL_OPTIONS': f'-Didea.use.native.compression=true -Dtoken={SECRET}'})
        self.assertNotIn(SECRET, repr(result))
        self.assertNotIn('-Dtoken', repr(result))


class ObservedJvms(unittest.TestCase):
    def jvm(self, **override):
        base = {'role': 'kotlin-compiler', 'pid': 42, 'java_home_sha256': 'a' * 64, 'execution_strategy': 'in-process',
                'input_arguments': ['-Xmx2g'], 'system_properties': {'kotlin.incremental': 'false'},
                'environment': {'HOME_TOKEN': SECRET},  # follow-up 7 (SR-22c): effective environment is mandatory
                'capture': {'class_load': True, 'native_load': True},
                'classes_loaded': ['org/jetbrains/kotlin/cli/jvm/K2JVMCompiler'], 'native_libraries_loaded': []}
        base.update(override)
        return base

    def test_complete_safe_observation_closes(self):
        result = G.check_observed_jvms([self.jvm()], ['kotlin-compiler'])
        self.assertTrue(result.ok, result.violations)

    def test_unobserved_required_jvm_keeps_gate_open(self):
        result = G.check_observed_jvms([], ['kotlin-compiler'])
        self.assertFalse(result.ok)
        self.assertIn('unobserved', ' '.join(v.setting for v in result.violations))

    def test_effective_native_compression_rejects(self):
        result = G.check_observed_jvms([self.jvm(input_arguments=['-Didea.use.native.compression=true'])], ['kotlin-compiler'])
        self.assertFalse(result.ok)
        result = G.check_observed_jvms([self.jvm(system_properties={'idea.use.native.compression': 'True'})], ['kotlin-compiler'])
        self.assertFalse(result.ok)

    def test_class_load_without_native_capture_is_incomplete(self):
        result = G.check_observed_jvms([self.jvm(capture={'class_load': True, 'native_load': False})], ['kotlin-compiler'])
        self.assertFalse(result.ok)
        self.assertIn('incomplete capture', ' '.join(v.setting for v in result.violations))

    def test_lz4_entry_point_observed_keeps_gate_open(self):
        loaded = ['org/jetbrains/kotlin/incremental/storage/LazyStorage', 'org/jetbrains/kotlin/com/intellij/util/CompressionUtil']
        result = G.check_observed_jvms([self.jvm(classes_loaded=loaded)], ['kotlin-compiler'])
        self.assertFalse(result.ok)
        self.assertIn('vulnerable LZ4 entry point', ' '.join(v.setting for v in result.violations))

    def test_native_library_must_match_reviewed_inventory(self):
        inventory = {'jansi-mac-x86_64': 'b' * 64}
        ok = G.check_observed_jvms([self.jvm(native_libraries_loaded=[{'name': 'jansi-mac-x86_64', 'sha256': 'b' * 64}])],
                                   ['kotlin-compiler'], native_inventory=inventory)
        self.assertTrue(ok.ok, ok.violations)
        self.assertEqual(ok.native_loads, [('kotlin-compiler', 'jansi-mac-x86_64')])
        bad = G.check_observed_jvms([self.jvm(native_libraries_loaded=[{'name': 'jansi-mac-x86_64', 'sha256': 'c' * 64}])],
                                    ['kotlin-compiler'], native_inventory=inventory)
        self.assertFalse(bad.ok)

    def test_native_baseline_contract_is_explicit(self):
        # Follow-up 2: capture includes JDK/platform libraries; they must match an explicit trusted baseline.
        observed = [{'name': 'libjava.dylib', 'sha256': 'd' * 64}, {'name': 'jansi-mac-x86_64', 'sha256': 'b' * 64}]
        payloads = {'jansi-mac-x86_64': 'b' * 64}
        missing = G.check_observed_jvms([self.jvm(native_libraries_loaded=observed)], ['kotlin-compiler'], native_inventory=payloads)
        self.assertFalse(missing.ok)
        result = G.check_observed_jvms([self.jvm(native_libraries_loaded=observed)], ['kotlin-compiler'],
                                       native_inventory=payloads, native_baseline={'libjava.dylib': 'd' * 64})
        self.assertTrue(result.ok, result.violations)
        self.assertEqual(result.native_loads, [('kotlin-compiler', 'jansi-mac-x86_64')])
        self.assertEqual(result.baseline_loads, [('kotlin-compiler', 'libjava.dylib')])
        tampered = G.check_observed_jvms([self.jvm(native_libraries_loaded=[{'name': 'libjava.dylib', 'sha256': 'e' * 64}])],
                                         ['kotlin-compiler'], native_inventory=payloads, native_baseline={'libjava.dylib': 'd' * 64})
        self.assertFalse(tampered.ok)

    # Follow-up 4 (review issue 2): no vacuous certification.
    def test_empty_required_roles_cannot_certify(self):
        self.assertFalse(G.check_observed_jvms([], []).ok)
        self.assertFalse(G.check_observed_jvms([self.jvm()], []).ok)

    def test_missing_or_empty_effective_properties_is_incomplete(self):
        for override in ({'system_properties': {}}, {'system_properties': None}):
            result = G.check_observed_jvms([self.jvm(**override)], ['kotlin-compiler'])
            self.assertFalse(result.ok, override)
            self.assertIn('incomplete capture', ' '.join(v.setting for v in result.violations))
        observation = self.jvm()
        del observation['system_properties']
        self.assertFalse(G.check_observed_jvms([observation], ['kotlin-compiler']).ok)
        observation = self.jvm()
        del observation['input_arguments']
        self.assertFalse(G.check_observed_jvms([observation], ['kotlin-compiler']).ok)

    def test_missing_effective_environment_is_incomplete(self):
        observation = self.jvm()
        del observation['environment']
        result = G.check_observed_jvms([observation], ['kotlin-compiler'])
        self.assertFalse(result.ok)
        self.assertIn('incomplete capture: effective environment missing', [v.setting for v in result.violations])

    # DEF-0031 UP05 (no cache evidence): malformed entries reject by field and role, never raise or echo.
    def test_malformed_native_entries_reject_without_raising_or_echo(self):
        sentinel = 'malformed-value-sentinel-91c4'
        cases = [(None, 'entry is not an object'), ([sentinel], 'entry is not an object'),
                 ({'name': [sentinel], 'sha256': 'b' * 64}, 'name missing or not a non-empty string'),
                 ({'name': {sentinel: 1}, 'sha256': 'b' * 64}, 'name missing or not a non-empty string'),
                 ({'sha256': 'b' * 64}, 'name missing or not a non-empty string'), ({'name': ''}, 'name missing or not a non-empty string')]
        for entry, identity in cases:
            with self.subTest(entry=repr(entry)):
                try:
                    result = G.check_observed_jvms([self.jvm(native_libraries_loaded=[entry])], ['kotlin-compiler'],
                                                   native_inventory={'jansi-mac-x86_64': 'b' * 64})
                except Exception as error:
                    self.fail('raised %s instead of rejecting' % type(error).__name__)
                self.assertFalse(result.ok)
                self.assertIn(('kotlin-compiler', 'incomplete capture: native library ' + identity),
                              [(v.source, v.setting) for v in result.violations])
                self.assertNotIn(sentinel, repr(result))
                self.assertEqual((result.native_loads, result.baseline_loads, result.discovered_images), ([], [], []))

    def test_strategy_mismatch_rejects(self):
        result = G.check_observed_jvms([self.jvm(execution_strategy='daemon')], ['kotlin-compiler'])
        self.assertFalse(result.ok)


if __name__ == '__main__':
    unittest.main()
