"""Lexical reverse checks for uncompiled snippets; never a substitute for a compiler."""
import re
from html.parser import HTMLParser
import highlight

class Code(HTMLParser):
    def __init__(self, source):
        super().__init__(convert_charrefs=True)
        self.skip = False
        self.source = ''
        self.feed(source)
    def handle_starttag(self, tag, attrs):
        self.skip = dict(attrs).get('class') in {'syntax-string','syntax-comment'}
    def handle_endtag(self, tag):
        self.skip = False
    def handle_data(self, data):
        self.source += ' ' * len(data) if self.skip else data


def check_calls(source, declarations, allowlist, lang='kotlin'):
    # Ignore strings/comments so documentation and business strings aren't APIs.
    code = Code(highlight.highlight(source, lang)).source
    names = set(re.findall(r'\.\s*([A-Za-z_]\w*)\s*\(', code))
    names.update(re.findall(r'\b([A-Z]\w*)\s*[(\{]', code))
    # Kotlin named arguments occur after '(' or ','; assignment statements do not.
    if lang == 'kotlin': names.update(re.findall(r'(?:\(|,)\s*([A-Za-z_]\w*)\s*=(?!=)', code))
    return [f'unknown call, type or named argument: {name}' for name in sorted(names)
            if name not in allowlist and not re.search(r'\b'+re.escape(name)+r'\b', declarations)]


def check_status(entry):
    status = entry.get('status')
    return (status == 'source-checked' and bool(entry.get('reason'))) or (status == 'executed' and not entry.get('reason'))
