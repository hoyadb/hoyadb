"""Offline lexical syntax coloring, not a parser. Every input byte stays text."""
import html
import re
from pathlib import Path

LABELS = {'javascript':'JavaScript', 'typescript':'TypeScript', 'tsx':'TypeScript / TSX',
          'rust':'Rust', 'swift':'Swift', 'kotlin':'Kotlin', 'c':'C', 'json':'JSON',
          'shell':'Shell', 'text':'Plain text'}
KEYWORDS = {
    'javascript': 'async await break case catch class const continue debugger default delete do else export extends false finally for from function if import in instanceof let new null of return static super switch this throw true try typeof undefined var void while yield',
    'typescript': 'abstract as asserts declare enum implements infer interface is keyof namespace never private protected public readonly satisfies type unknown any boolean number string bigint',
    'rust': 'as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while',
    'swift': 'actor as associatedtype async await break case catch class continue default defer deinit do else enum extension fallthrough false fileprivate for func guard if import in init inout internal is let nil nonisolated open operator private protocol public repeat rethrows return self some static struct subscript super switch throw throws true try typealias var weak where while',
    'kotlin': 'abstract actual annotation as break by catch class companion const constructor continue crossinline data delegate do dynamic else enum expect external false final finally for fun get if import in infix init inline inner interface internal is lateinit noinline null object open operator out override package private protected public reified return sealed set super suspend tailrec this throw true try typealias val var vararg when where while',
    'c': 'auto bool break case char const continue default do double else enum extern false float for if inline int long NULL register restrict return short signed sizeof static struct switch typedef union unsigned void volatile while _Bool _Atomic',
    'json': 'true false null',
    'shell': 'if then else elif fi for in do done while case esac function export local readonly return exit set unset source'
}
KEYWORDS['typescript'] += ' ' + KEYWORDS['javascript']
KEYWORDS['tsx'] = KEYWORDS['typescript']
KEYWORDS = {k:set(v.split()) for k,v in KEYWORDS.items()}


RUST_RAW = re.compile(r'(?:br|r)(#+)?"')
SWIFT_RAW = re.compile(r'(#+)("""|")')
CHAR = re.compile(r"'(?:[^'\\\n]|\\(?:[^\n]|u\{[0-9a-fA-F]+\}))'")
NUMBER = re.compile(r'(?:0[xX][\da-fA-F_]+|0[bB][01_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d[\d_]*)?)(?:[a-zA-Z]\w*)?')
WORD = re.compile(r'[A-Za-z_$][\w$]*')
CALL = re.compile(r'\s*\(')


def language(path):
    return {'.js':'javascript','.mjs':'javascript','.cjs':'javascript','.ts':'typescript',
            '.tsx':'tsx','.rs':'rust','.swift':'swift','.kt':'kotlin','.kts':'kotlin',
            '.c':'c','.h':'c','.json':'json','.sh':'shell','.bash':'shell'}.get(Path(path).suffix.lower(), 'text')


def quoted(source, start, delimiter, multiline=False):
    i = start + len(delimiter)
    while i < len(source):
        if source.startswith(delimiter, i): return i + len(delimiter)
        if source[i] == '\n' and not multiline: return i
        if source[i] == '\\': i += 2
        else: i += 1
    return len(source)


def template(source, start):
    # Color the entire template as a string, including balanced nested templates.
    i = start + 1
    depth = 0
    while i < len(source):
        c = source[i]
        if c == '\\': i += 2; continue
        if c == '`':
            if depth == 0: return i+1
            i = template(source, i); continue
        if source.startswith('${', i): depth += 1; i += 2; continue
        if depth and c in "\"'": i = quoted(source, i, c); continue
        if depth and c == '{': depth += 1
        if depth and c == '}': depth -= 1
        i += 1
    return i


def highlight(source, lang):
    if lang not in KEYWORDS: return html.escape(source)
    result, i, previous = [], 0, ''
    js = lang in {'javascript','typescript','tsx'}
    while i < len(source):
        start, end, kind = i, i+1, None
        if (lang != 'shell' and source.startswith('//', i)) or (lang == 'shell' and source.startswith('#', i)):
            end = source.find('\n', i)
            if end < 0: end = len(source)
            kind = 'comment'
        elif lang != 'shell' and source.startswith('/*', i):
            depth, end = 1, i+2
            nested = lang in {'rust','swift','kotlin'}
            while end < len(source) and depth:
                if nested and source.startswith('/*', end): depth += 1; end += 2
                elif source.startswith('*/', end): depth -= 1; end += 2
                else: end += 1
            kind = 'comment'
        else:
            raw = RUST_RAW.match(source, i) if lang == 'rust' else SWIFT_RAW.match(source, i) if lang == 'swift' else None
            if raw:
                hashes = raw[1] or ''
                quote = (raw[2] if lang == 'swift' else '"')
                terminator = quote+hashes
                close = source.find(terminator, raw.end())
                end = len(source) if close < 0 else close+len(terminator)
                kind = 'string'
            elif lang in {'swift','kotlin'} and source.startswith('"""', i):
                end, kind = quoted(source, i, '"""', True), 'string'
            elif js and source[i] == '`': end, kind = template(source, i), 'string'
            elif source[i] == '"' or (source[i] == "'" and lang != 'rust'):
                end, kind = quoted(source, i, source[i], lang == 'shell'), 'string'
            elif lang == 'rust' and CHAR.match(source, i):
                end, kind = quoted(source, i, "'"), 'string'
            elif js and source[i] == '/' and (not previous or previous in {'=','(','[','{',',',':',';','!','?','return','=>','&&','||','typeof','case','yield','await','void','delete','in','of','instanceof'}):
                end, in_class = i+1, False
                while end < len(source) and source[end] != '\n':
                    if source[end] == '\\': end += 2; continue
                    if source[end] == '[': in_class = True
                    elif source[end] == ']': in_class = False
                    elif source[end] == '/' and not in_class:
                        end += 1
                        while end < len(source) and source[end].isalpha(): end += 1
                        kind = 'string'; break
                    end += 1
                if kind is None: end = i+1
            elif source[i] in '0123456789':
                number = NUMBER.match(source, i)
                end, kind = number.end(), 'number'
            elif source[i].isascii() and (source[i].isalpha() or source[i] in '_$'):
                word = WORD.match(source, i)[0]
                end = i+len(word)
                if word in KEYWORDS[lang]: kind = 'keyword'
                elif word[0].isupper(): kind = 'type'
                elif CALL.match(source, end): kind = 'callable'
            elif source[i] in '{}[]();,.:+-*/=<>!&|?%@#': kind = 'punctuation'
        token = source[start:end]
        escaped = html.escape(token)
        result.append(f'<span class="syntax-{kind}">{escaped}</span>' if kind else escaped)
        if token.strip() and kind != 'comment': previous = token
        i = end
    return ''.join(result)
