#!/usr/bin/env python3
"""SPEC-0034 R1 static guard for the compile-alignment init script. Gradle is never run here.

Run: python3 -I packages/react-native/tools/security/test_init_script_guard.py
     python3 -I -O packages/react-native/tools/security/test_init_script_guard.py
"""
from pathlib import Path
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import init_script_guard as I  # noqa: E402

SCRIPT = HERE / 'r1-compile-alignment.init.gradle'
APPROVED = {'com.google.guava:guava:33.3.1-jre', 'com.google.code.gson:gson:2.11.0',
            'com.google.errorprone:error_prone_annotations:2.30.0'}


class InitScript(unittest.TestCase):
    def check(self, text):
        with tempfile.TemporaryDirectory(prefix='hoyadb-r1-') as temp:
            path = Path(temp) / 'r1.init.gradle'
            path.write_text(text)
            return I.check(path, approved=APPROVED)

    def test_reviewed_script_passes_with_exact_scope(self):
        result = I.check(SCRIPT, approved=APPROVED)
        self.assertTrue(result.ok, result.problems)
        self.assertEqual(result.build, 'gradle-plugin-root')
        self.assertEqual(result.projects, [':react-native-gradle-plugin', ':settings-plugin', ':shared'])
        self.assertEqual(result.configuration, 'compileClasspath')
        self.assertEqual(result.constraints, {'com.google.guava:guava': '33.3.1-jre', 'com.google.code.gson:gson': '2.11.0',
                                              'com.google.errorprone:error_prone_annotations': '2.30.0'})

    def test_forbidden_resolution_overrides_reject(self):
        base = SCRIPT.read_text()
        for bad in ['force(', 'force true', 'exclude group', 'dependencySubstitution', 'eachDependency', 'resolutionStrategy',
                    'strictly ', 'componentSelection']:
            result = self.check(base + f'\n// injected\n{bad}\n')
            self.assertFalse(result.ok, bad)

    def test_unapproved_version_rejects(self):
        result = self.check(SCRIPT.read_text().replace("'33.3.1-jre'", "'32.0.0-jre'"))
        self.assertFalse(result.ok)
        self.assertIn('unapproved constraint', ' '.join(result.problems))

    def test_unnamed_configuration_or_extra_project_rejects(self):
        self.assertFalse(self.check(SCRIPT.read_text().replace("'compileClasspath'", "'runtimeClasspath'")).ok)
        self.assertFalse(self.check(SCRIPT.read_text().replace("':shared']", "':shared', ':shared-testutil']")).ok)
        self.assertFalse(self.check(SCRIPT.read_text().replace("it.name == HOYADB_R1_CONFIGURATION", "true")).ok)

    # Follow-up 4 (review issue 4).
    def test_exact_constraint_set_required(self):
        text = SCRIPT.read_text()
        for line in ["    'com.google.guava:guava'                        : '33.3.1-jre',\n",
                     "    'com.google.errorprone:error_prone_annotations' : '2.30.0',\n"]:
            self.assertIn(line, text)
            result = self.check(text.replace(line, ''))
            self.assertFalse(result.ok, line)
            self.assertIn('constraint set differs from reviewed', ' '.join(result.problems))

    def test_line_comment_inside_string_cannot_hide_override(self):
        injected = ("def ref='https://example.invalid'; allprojects { configurations.all { "
                    "resolutionStrategy.force 'com.google.guava:guava:31.0.1-jre' } }\n")
        self.assertFalse(self.check(SCRIPT.read_text() + injected).ok)
        self.assertFalse(self.check(SCRIPT.read_text() + 'def u = "http://x" ; configurations.all { exclude group: "a" }\n').ok)

    def test_block_comment_and_string_handling(self):
        self.assertTrue(self.check(SCRIPT.read_text() + '/* force exclude */\n').ok)
        self.assertFalse(self.check(SCRIPT.read_text() + "def s = 'unterminated\n").ok)

    def test_script_sha256_pin_is_enforced(self):
        self.assertEqual(I.R1_SCRIPT_SHA256, __import__('hashlib').sha256(SCRIPT.read_bytes()).hexdigest())
        self.assertTrue(I.verify_pinned(SCRIPT, approved=APPROVED).ok)
        with tempfile.TemporaryDirectory(prefix='hoyadb-r1-') as temp:
            copy = Path(temp) / 'r1.init.gradle'
            copy.write_bytes(SCRIPT.read_bytes() + b'\n')
            result = I.verify_pinned(copy, approved=APPROVED)
            self.assertFalse(result.ok)
            self.assertIn('script sha256 differs from pin', ' '.join(result.problems))

    def test_wrong_build_name_rejects(self):
        self.assertFalse(self.check(SCRIPT.read_text().replace("'gradle-plugin-root'", "'other'")).ok)


if __name__ == '__main__':
    unittest.main()
