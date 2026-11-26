"""SPEC-0034 / TC-0038 SR-20..22 Kotlin launch and observed-JVM guards.

Pure functions: callers pass argv, environment and gradle.properties texts; observation records come
from the pinned JDK's own facilities captured by the runner. Only source and setting names are
reported, never option values, so unrelated environment data cannot leak into evidence.
"""
from dataclasses import dataclass, field
import shlex

import java_properties  # independent implementation from the public Properties.load(Reader) API
import native_cache

MalformedProperties = java_properties.MalformedProperties

PINNED = {'kotlin.incremental': 'false', 'kotlin.compiler.execution.strategy': 'in-process'}
NATIVE_COMPRESSION = 'idea.use.native.compression'
OPTION_ENV = ('GRADLE_OPTS', 'JAVA_OPTS', 'JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS', 'JDK_JAVA_OPTIONS')
PROJECT_ENV_PREFIX = 'ORG_GRADLE_PROJECT_'
PROJECT_SYSPROP_PREFIX = 'org.gradle.project.'
DAEMON_NESTED = 'kotlin.daemon.jvm.options'
LOADER_ENV_PREFIX = 'DYLD_'
LZ4_ENTRY_PREFIXES = (
    'org/jetbrains/kotlin/net/jpountz/',
    'org/jetbrains/kotlin/com/intellij/util/CompressionUtil',
    'org/jetbrains/kotlin/com/intellij/util/io/PersistentMapWal',
    'org/jetbrains/kotlin/com/intellij/util/io/CompressedAppendableFile',
    'org/jetbrains/kotlin/incremental/storage/',
    'org/jetbrains/kotlin/incremental/classpathDiff/',
)


@dataclass(frozen=True)
class Violation:
    source: str
    setting: str


@dataclass
class Result:
    ok: bool
    violations: list = field(default_factory=list)
    native_loads: list = field(default_factory=list)
    baseline_loads: list = field(default_factory=list)
    system_cache_loads: list = field(default_factory=list)
    discovered_images: list = field(default_factory=list)


def secure_launch_args():
    return ['-P%s=%s' % item for item in PINNED.items()]


def _enabled(value):
    # java.lang.Boolean.getBoolean semantics
    return value is not None and value.strip().strip('"\'').lower() == 'true'


def _tokens(text):
    try:
        return shlex.split(text)
    except ValueError:
        return text.split()


def _project(source, key, value, out):
    if key in PINNED and (value or '').strip().strip('"\'').lower() != PINNED[key]:
        out.add(Violation(source, key))


def _nested_daemon(source, value, out):
    for item in value.strip().strip('"\'').split(','):
        item = item.strip().strip('"\'').lstrip('-')
        if item.startswith('D'):
            key, _, val = item[1:].partition('=')
            _system(source + ':' + DAEMON_NESTED, key, val, out)


def _system(source, key, value, out):
    if key == NATIVE_COMPRESSION and _enabled(value):
        out.add(Violation(source, NATIVE_COMPRESSION))
    elif key.startswith(PROJECT_SYSPROP_PREFIX):
        _project_or_options(source, key[len(PROJECT_SYSPROP_PREFIX):], value, out)
    elif key == DAEMON_NESTED:
        _nested_daemon(source, value, out)
    elif key == 'org.gradle.jvmargs':
        _jvm_options(source + ':org.gradle.jvmargs', value, out)


def _project_or_options(source, key, value, out):
    if key == 'kotlin.daemon.jvmargs':
        _jvm_options(source + ':kotlin.daemon.jvmargs', value, out)
    else:
        _project(source, key, value, out)


def _jvm_options(source, text, out):
    for token in _tokens(text or ''):
        if token.startswith('-D'):
            key, _, value = token[2:].partition('=')
            _system(source, key, value, out)


def _property_decodings(data):
    """Properties.load(InputStream) decodes ISO-8859-1; a non-ASCII file is also checked as UTF-8 (fail closed)."""
    if isinstance(data, str):
        return [data]
    texts = [data.decode('iso-8859-1')]
    if any(b > 0x7f for b in data):
        try:
            texts.append(data.decode('utf-8'))
        except UnicodeDecodeError:
            pass
    return texts


def check_launch(argv, env, properties):
    out = set()
    seen = {key: False for key in PINNED}
    argv = list(argv)
    i = 0
    while i < len(argv):
        token = argv[i]
        if token in ('-P', '--project-prop', '-D', '--system-prop') and i + 1 < len(argv):
            kind, payload = token, argv[i + 1]
            i += 2
        else:
            for prefix in ('--project-prop=', '--system-prop=', '-P', '-D'):
                if token.startswith(prefix):
                    kind, payload = prefix.rstrip('='), token[len(prefix):]
                    break
            else:
                i += 1
                continue
            i += 1
        key, _, value = payload.partition('=')
        if kind in ('-P', '--project-prop'):
            if key in seen and value.lower() == PINNED[key]:
                seen[key] = True
            _project_or_options('launch', key, value, out)
        else:
            _system('launch', key, value, out)
    for key, ok in seen.items():
        if not ok:
            out.add(Violation('launch', key))
    for name in OPTION_ENV:
        if name in env:
            _jvm_options('env:' + name, env[name], out)
    for name in env:
        if name.startswith(LOADER_ENV_PREFIX):
            out.add(Violation('env:' + name, 'DYLD_ loader environment'))
    for name, value in env.items():
        if name.startswith(PROJECT_ENV_PREFIX):
            key = name[len(PROJECT_ENV_PREFIX):]
            if key == 'kotlin.daemon.jvmargs':
                _jvm_options('env:' + name, value, out)
            else:
                _project('env:' + name, key, value, out)
    for path, data in sorted(properties.items()):
        for text in _property_decodings(data):
            try:
                entries = java_properties.load_reader(text)
            except MalformedProperties:
                out.add(Violation(path, 'malformed properties'))
                continue
            _check_properties(path, entries, out)
    violations = sorted(out, key=lambda v: (v.source, v.setting))
    return Result(ok=not violations, violations=violations)


def _check_properties(path, entries, out):
    for key, value in entries.items():
        if key == 'org.gradle.jvmargs':
            _jvm_options(path + ':org.gradle.jvmargs', value, out)
        elif key == 'kotlin.daemon.jvmargs':
            _jvm_options(path + ':kotlin.daemon.jvmargs', value, out)
        elif key.startswith('systemProp.'):
            _system(path, key[len('systemProp.'):], value, out)
        else:
            _project(path, key, value, out)


def check_observed_jvms(observations, required_roles, native_inventory=None, native_baseline=None,
                        pinned_strategy=PINNED['kotlin.compiler.execution.strategy'], *, system_cache=None):
    """native_inventory: reviewed embedded payloads (e.g. compiler Jansi/LZ4 JNI); native_baseline: the explicitly
    trusted file-backed JDK/platform libraries of the pinned JDK, both matched by name+sha256. system_cache: an
    optional native_cache.SystemCacheEvidence for digest-less macOS shared-cache images. Every observed library
    is classified exactly once; a missing digest is never an exemption outside the reviewed cache record."""
    out, loads, baseline_loads, cache_loads, discovered = set(), [], [], [], []
    inventory = native_inventory or {}
    baseline = native_baseline or {}
    cache_ready = False
    if system_cache is not None:
        host_errors = native_cache.check_host(system_cache)
        out.update(Violation('system-cache', e) for e in host_errors)
        cache_ready = not host_errors
    roles = {o.get('role') for o in observations}
    if not required_roles:
        out.add(Violation('observation', 'no required compiling JVM roles (vacuous)'))
    for role in required_roles:
        if role not in roles:
            out.add(Violation(role, 'unobserved JVM'))
    for o in observations:
        role = o.get('role', '?')
        capture = o.get('capture') or {}
        if capture.get('class_load') is not True or capture.get('native_load') is not True:
            out.add(Violation(role, 'incomplete capture'))
        if not o.get('system_properties') or not isinstance(o.get('system_properties'), dict):
            out.add(Violation(role, 'incomplete capture: effective system properties missing'))
        if not isinstance(o.get('input_arguments'), list):
            out.add(Violation(role, 'incomplete capture: effective input arguments missing'))
        environment = o.get('environment')
        if not isinstance(environment, dict):
            out.add(Violation(role, 'incomplete capture: effective environment missing'))
            environment = {}
        for name in environment:
            if name.startswith(LOADER_ENV_PREFIX):
                out.add(Violation(role, 'DYLD_ loader environment: ' + name))
        if o.get('execution_strategy') != pinned_strategy:
            out.add(Violation(role, 'kotlin.compiler.execution.strategy'))
        for token in o.get('input_arguments') or []:
            if token.startswith('-D'):
                key, _, value = token[2:].partition('=')
                _system(role, key, value, out)
        for key, value in (o.get('system_properties') or {}).items():
            if key == NATIVE_COMPRESSION and _enabled(value):
                out.add(Violation(role, NATIVE_COMPRESSION))
        for name in o.get('classes_loaded') or []:
            internal = name.replace('.', '/')
            if internal.startswith(LZ4_ENTRY_PREFIXES):
                out.add(Violation(role, 'vulnerable LZ4 entry point: ' + internal))
        cached = []
        for lib in o.get('native_libraries_loaded') or []:
            # DEF-0031: validate the entry and its name before any lookup; never echo malformed values.
            if not isinstance(lib, dict):
                out.add(Violation(role, 'incomplete capture: native library entry is not an object'))
                continue
            name, digest = lib.get('name'), lib.get('sha256')
            if not isinstance(name, str) or not name:
                out.add(Violation(role, 'incomplete capture: native library name missing or not a non-empty string'))
                continue
            cache_path = native_cache.lookup(system_cache, name) if cache_ready else None
            if digest is not None:
                bins = [inventory.get(name) == digest, baseline.get(name) == digest]
                if cache_path is not None:
                    out.add(Violation(role, 'file-backed library claims a reviewed system cache path: %s' % name))
                elif sum(bins) > 1:
                    out.add(Violation(role, 'ambiguous native classification: %s' % name))
                elif bins[0]:
                    loads.append((role, name))
                elif bins[1]:
                    baseline_loads.append((role, name))
                else:
                    out.add(Violation(role, 'native library outside reviewed inventory and trusted baseline: %s' % name))
            elif cache_path is not None and name not in inventory and name not in baseline:
                cached.append((cache_path, lib))
            else:
                out.add(Violation(role, 'native library with missing digest outside reviewed system cache: %s' % name))
                if system_cache is not None and 'base_address' in lib:
                    discovered.append((role, name))
        if cache_ready:
            out.update(Violation(role, e) for e in native_cache.check_capture(system_cache, o, cached))
            for path in dict.fromkeys(path for path, _ in cached):
                cache_loads.append((role, path))
    violations = sorted(out, key=lambda v: (v.source, v.setting))
    return Result(ok=not violations, violations=violations, native_loads=loads, baseline_loads=baseline_loads,
                  system_cache_loads=cache_loads, discovered_images=discovered)
