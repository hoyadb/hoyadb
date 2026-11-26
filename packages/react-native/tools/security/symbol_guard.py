"""SPEC-0034 / TC-0038 SR-30 byte-level absence, fork-patch and call-path evidence.

Reads JAR entries with zipfile and parses class constant pools; never loads or executes JAR code.
"""
from pathlib import Path
import zipfile

import classfile


def _classes(jar):
    with zipfile.ZipFile(jar) as archive:
        for name in archive.namelist():
            if name.endswith('.class'):
                yield name, archive.read(name)


def present(jars, entries, relocation_prefixes=()):
    """Map each affected entry to the JARs containing it (directly or under a relocation prefix)."""
    found = {}
    for jar in jars:
        with zipfile.ZipFile(jar) as archive:
            names = set(archive.namelist())
        for entry in entries:
            candidates = [entry] + [prefix + entry for prefix in relocation_prefixes]
            if any(candidate in names for candidate in candidates):
                found.setdefault(entry, []).append(str(jar))
    return found


def declares(jar, class_name, method):
    """True when the exact class declares the method; a missing class is an explicit failure."""
    with zipfile.ZipFile(jar) as archive:
        entry = class_name + '.class'
        if entry not in archive.namelist():
            raise SystemExit('class not present for patch check: ' + class_name)
        info = classfile.parse(archive.read(entry))
    return any(name == method for name, _ in info.methods)


def external_invocations(jars, owner, method):
    """(jar, class) pairs outside the owner class whose constant pool references owner.method."""
    hits = []
    for jar in jars:
        for name, data in _classes(jar):
            info = classfile.parse(data)
            if info.name == owner or info.name.startswith(owner + '$'):
                continue
            if any(ref_owner == owner and ref_name == method for ref_owner, ref_name, _ in info.methodrefs):
                hits.append((str(jar), info.name))
    return sorted(hits)


def string_references(jars, internal_name):
    """Reflective references: the dotted or slashed name as a string constant outside the owner."""
    dotted = internal_name.replace('/', '.')
    hits = []
    for jar in jars:
        for name, data in _classes(jar):
            info = classfile.parse(data)
            if info.name == internal_name:
                continue
            if dotted in info.strings or internal_name in info.strings:
                hits.append((str(jar), info.name))
    return sorted(hits)


def type_users(jars, internal_name, exclude_prefix=''):
    """Classes (outside exclude_prefix) referencing a type as a class constant or in a descriptor."""
    descriptor = 'L' + internal_name + ';'
    hits = []
    for jar in jars:
        for name, data in _classes(jar):
            info = classfile.parse(data)
            if exclude_prefix and info.name.startswith(exclude_prefix):
                continue
            if internal_name in info.class_refs or any(descriptor in text for text in info.utf8):
                hits.append((str(jar), info.name))
    return sorted(hits)
