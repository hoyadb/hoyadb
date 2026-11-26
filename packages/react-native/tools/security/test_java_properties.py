#!/usr/bin/env python3
"""SPEC-0034 / TC-0038: synthetic public Reader-format behavior tests.

Run: python3 -I packages/react-native/tools/security/test_java_properties.py
     python3 -I -O packages/react-native/tools/security/test_java_properties.py
"""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from java_properties import MalformedProperties, load_reader  # noqa: E402


class ReaderProperties(unittest.TestCase):
    def test_empty_blank_and_comment_lines_are_ignored(self):
        self.assertEqual(load_reader(' \t\f\n  #ignored=one\\\n\t!ignored=two\n'), {})

    def test_comments_do_not_continue_to_the_next_natural_line(self):
        self.assertEqual(load_reader('# ignored\\\nkey=value\n! ignored\\\nnext=ok'),
                         {'key': 'value', 'next': 'ok'})

    def test_natural_lines_use_lf_cr_and_crlf(self):
        self.assertEqual(load_reader('one=1\rtwo=2\r\nthree=3\nfour=4'),
                         {'one': '1', 'two': '2', 'three': '3', 'four': '4'})

    def test_only_space_tab_and_form_feed_are_leading_whitespace(self):
        self.assertEqual(load_reader(' \t\fkey=value\n\u00a0other=x\n\vthird=y'),
                         {'key': 'value', '\u00a0other': 'x', '\vthird': 'y'})

    def test_non_line_unicode_separators_and_form_feed_remain_in_values(self):
        self.assertEqual(load_reader('key=a\vb\fc\u0085d\u2028e\u2029f'),
                         {'key': 'a\vb\fc\u0085d\u2028e\u2029f'})

    def test_whitespace_equal_and_colon_separate_key_and_value(self):
        for separator in ['=', ':', ' ', '\t', '\f', ' \t: \f', '\f= \t']:
            with self.subTest(separator=repr(separator)):
                self.assertEqual(load_reader('key' + separator + 'value'), {'key': 'value'})

    def test_only_one_optional_separator_is_consumed(self):
        self.assertEqual(load_reader('key = :value\nother::x\nthird==y'),
                         {'key': ':value', 'other': ':x', 'third': '=y'})

    def test_empty_values_and_empty_keys_are_supported(self):
        self.assertEqual(load_reader('alone\nempty=\nblank: \t\n=value'),
                         {'alone': '', 'empty': '', 'blank': '', '': 'value'})

    def test_trailing_and_embedded_value_whitespace_is_preserved(self):
        self.assertEqual(load_reader('key = a b\t\f '), {'key': 'a b\t\f '})

    def test_escaped_separators_and_whitespace_belong_to_key(self):
        self.assertEqual(load_reader(r'a\ b\:c\=d' + '\\\t' + '=value'),
                         {'a b:c=d\t': 'value'})

    def test_escaped_comment_markers_and_inline_markers_are_data(self):
        self.assertEqual(load_reader('\\#key=one\n\\!key=two\nx=a#b!c'),
                         {'#key': 'one', '!key': 'two', 'x': 'a#b!c'})

    def test_continuation_joins_key_and_value_for_all_line_endings(self):
        for ending in ['\n', '\r', '\r\n']:
            with self.subTest(ending=repr(ending)):
                text = 'ko\\' + ending + ' \t\ftlin.incre\\' + ending + ' mental=tr\\' + ending + '\t ue'
                self.assertEqual(load_reader(text), {'kotlin.incremental': 'true'})

    def test_trailing_backslash_parity_controls_continuation(self):
        for count in range(1, 7):
            with self.subTest(backslashes=count):
                text = 'key=left' + '\\' * count + '\nnext=right'
                if count % 2:
                    expected = {'key': 'left' + '\\' * (count // 2) + 'next=right'}
                else:
                    expected = {'key': 'left' + '\\' * (count // 2), 'next': 'right'}
                self.assertEqual(load_reader(text), expected)

    def test_blank_continuation_line_ends_the_logical_line(self):
        self.assertEqual(load_reader('key=left\\\n \t\f\nnext=right'),
                         {'key': 'left', 'next': 'right'})

    def test_comment_marker_on_continuation_line_is_value_data(self):
        self.assertEqual(load_reader('key=left\\\n  #right'), {'key': 'left#right'})

    def test_escaped_controls_and_backslashes_are_decoded(self):
        self.assertEqual(load_reader(r'key=\t\n\r\f\\'), {'key': '\t\n\r\f\\'})

    def test_non_special_escapes_drop_backslash_without_octal_or_backspace(self):
        self.assertEqual(load_reader(r'''key=\b\z\123\'\"'''), {'key': '''bz123'"'''})

    def test_unicode_escapes_decode_keys_values_and_exactly_four_digits(self):
        self.assertEqual(load_reader(r'\u006bey=\u0074rue\u00e9\uABcD\u00410'),
                         {'key': 'trueé\uabcdA0'})

    def test_escape_decoding_does_not_rescan_generated_characters(self):
        self.assertEqual(load_reader(r'key=\u005cu0041\u003a\u000a'),
                         {'key': '\\u0041:\n'})

    def test_unicode_escaped_separator_does_not_split_key(self):
        self.assertEqual(load_reader(r'a\u003db\u003ac=value'), {'a=b:c': 'value'})

    def test_malformed_unicode_escapes_fail_explicitly_in_keys_and_values(self):
        for escape in [r'\u', r'\u1', r'\u12', r'\u123', r'\u12xz', r'\uu0041', '\\u１２３４']:
            for text in [escape + '=value', 'key=' + escape]:
                with self.subTest(text=text):
                    with self.assertRaises(MalformedProperties):
                        load_reader(text)

    def test_comment_unicode_is_not_decoded(self):
        self.assertEqual(load_reader('# \\u00zz\n! \\u\nkey=ok'), {'key': 'ok'})

    def test_last_decoded_duplicate_key_wins(self):
        self.assertEqual(load_reader('key=first\n\\u006bey=second\nkey:last'), {'key': 'last'})

    def test_reader_preserves_literal_unicode_and_nul(self):
        self.assertEqual(load_reader('clé=飛行🚀\x00'), {'clé': '飛行🚀\x00'})

    def test_original_security_fixture_patterns_expose_escaped_settings(self):
        text = ('systemProp.\\u0069dea.use.native.compression=tr\\\n\t ue\n'
                'org.gradle.jvmargs=-Xmx1g \\\r\n   -D\\u0069dea.use.native.compression=true\n')
        self.assertEqual(load_reader(text), {
            'systemProp.idea.use.native.compression': 'true',
            'org.gradle.jvmargs': '-Xmx1g -Didea.use.native.compression=true',
        })


if __name__ == '__main__':
    unittest.main()
