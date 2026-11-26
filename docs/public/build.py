"""SPEC-0041 / TC-0045: build a complete offline site with no Python dependencies."""
import argparse
import hashlib
import highlight
import html
import json
import os
import re
import shutil
import subprocess
import textwrap
from datetime import datetime, timezone
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
OUT = HERE / 'dist'


def prepare_output(root):
    root = Path(root)
    if root.is_symlink():
        raise ValueError(f'refusing symlink build output: {root}')
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)


def example_regions(source):
    regions, active, lines = {}, None, []
    for line in source.splitlines(keepends=True):
        marker = re.fullmatch(r'\s*// docs:(start|end) ([\w-]+)\s*', line)
        if marker:
            action, name = marker.groups()
            if action == 'start':
                if active is not None or name in regions:
                    raise ValueError(f'duplicate or nested example region: {name}')
                active, lines = name, []
            else:
                if active != name:
                    raise ValueError(f'unmatched example region: {name}')
                regions[name] = ''.join(lines)
                active = None
        elif active is not None:
            lines.append(line)
    if active is not None:
        raise ValueError(f'unclosed example region: {active}')
    return regions


def render_examples(body, examples):
    if re.search(r'<\s*pre\b|\bdata-(?:example|reference)\s*=', body, re.I):
        raise ValueError('handwritten code block or verification label in page source')
    def include(match):
        name = match[1]
        if name not in examples:
            raise ValueError(f'missing example: {name}')
        entry = examples[name]
        path, source = (entry['path'], entry['source']) if isinstance(entry, dict) else entry
        status = entry.get('status', 'executed') if isinstance(entry, dict) else 'executed'
        reason = entry.get('reason', '') if isinstance(entry, dict) else ''
        lang = highlight.language(path)
        if lang == 'text' or status not in {'executed', 'source-checked'}:
            raise ValueError(f'{name}: missing language or invalid verification status')
        if (status == 'source-checked' and not reason) or (status == 'executed' and reason):
            raise ValueError(f'{name}: verification status conflicts with reason')
        label = 'Source-checked · not compiled or executed'
        if status == 'executed':
            label = entry.get('execution_label', 'Executed example') if isinstance(entry, dict) else 'Executed example'
        regions = example_regions(source)
        if match[2]:
            if match[2] not in regions:
                raise ValueError(f'missing example region: {name}:{match[2]}')
            source = textwrap.dedent(regions[match[2]])
        return (f'<figure class="example"><figcaption>{html.escape(label)} · {highlight.LABELS[lang]} · '
                f'<a href="examples/{html.escape(path)}">{html.escape(path)}</a> · '
                f'<a href="verification.html#case-{"rust-build" if name == "support" else name}">verification</a>'
                f'{" · " + html.escape(reason) if reason else ""}</figcaption>'
                f'<pre data-example="{html.escape(name)}" data-language="{lang}" data-status="{status}">'
                f'<code class="language-{lang}">{highlight.highlight(source, lang)}</code></pre></figure>')
    return re.sub(r'\{\{example:([\w-]+)(?::([\w-]+))?\}\}', include, body)


class Document(HTMLParser):
    def __init__(self, source):
        super().__init__(convert_charrefs=True)
        self.ids = set()
        self.redirect = None
        self.links = []
        self.errors = []
        self.assets = []
        self.labelled = []
        self.feed(source)

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if tag == 'meta' and a.get('http-equiv', '').lower() == 'refresh':
            target = re.fullmatch(r'\s*0\s*;\s*url\s*=\s*(.+)', a.get('content', ''), re.I)
            if target:
                self.redirect = target[1].strip()
                if urlsplit(self.redirect).scheme or urlsplit(self.redirect).netloc:
                    self.errors.append('remote redirect not permitted')
        if 'id' in a:
            if a['id'] in self.ids:
                self.errors.append(f'duplicate id: {a["id"]}')
            self.ids.add(a['id'])
        if tag in ('a', 'link', 'script', 'img', 'iframe'):
            target = a.get('href') if tag in ('a', 'link') else a.get('src')
            if target:
                self.links.append(target)
                if tag != 'a':
                    self.assets.append(target)
        if tag == 'pre':
            labelled = 'data-example' in a or 'data-reference' in a
            self.labelled.append(labelled)
            if a.get('data-language') not in highlight.LABELS:
                self.errors.append('missing or unknown code language')
            if not labelled:
                self.errors.append('unlabelled code block')
        if tag == 'code' and self.labelled and not self.labelled[-1]:
            self.errors.append('unlabelled code')

    def handle_endtag(self, tag):
        if tag == 'pre' and self.labelled:
            self.labelled.pop()


def check_site(root):
    root = Path(root).resolve()
    documents = {p.resolve(): Document(p.read_text()) for p in root.rglob('*.html')}
    errors = []
    for path, doc in documents.items():
        # rustdoc has its own generated structure and source block conventions.
        if path.relative_to(root).parts[0] != 'api':
            errors.extend(f'{path.relative_to(root)}: {e}' for e in doc.errors)
        if doc.redirect and (urlsplit(doc.redirect).scheme or urlsplit(doc.redirect).netloc):
            errors.append(f'{path.relative_to(root)}: remote redirect {doc.redirect}')
        for asset in doc.assets:
            url = urlsplit(asset)
            if url.scheme or url.netloc:
                errors.append(f'{path.relative_to(root)}: remote runtime asset {asset}')
        for link in doc.links:
            url = urlsplit(link)
            if url.scheme or url.netloc:
                continue
            dest = (path.parent / unquote(url.path)).resolve() if url.path else path
            if dest.is_dir():
                dest /= 'index.html'
            if not dest.is_relative_to(root) or not dest.exists():
                errors.append(f'{path.relative_to(root)}: missing local link {link}')
            elif dest in documents:
                seen = set()
                while dest in documents and documents[dest].redirect:
                    if dest in seen:
                        errors.append(f'{path.relative_to(root)}: redirect cycle {link}')
                        break
                    seen.add(dest)
                    redirect = urlsplit(documents[dest].redirect)
                    if redirect.scheme or redirect.netloc:
                        errors.append(f'{path.relative_to(root)}: remote redirect {link}')
                        break
                    dest = (dest.parent / unquote(redirect.path)).resolve()
                    if not dest.is_relative_to(root) or not dest.exists():
                        errors.append(f'{path.relative_to(root)}: missing redirect target {link}')
                        break
                if not url.fragment or dest not in documents:
                    continue
                ids = documents[dest].ids
                fragment = unquote(url.fragment)
                valid = fragment in ids or url.fragment in ids
                # rustdoc's source viewer uses #start-end for line ranges.
                line_range = re.fullmatch(r'(\d+)-(\d+)', fragment)
                if line_range and 'src' in dest.relative_to(root).parts and dest.relative_to(root).parts[0] == 'api':
                    valid = line_range[1] in ids and line_range[2] in ids
                if not valid:
                    errors.append(f'{path.relative_to(root)}: missing fragment {link}')
    for path in root.rglob('*.css'):
        css = path.read_text()
        if re.search(r'@import\b', css, re.I):
            errors.append(f'{path.relative_to(root)}: remote/import CSS not permitted')
        for value in re.findall(r'url\(\s*[\'\"]?([^\)\'\"]+)', css, re.I):
            url = urlsplit(value.strip())
            if (url.scheme and not value.strip().lower().startswith('data:image/')) or url.netloc:
                errors.append(f'{path.relative_to(root)}: remote CSS asset {value}')
    return errors


def check_symbols(inventory, reference_root):
    errors = []
    for guide, entries in inventory.items():
        for entry in entries:
            path = Path(reference_root) / entry['file']
            if not path.is_file():
                errors.append(f"{guide}: missing reference source {entry['file']}")
                continue
            source = path.read_text()
            for name in entry['names']:
                if not re.search(r'\b' + re.escape(name) + r'\b', source):
                    errors.append(f"{guide}: missing reference symbol {name} in {entry['file']}")
    return errors


def source_page(path):
    path = Path(path)
    if path.is_absolute() or '..' in path.parts or not (REPO / path).is_file():
        raise ValueError(f'missing source: {path}')
    output = OUT / 'source' / (str(path) + '.html')
    output.parent.mkdir(parents=True, exist_ok=True)
    depth = len(output.relative_to(OUT).parts) - 1
    home = '../' * depth + 'index.html'
    output.write_text('<!doctype html><html lang="en"><meta charset="utf-8">'
        f'<meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="{"../" * depth}assets/style.css"><title>{html.escape(str(path))} · source</title><body class="source-view"><a href="{home}">Documentation</a>'
        f'<h1>{html.escape(str(path))}</h1><p>Source-only reference; not an application example.</p>'
        f'<p>{highlight.LABELS[highlight.language(path)]}</p><pre data-reference="source" data-language="{highlight.language(path)}"><code>{highlight.highlight((REPO/path).read_text(), highlight.language(path))}</code></pre></body></html>')
    return 'source/' + str(path) + '.html'


def declarations_page(files):
    sections = []
    for path in files:
        rel = path.relative_to(REPO).as_posix()
        target = source_page(rel)
        sections.append(f'<h2 id="{html.escape(rel.replace('/', '-').replace('.', '-'))}">{html.escape(rel)}</h2>'
                        f'<p>Source-only declaration reference · <a href="{target}">complete source</a>. '
                        f'Declarations are not executable usage examples. Language: {highlight.LABELS[highlight.language(path)]}.</p>'
                        f'<pre data-reference="declaration" data-language="{highlight.language(path)}"><code>{highlight.highlight(path.read_text(), highlight.language(path))}</code></pre>')
    return ''.join(sections)


def validate_example_report(name, entry, commands):
    rows = [row for row in commands if row['name'] == name and row['exit_code'] == 0]
    if entry.get('status', 'executed') == 'source-checked':
        if not any(row.get('mode') == 'source-check' for row in rows):
            raise ValueError(f'{name}: source-check verification mode missing')
    else:
        runs = [row for row in rows if row.get('mode', 'execution') == 'execution']
        if not runs:
            raise ValueError(f'{name}: example has no passing execution')
        if name != 'rust-build':
            markers = json.loads((HERE/'example-completions.json').read_text())
            if name not in markers or not any(markers[name] in row.get('output', '').splitlines() for row in runs):
                raise ValueError(f'{name}: final completion line missing from execution output')


def kotlin_execution_label(report):
    proof = report.get('kotlin_run', {})
    expected = report['source_hashes'].get('target/documentation-languages/kotlin/manifest.json')
    if (not expected or proof.get('manifest_sha256') != expected or proof.get('exit') != 0
            or proof.get('completion_line') != 'kotlin tour: ok'
            or 'app_process' not in proof.get('command', [])
            or not proof.get('artifacts') or proof['artifacts'] != report.get('kotlin_build', {}).get('artifacts')):
        raise ValueError('Kotlin execution proof is missing, stale or not an Android app_process run')
    device = proof.get('device', {})
    sdk = device.get('sdk', '')
    if device.get('abi') != 'x86_64' or not re.fullmatch(r'[1-9]\d*', sdk) or int(sdk) < 24:
        raise ValueError('Kotlin runtime device facts do not match the compiled target')
    return f'Executed on Android emulator (x86_64, API {sdk}, app_process; not an APK build or device certification)'


def build(rustdoc=True):
    config = json.loads((HERE / 'site.json').read_text())
    symbol_errors = check_symbols(json.loads((HERE/'symbols.json').read_text()), REPO)
    if symbol_errors:
        raise ValueError('\n'.join(symbol_errors))
    report_path = HERE / 'verification.json'
    if not report_path.exists():
        raise ValueError('Run python3 docs/public/verify.py before building: missing verification.json')
    report = json.loads(report_path.read_text())
    if not re.fullmatch(r'[0-9a-f]{40}', report['commit']):
        raise ValueError('invalid report commit')
    for row in report['commands']:
        if not re.fullmatch(r'[a-z0-9_-]+', row['name']):
            raise ValueError('invalid report command name')
    for page in config:
        if not re.fullmatch(r'[a-z0-9-]+', page['slug']):
            raise ValueError('invalid page slug')
    # A successful report is bound to every library and documentation example byte.
    for rel, expected in report['source_hashes'].items():
        path = REPO / rel
        if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise ValueError(f'stale verification: {rel}; rerun verify.py')
    rust_dir = REPO / 'target/public-documentation/examples/doc'
    if rustdoc:
        if rust_dir.exists():
            shutil.rmtree(rust_dir)
        env = dict(os.environ, CARGO_TARGET_DIR=str(REPO/'target/public-documentation/examples'))
        subprocess.run(['cargo', 'doc', '--workspace', '--lib', '--no-deps', '--offline', '--locked'], cwd=REPO, env=env, check=True)
    rust_dir = REPO / 'target/public-documentation/examples/doc'
    if not (rust_dir / 'hoyadb_core/index.html').exists():
        raise ValueError('missing Rust reference; build without --skip-rustdoc first')
    prepare_output(OUT)
    shutil.copytree(rust_dir, OUT/'api', dirs_exist_ok=True)
    (OUT/'api/index.html').write_text('<!doctype html><html lang="en"><meta charset="utf-8"><title>Rust API reference</title><body><h1>Rust API reference</h1><a href="../rust-reference.html">Library documentation</a><ul>' + ''.join(f'<li><a href="{name}/index.html">{name}</a></li>' for name in ['hoyadb_core','hoyadb_storage_memory','hoyadb_storage_sqlite','hoyadb_bindings']) + '</ul></body></html>')
    shutil.copytree(HERE/'assets', OUT/'assets', dirs_exist_ok=True)
    shutil.copytree(REPO/'examples', OUT/'examples', ignore=shutil.ignore_patterns('target', '__pycache__', '.build', '.swiftpm'), dirs_exist_ok=True)
    examples = {'support': ('src/lib.rs', (REPO/'examples/src/lib.rs').read_text())}
    examples.update({p.stem: (p.relative_to(REPO/'examples').as_posix(), p.read_text()) for p in (REPO/'examples/src/bin').glob('*.rs')})
    for name in ['javascript','application']:
        js = REPO/f'examples/{name}.mjs'
        if js.exists():
            examples[name] = (f'{name}.mjs', js.read_text())
    for name, entry in json.loads((HERE/'language-examples.json').read_text()).items():
        path = Path(entry['path'])
        if path.is_absolute() or '..' in path.parts or not (REPO/'examples'/path).is_file():
            raise ValueError(f'{name}: missing or unsafe source: {path}')
        clean_entry = {key:value for key,value in entry.items() if key in {'path','status','reason'}}
        if name == 'kotlin' and entry['status'] == 'executed':
            clean_entry['execution_label'] = kotlin_execution_label(report)
        examples[name] = {**clean_entry, 'source':(REPO/'examples'/path).read_text()}
    for name, entry in examples.items():
        validate_example_report('rust-build' if name == 'support' else name,
                                entry if isinstance(entry, dict) else {}, report['commands'])
    generated = {
        'sdk-declarations': [REPO/'packages/client/index.d.ts', REPO/'packages/client/web.d.ts', REPO/'packages/client/application.d.ts', REPO/'packages/client/node.d.ts'],
        'native-reference': [REPO/'crates/hoyadb-native/include/hoyadb.h'],
        'swift-reference': sorted((REPO/'bindings/swift/Sources/HoyaDB').glob('*.swift')),
        'kotlin-reference': sorted((REPO/'bindings/android/src').rglob('*.kt')),
        'react-native-reference': sorted((REPO/'packages/react-native').glob('*.ts')),
    }
    bodies = {}
    for page in config:
        slug = page['slug']
        if slug in generated:
            if not generated[slug]:
                raise ValueError(f'empty reference input: {slug}')
            body = declarations_page(generated[slug])
        elif slug == 'verification':
            body = '<p>Every executable example below ran against the real library. SDK declaration pages are source reference, not runtime claims.</p>'
            body += f'<p>Source commit <code>{html.escape(report["commit"])}</code>; source working tree hashes are recorded because uncommitted working-tree changes are preserved. Run date {html.escape(report["date"])}.</p>'
            for row in report['commands']:
                body += f'<h2 id="case-{row["name"]}">{html.escape(row["name"])}</h2><p>{"Source check only — not compiled or executed" if row.get("mode") == "source-check" else "Executed command"} · Exit {row["exit_code"]} · <code>{html.escape(row["command"])}</code></p><pre data-reference="execution-output" data-language="text"><code>{html.escape(row["output"])}</code></pre>'
            body += '<h2>Reproduce and inspect</h2><p><a href="verification.json">Machine-readable commands, source hashes and results</a>. Run <code>python3 docs/public/verify.py</code>, then <code>python3 docs/public/build.py</code> from the repository root. Swift and C tours run locally on macOS; these runs do not establish iOS or React Native deployment.</p>'
            body += '<p>Swift and C use a fresh core from an archive of the reported HEAD commit, with a task-only standalone workspace stanza. Rust and WASM examples use the hashed working-tree sources. The <a href="native-build.json">native build manifest</a> identifies the exact archived inputs, commands and artifact hashes; packaged XCFrameworks are preserved.</p>'
            device = report['kotlin_run']['device']
            body += f'<p>{html.escape(kotlin_execution_label(report))}. Page size {html.escape(device["page_size"])} bytes. The <a href="kotlin-build.json">Kotlin build manifest</a> and <a href="kotlin-run.json">Android run report</a> record archived source, artifacts and runtime facts. This documentation example does not satisfy any D1/D2, Phase 7 Android or R9 SDK gate. The wider SDK and policy holds remain.</p>'
            body += '<p>Kotlin also uses an archived HEAD core and binding; its guide source is hashed separately. The runner uses the task-owned read-only emulator and removes its temporary device files. It records whether the shared adb server was already running and leaves that server running.</p>'
            body += '<p>Uncompiled snippets use lexical source checks: member calls, capitalized constructors and Kotlin named arguments are compared with current declarations and a small standard-library allowlist. Property accesses, infix/extension calls, lowercase top-level calls and unparsed types are not fully validated. This is neither compilation nor runtime proof.</p>'
            body += '<p>Tooling tests cover missing links, fragments, examples, escaping and code labels. Browser inspection is recorded separately in <a href="browser-check.json">browser-check.json</a>. It is not a WCAG certification.</p>'
        else:
            source = HERE/'pages'/f'{slug}.html'
            if not source.exists():
                raise ValueError(f'missing page: {source}')
            body = render_examples(source.read_text(), examples)
        for symbol in page.get('symbols', []):
            if not (OUT/'api'/symbol).is_file():
                raise ValueError(f'{slug}: missing reference symbol {symbol}')
        body = re.sub(r'href="source:([^"]+)"', lambda m: 'href="' + source_page(m[1]) + '"', body)
        if '{{' in body:
            raise ValueError(f'{slug}: unresolved include')
        bodies[slug] = body
    search = [{'title': p['title'], 'url': p['slug']+'.html', 'text': p['summary']+' '+re.sub('<[^>]+>', ' ', bodies[p['slug']])[:4000]} for p in config]
    search_json = json.dumps(search, ensure_ascii=True).replace('<', '\\u003c')
    commit = report['commit'][:12]
    date = report['date'][:10]
    nav = ''
    for group in dict.fromkeys(p['group'] for p in config):
        nav += f'<h2>{html.escape(group)}</h2><ul>' + ''.join(f'<li><a href="{p["slug"]}.html">{html.escape(p["title"])}</a></li>' for p in config if p['group'] == group) + '</ul>'
    for index, page in enumerate(config):
        slug = page['slug']
        prev = config[index-1] if index else None
        nxt = config[index+1] if index+1 < len(config) else None
        pager = ''.join(f'<a href="{p["slug"]}.html">{label}: {html.escape(p["title"])}</a>' for p,label in [(prev,'Previous'),(nxt,'Next')] if p)
        output = f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="description" content="{html.escape(page['summary'],quote=True)}"><title>{html.escape(page['title'])} · hoyadb</title><link rel="stylesheet" href="assets/style.css"><script>window.HOYA_SEARCH={search_json};</script><script defer src="assets/search.js"></script></head>
<body><a class="skip" href="#main">Skip to content</a><header><a class="brand" href="index.html"><span class="mark">h</span> hoyadb <span>docs</span></a><span class="version">Unreleased · local packages</span><a href="verification.html">Tested examples ↗</a></header>
<div class="layout"><aside><label for="search">Search documentation</label><input id="search" type="search" placeholder="Search documentation" autocomplete="off" aria-controls="search-results"><div id="search-results" aria-live="polite"></div><nav aria-label="Documentation">{nav}</nav></aside><main id="main" tabindex="-1"><p class="eyebrow">{html.escape(page['group'])} / hoyadb</p><h1>{html.escape(page['title'])}</h1><p class="lead">{html.escape(page['summary'])}</p>{bodies[slug]}<nav class="pager" aria-label="Adjacent pages">{pager}</nav><footer>Source {commit} + uncommitted working-tree changes · Built {date} · <a href="verification.html">Verification and limitations</a></footer></main></div></body></html>'''
        (OUT/f'{slug}.html').write_text(output)
    (OUT/'verification.json').write_text(json.dumps(report, indent=2)+'\n')
    (OUT/'native-build.json').write_text(json.dumps(report['native_build'], indent=2)+'\n')
    for key in ['kotlin_build', 'kotlin_run']:
        (OUT/(key.replace('_','-')+'.json')).write_text(json.dumps(report[key], indent=2)+'\n')
    browser = HERE/'browser-check.json'
    browser_report = json.loads(browser.read_text()) if browser.exists() else {}
    browser_current = browser_report.get('sourceHashes') and all(
        (REPO/path).is_file() and hashlib.sha256((REPO/path).read_bytes()).hexdigest() == digest
        for path, digest in browser_report.get('sourceHashes', {}).items())
    (OUT/'browser-check.json').write_text(json.dumps(browser_report if browser_current else
        {'status':'pending current browser inspection; source has changed or no inspection exists'}, indent=2)+'\n')
    errors = check_site(OUT)
    for page in config:
        text = (OUT/f'{page["slug"]}.html').read_text()
        for expected in ['lang="en"', 'aria-label="Documentation"', 'id="main"', 'Skip to content', 'Unreleased · local packages', commit, date]:
            if expected not in text:
                errors.append(f'{page["slug"]}: missing structural marker {expected}')
    if errors:
        raise ValueError('\n'.join(errors))
    executed = sum(not isinstance(e, dict) or e['status'] == 'executed' for name,e in examples.items() if name != 'support')
    source_only = len(examples)-1-executed
    print(f'PASS {len(config)} documentation pages; {executed} executed examples; {source_only} source-checked examples; all local links and fragments resolve')
    print(OUT/'index.html')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--skip-rustdoc', action='store_true', help='reuse an existing local cargo doc build')
    args = parser.parse_args()
    try:
        build(not args.skip_rustdoc)
    except (ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error)+'\n')
