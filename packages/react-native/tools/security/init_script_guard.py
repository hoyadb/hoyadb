"""SPEC-0034 R1 static validation of r1-compile-alignment.init.gradle (Gradle is never executed here).

Not a general Gradle/Groovy parser: it accepts only the reviewed shape. verify_pinned() additionally requires
the exact reviewed script bytes (R1_SCRIPT_SHA256) and is the check a runner must apply before Gradle starts.
"""
from dataclasses import dataclass, field
import hashlib
from pathlib import Path
import re

R1_SCRIPT_SHA256 = '74aa545a6c82cbdc434e71328653979715659818603a6c0d60627245e8d1f9a0'
EXPECTED_CONSTRAINTS = {'com.google.guava:guava': '33.3.1-jre', 'com.google.code.gson:gson': '2.11.0',
                        'com.google.errorprone:error_prone_annotations': '2.30.0'}

BUILD = 'gradle-plugin-root'
PROJECTS = [':react-native-gradle-plugin', ':settings-plugin', ':shared']
CONFIGURATION = 'compileClasspath'
FORBIDDEN = ['force', 'exclude', 'dependencySubstitution', 'substitute', 'eachDependency', 'resolutionStrategy',
             'strictly', 'componentSelection', 'reject', 'prefer', 'allDependencies', 'withDependencies', 'beforeResolve']
REQUIRED_SNIPPETS = [
    'project.configurations.matching { it.name == HOYADB_R1_CONFIGURATION }',
    'classpath.extendsFrom(holder)',
    'canBeResolved = false',
    'canBeConsumed = false',
    'version { require required }',
    'if (loaded.rootProject.name != HOYADB_R1_BUILD)',
    'if (!(project.path in HOYADB_R1_PROJECTS))',
]


@dataclass
class Result:
    ok: bool
    problems: list = field(default_factory=list)
    build: str = None
    projects: list = None
    configuration: str = None
    constraints: dict = None


class _Unterminated(Exception):
    pass


def _code(text):
    """Remove // and /* */ comments outside '...', "...", '''...''' and \"\"\"...\"\"\" literals (kept verbatim)."""
    out, i, n = [], 0, len(text)
    while i < n:
        if text.startswith("'''", i) or text.startswith('"""', i):
            quote = text[i:i + 3]
            end = text.find(quote, i + 3)
            if end < 0:
                raise _Unterminated()
            out.append(text[i:end + 3])
            i = end + 3
        elif text[i] in '\'"':
            quote, j = text[i], i + 1
            while j < n and text[j] != quote:
                if text[j] == '\\':
                    j += 1
                elif text[j] == '\n':
                    raise _Unterminated()
                j += 1
            if j >= n:
                raise _Unterminated()
            out.append(text[i:j + 1])
            i = j + 1
        elif text.startswith('//', i):
            end = text.find('\n', i)
            i = n if end < 0 else end
        elif text.startswith('/*', i):
            end = text.find('*/', i + 2)
            if end < 0:
                raise _Unterminated()
            i = end + 2
        else:
            out.append(text[i])
            i += 1
    return ''.join(out)


def check(path, approved):
    problems = []
    try:
        code = _code(Path(path).read_text())
    except _Unterminated:
        return Result(ok=False, problems=['unterminated string or comment'])
    for word in FORBIDDEN:
        if re.search(r'\b' + re.escape(word) + r'\b', code):
            problems.append('forbidden resolution override: ' + word)
    for snippet in REQUIRED_SNIPPETS:
        if snippet not in code:
            problems.append('missing reviewed scope: ' + snippet)
    build = re.search(r"def HOYADB_R1_BUILD = '([^']+)'", code)
    projects = re.search(r'def HOYADB_R1_PROJECTS = \[([^\]]*)\]', code)
    configuration = re.search(r"def HOYADB_R1_CONFIGURATION = '([^']+)'", code)
    block = re.search(r'def HOYADB_R1_CONSTRAINTS = \[(.*?)\n\]', code, re.S)
    build = build.group(1) if build else None
    projects = re.findall(r"'([^']+)'", projects.group(1)) if projects else None
    configuration = configuration.group(1) if configuration else None
    constraints = dict(re.findall(r"'([^':]+:[^':]+)'\s*:\s*'([^']+)'", block.group(1))) if block else None
    if build != BUILD:
        problems.append('unexpected build scope')
    if projects != PROJECTS:
        problems.append('unexpected project scope')
    if configuration != CONFIGURATION:
        problems.append('unexpected configuration scope')
    if constraints != EXPECTED_CONSTRAINTS:
        problems.append('constraint set differs from reviewed')
    if not constraints:
        problems.append('no constraints')
    else:
        for module, version in constraints.items():
            if '%s:%s' % (module, version) not in approved:
                problems.append('unapproved constraint ' + module)
    return Result(ok=not problems, problems=problems, build=build, projects=projects, configuration=configuration, constraints=constraints)


def verify_pinned(path, approved):
    """Runner pre-launch check: exact reviewed bytes AND the reviewed structure."""
    result = check(path, approved)
    if hashlib.sha256(Path(path).read_bytes()).hexdigest() != R1_SCRIPT_SHA256:
        result.problems.append('script sha256 differs from pin')
        result.ok = False
    return result
