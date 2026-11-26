"""TC-0047 lexical fidelity, language detection, contrast and honest labels."""
import html
from html.parser import HTMLParser
from pathlib import Path
import re
import unittest
import highlight
import build

class Text(HTMLParser):
    def __init__(self, source):
        super().__init__(convert_charrefs=True)
        self.text = ''
        self.feed(source)
    def handle_data(self, value):
        self.text += value

class LanguageTests(unittest.TestCase):
    def test_language_detection(self):
        for path, expected in {'a.mjs':'javascript','a.d.ts':'typescript','a.tsx':'tsx','a.rs':'rust','a.swift':'swift','a.kt':'kotlin','a.c':'c','a.h':'c','a.json':'json','a.sh':'shell','a.txt':'text'}.items():
            self.assertEqual(highlight.language(path), expected)

    def test_every_language_colors_and_preserves_exact_source(self):
        cases = {'javascript':'const n = 12; // λ\nconst s = "<script>&";',
                 'typescript':'interface Task { title: string } // note\nconst n = 12; const s = "hi";',
                 'tsx':'const n = 12; // note\nconst view = <Task title="hi" />;',
                 'rust':'fn main() { let n = 12; /* note */ let s = "hi"; }',
                 'swift':'let n = 12 // note\nlet s = "hi"',
                 'kotlin':'val n = 12 // note\nval s = "hi"',
                 'c':'int main(void) { int n = 12; /* note */ char *s = "hi"; }',
                 'json':'{"answer":12, "yes":true}',
                 'shell':'export N=12 # note\necho "hi"'}
        for lang, source in cases.items():
            with self.subTest(lang=lang):
                rendered = highlight.highlight(source, lang)
                self.assertEqual(Text(rendered).text, source)
                for token in ['keyword','string','number']:
                    self.assertIn('syntax-'+token, rendered)
                if lang != 'json': self.assertIn('syntax-comment', rendered)
                self.assertNotIn('<script>', rendered)

    def test_lexical_edge_cases(self):
        cases = [('rust', '/* outer /* inner */ tail */ fn go<\'a>(x: &\'a str) { let c = \'a\'; let s = r##"// <tag>"##; }', '/* outer /* inner */ tail */'),
                 ('javascript', 'const r = /a[/*]b/g; const n = 10 / 2; const s = `outer ${`inner ${n}`} tail`; // done', '/a[/*]b/g'),
                 ('swift', 'let s = #"// raw"#; let t = """\n/* string */\n"""; // real', '#"// raw"#'),
                 ('kotlin', 'val s = """\n// ${value}\n"""; // real', '"""\n// ${value}\n"""'),
                 ('c', 'char *s = "// not a comment /* */"; // real', '"// not a comment /* */"')]
        for lang, source, token in cases:
            with self.subTest(lang=lang):
                rendered = highlight.highlight(source, lang)
                self.assertEqual(Text(rendered).text, source)
                self.assertIn(html.escape(token), rendered)
        rendered = highlight.highlight('"unterminated\nconst n = 2;', 'javascript')
        self.assertIn('syntax-keyword', rendered)

    def test_unknown_language_stays_plain_escaped(self):
        self.assertEqual(highlight.highlight('<unsafe>&λ\n', 'unknown'), '&lt;unsafe&gt;&amp;λ\n')

    def test_palette_meets_contrast(self):
        css = (Path(__file__).parent/'assets/style.css').read_text()
        def lum(color):
            cs = [int(color[i:i+2],16)/255 for i in (1,3,5)]
            cs = [c/12.92 if c <= .04045 else ((c+.055)/1.055)**2.4 for c in cs]
            return sum(c*w for c,w in zip(cs,[.2126,.7152,.0722]))
        for token in ['keyword','string','comment','number','type','callable','punctuation']:
            match = re.search(r'--syntax-'+token+r':(#[0-9a-fA-F]{6})', css)
            self.assertIsNotNone(match, token)
            self.assertGreaterEqual((lum(match[1])+.05)/(lum('#152b23')+.05), 4.5, token)

    def test_source_only_status_and_language_are_explicit(self):
        source = {'path':'kotlin/TodoGuide.kt','source':'val n = 12', 'status':'source-checked', 'reason':'JVM execution held'}
        result = build.render_examples('{{example:kotlin}}', {'kotlin': source})
        self.assertIn('Source-checked · not compiled or executed', result)
        self.assertIn('Kotlin', result)
        self.assertIn('JVM execution held', result)
        self.assertIn('data-language="kotlin"', result)
        self.assertIn('syntax-keyword', result)
        for invalid in [{**source,'status':'executed','reason':'JVM execution held'}, {**source,'path':'unknown.txt'}, {**source,'reason':''}]:
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                build.render_examples('{{example:kotlin}}', {'kotlin': invalid})

    def test_report_cannot_promote_source_checks_to_execution(self):
        entry = {'status':'source-checked', 'path':'demo.kt', 'reason':'held'}
        with self.assertRaisesRegex(ValueError, 'mode'):
            build.validate_example_report('demo', entry, [{'name':'demo','exit_code':0}])
        build.validate_example_report('demo', entry, [{'name':'demo','exit_code':0,'mode':'source-check'}])
        with self.assertRaisesRegex(ValueError, 'execution'):
            build.validate_example_report('demo', {'status':'executed'}, [{'name':'demo','exit_code':0,'mode':'source-check'}])

    def test_execution_requires_completion_after_assertions(self):
        with self.assertRaisesRegex(ValueError, 'completion'):
            build.validate_example_report('typescript', {'status':'executed'}, [{'name':'typescript','exit_code':0,'output':'compiled only'}])
        build.validate_example_report('typescript', {'status':'executed'}, [{'name':'typescript','exit_code':0,'output':'typescript tour: ok\n'}])

    def test_unlisted_invented_source_calls_are_rejected(self):
        import source_checks
        self.assertEqual(source_checks.check_calls('db.fetch(Query("todos"))', 'fun fetch(query: Query); class Query', set()), [])
        errors = source_checks.check_calls('db.nonexistentMethod()', 'fun fetch(query: Query)', set())
        self.assertTrue(any('nonexistentMethod' in error for error in errors))
        errors = source_checks.check_calls('db.fetch(query = Query("todos"), invented = true)', 'fun fetch(query: Query); class Query', set())
        self.assertTrue(any('invented' in error for error in errors))
        self.assertEqual(source_checks.check_calls('// db.nonexistentMethod()\nprintln("db.fake()")', '', {'println'}), [])

    def test_unicode_identifiers_and_numeric_symbols_stay_text(self):
        source = 'const café = ² + ١; // λ\n'
        error, rendered = None, ''
        try: rendered = highlight.highlight(source, 'javascript')
        except Exception as caught: error = caught
        self.assertIsNone(error, f'ordinary Unicode source must not crash: {error}')
        self.assertEqual(Text(rendered).text, source)

    def test_code_block_without_language_fails_site_check(self):
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/'index.html').write_text('<pre data-example="fake"><code>fake()</code></pre>')
            self.assertTrue(any('language' in error for error in build.check_site(root)))

    def test_all_binding_quickstarts_are_discoverable(self):
        import json
        root = Path(__file__).parent
        slugs = {entry['slug'] for entry in json.loads((root/'site.json').read_text())}
        picker = (root/'pages/getting-started.html').read_text()
        for slug in ['getting-started','typescript-getting-started','rust-getting-started','swift-getting-started','kotlin-getting-started','react-native-getting-started','c-getting-started']:
            self.assertIn(slug, slugs)
            if slug != 'getting-started': self.assertIn(slug+'.html', picker)

    def test_executed_snippets_retain_additional_source_checks(self):
        import source_checks
        self.assertTrue(source_checks.check_status({'status':'source-checked','reason':'native unavailable'}))
        self.assertTrue(source_checks.check_status({'status':'executed'}))
        self.assertFalse(source_checks.check_status({'status':'source-checked'}))
        self.assertFalse(source_checks.check_status({'status':'executed','reason':'not run'}))

    def test_kotlin_caption_requires_real_runtime_proof(self):
        import copy
        proof = {'exit':0,'completion_line':'kotlin tour: ok','manifest_sha256':'1'*64,
                 'artifacts':{'target/documentation-languages/kotlin/classes.dex':'2'*64},
                 'command':['adb','shell','app_process'], 'device':{'abi':'x86_64','sdk':'37'}}
        report = {'kotlin_run':proof,'kotlin_build':{'artifacts':proof['artifacts']},
                  'source_hashes':{'target/documentation-languages/kotlin/manifest.json':'1'*64}}
        self.assertEqual(build.kotlin_execution_label(report),
                         'Executed on Android emulator (x86_64, API 37, app_process; not an APK build or device certification)')
        for field, value in [('exit',1), ('completion_line','compiled only'), ('manifest_sha256','3'*64), ('command',['java'])]:
            changed = copy.deepcopy(report); changed['kotlin_run'][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                build.kotlin_execution_label(changed)

if __name__ == '__main__': unittest.main()
