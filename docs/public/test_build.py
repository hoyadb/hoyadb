"""TC-0045: executable documentation tooling regression tests."""
import tempfile
import html
import re
import unittest
from pathlib import Path
import build
import verify


class DocumentationTests(unittest.TestCase):
    def test_verification_pins_public_package_and_browser_inputs(self):
        hashes = verify.source_hashes()
        for path in ['packages/client/package.json', 'packages/testing/package.json',
                     'tools/javascript/scripts/build.mjs', 'tools/javascript/scripts/application-browser.mjs',
                     'docs/public/language-examples.json', 'docs/public/example-completions.json',
                     'examples/typescript.ts', 'examples/react-native-bootstrap.mjs',
                     'examples/swift/Sources/TodoGuide/main.swift',
                     'examples/kotlin/TodoGuide.kt', 'examples/c/todo.c',
                     'crates/hoyadb-native/Cargo.toml', 'bindings/swift/Package.swift']:
            self.assertIn(path, hashes)

    def test_example_is_included_and_html_escaped(self):
        result = build.render_examples('{{example:demo}}', {'demo': ('demo.rs', '<tag>&"')})
        self.assertEqual(html.unescape(re.sub('<[^>]+>', '', result.split('<code', 1)[1].split('>', 1)[1].split('</code>')[0])), '<tag>&\"')
        self.assertNotIn('<tag>', result)
        self.assertIn('data-example="demo"', result)
        self.assertIn('verification.html', result)

    def test_named_regions_include_only_executed_source(self):
        source = '// docs:start open\nconst db = await openDatabase();\n// docs:end open\nassert(db);'
        result = build.render_examples('{{example:application:open}}', {'application': ('application.mjs', source)})
        self.assertIn('const db = await openDatabase();', html.unescape(re.sub('<[^>]+>', '', result)))
        self.assertNotIn('assert(db)', result)
        self.assertIn('verification.html#case-application', result)

    def test_invalid_or_missing_regions_fail_closed(self):
        for source in ['// docs:start other\nx();\n// docs:end other',
                       '// docs:start open\nx();',
                       '// docs:start open\n// docs:start nested\nx();\n// docs:end nested\n// docs:end open',
                       '// docs:start open\nx();\n// docs:end open\n// docs:start open\ny();\n// docs:end open']:
            with self.subTest(source=source), self.assertRaisesRegex(ValueError, 'region'):
                build.render_examples('{{example:application:open}}', {'application': ('application.mjs', source)})

    def test_missing_example_refuses_build(self):
        with self.assertRaisesRegex(ValueError, 'missing'):
            build.render_examples('{{example:missing}}', {})

    def test_missing_link_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'index.html').write_text('<a href="missing.html">bad</a>')
            self.assertTrue(any('missing.html' in e for e in build.check_site(root)))

    def test_rustdoc_redirect_preserves_and_validates_fragments(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/'index.html').write_text('<a href="alias.html#method.scan">scan</a>')
            (root/'alias.html').write_text('<meta http-equiv="refresh" content="0;URL=actual.html">')
            (root/'actual.html').write_text('<h2 id="method.scan">scan</h2>')
            self.assertEqual(build.check_site(root), [])
            (root/'actual.html').write_text('<h2 id="other">other</h2>')
            self.assertTrue(any('fragment' in e for e in build.check_site(root)))

    def test_redirect_cycle_and_remote_target_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/'index.html').write_text('<a href="alias.html">alias</a>')
            (root/'alias.html').write_text('<meta http-equiv="refresh" content="0;URL=alias.html">')
            self.assertTrue(any('redirect' in e for e in build.check_site(root)))
            (root/'alias.html').write_text('<meta http-equiv="refresh" content="0;URL=https://remote.invalid/">')
            self.assertTrue(any('remote' in e for e in build.check_site(root)))

    def test_missing_fragment_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'index.html').write_text('<a href="#missing">bad</a>')
            self.assertTrue(any('missing' in e for e in build.check_site(root)))

    def test_unlabelled_code_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'index.html').write_text('<pre><code>untested()</code></pre>')
            self.assertTrue(any('code' in e for e in build.check_site(root)))

    def test_missing_guide_symbol_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'declarations.txt').write_text('public existing()')
            errors = build.check_symbols({'guide': [{'file': 'declarations.txt', 'names': ['missing']}]}, root)
            self.assertTrue(any('guide' in e and 'missing' in e for e in errors))

    def test_valid_guide_symbol(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'declarations.txt').write_text('public existing()')
            self.assertEqual(build.check_symbols({'guide': [{'file': 'declarations.txt', 'names': ['existing']}]}, root), [])

    def test_remote_runtime_assets_are_rejected(self):
        for source in ['<script src="https://cdn.invalid/x.js"></script>',
                       '<link rel="stylesheet" href="https://cdn.invalid/x.css">',
                       '<img src="https://cdn.invalid/x.png">',
                       '<iframe src="https://cdn.invalid/"></iframe>']:
            with self.subTest(source=source), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / 'index.html').write_text(source)
                self.assertTrue(any('remote' in e for e in build.check_site(root)))

    def test_remote_css_is_rejected(self):
        for source in ['@import "https://cdn.invalid/x.css";',
                       'body { background:url(https://cdn.invalid/x.png); }']:
            with self.subTest(source=source), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / 'style.css').write_text(source)
                self.assertTrue(any('remote' in e for e in build.check_site(root)))

    def test_handwritten_code_cannot_claim_verification(self):
        for source in ['<pre data-example="quickstart">fake</pre>',
                       '<div data-reference="source">fake</div>']:
            with self.subTest(source=source):
                with self.assertRaisesRegex(ValueError, 'handwritten'):
                    build.render_examples(source, {})

    def test_build_output_rejects_stale_file(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'dist'
            root.mkdir()
            (root/'stale.html').write_text('old')
            build.prepare_output(root)
            self.assertFalse((root/'stale.html').exists())

    def test_valid_link_and_source_declaration(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'index.html').write_text('<a href="#ok">yes</a><h2 id="ok">OK</h2><pre data-reference="source" data-language="text"><code>declaration</code></pre>')
            self.assertEqual(build.check_site(root), [])


if __name__ == '__main__':
    unittest.main()
