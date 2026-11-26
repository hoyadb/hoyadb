"""TC-0041 SI-Y03: source-token guard for the single core SyncDelta proof site.

This is a narrow source convention check, not a Rust parser or an ownership
proof. Rust visibility, compile-fail tests and review establish the opaque API;
this check pins the named constructor, literal and positive owner gate. Macro
expansion and arbitrary equivalent Rust spellings are outside its claim.
"""

import argparse
import hashlib
import json
import re
from pathlib import Path


STORAGE = "storage/sync.rs"
OWNER = "command/owner.rs"
CONSTRUCTOR = "validated_exclusive"
GATE = ["if", "self", ".", "db", ".", "db", ".", "validated_exclusive", "{"]


def tokens(source):
    """Skip nested comments and Rust quoted literals; retain lifetime tokens."""
    result = []
    pos = 0
    while pos < len(source):
        if source[pos].isspace():
            pos += 1
            continue
        if source.startswith("//", pos):
            end = source.find("\n", pos)
            pos = len(source) if end < 0 else end + 1
            continue
        if source.startswith("/*", pos):
            depth, pos = 1, pos + 2
            while depth and pos < len(source):
                if source.startswith("/*", pos):
                    depth, pos = depth + 1, pos + 2
                elif source.startswith("*/", pos):
                    depth, pos = depth - 1, pos + 2
                else:
                    pos += 1
            if depth:
                raise ValueError("unterminated comment")
            continue
        raw = re.match(r'(?:br|cr|r)(#*)"', source[pos:])
        if raw:
            end = source.find('"' + raw[1], pos + len(raw[0]))
            if end < 0:
                raise ValueError("unterminated raw string")
            pos = end + 1 + len(raw[1])
            result.append("LITERAL")
            continue
        if source[pos] == '"':
            pos += 1
            while pos < len(source) and source[pos] != '"':
                pos += 2 if source[pos] == "\\" else 1
            if pos >= len(source):
                raise ValueError("unterminated string")
            pos += 1
            result.append("LITERAL")
            continue
        char = re.match(r"'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'", source[pos:])
        if char:
            pos += len(char[0])
            result.append("LITERAL")
            continue
        token = re.match(r"[A-Za-z_][A-Za-z_0-9]*|::|[^\s]", source[pos:])[0]
        result.append(token)
        pos += len(token)
    return result


def pairs(items):
    stack, matched = [], {}
    closing = {")": "(", "]": "[", "}": "{"}
    for i, item in enumerate(items):
        if item in ("(", "[", "{"):
            stack.append(i)
        elif item in closing:
            if not stack or items[stack[-1]] != closing[item]:
                raise ValueError("unbalanced delimiter")
            start = stack.pop()
            matched[start] = i
    if stack:
        raise ValueError("unclosed delimiter")
    return matched


def sequences(items, sequence):
    size = len(sequence)
    return [i for i in range(len(items) - size + 1) if items[i:i + size] == sequence]


def function_body(items, matched, start):
    """Locate the body after a named fn, skipping its parameter delimiters."""
    pos = start + 2
    while pos < len(items):
        if items[pos] == "{":
            return pos, matched[pos]
        if items[pos] == ";":
            return None
        pos = matched[pos] + 1 if items[pos] in ("(", "[") else pos + 1
    return None


def within(index, span):
    return span is not None and span[0] < index < span[1]


def attributes_before(items, matched, declaration):
    ends = {end: start for start, end in matched.items()}
    pos = declaration
    if items[pos - 1:pos] == ["pub"]:
        pos -= 1
    elif items[pos - 1:pos] == [")"]:
        opening = ends[pos - 1]
        if items[max(0, opening - 1):opening] == ["pub"]:
            pos = opening - 1
    attributes = []
    while pos > 1 and items[pos - 1] == "]":
        opening = ends[pos - 1]
        if items[max(0, opening - 1):opening] != ["#"]:
            break
        attributes.extend(items[opening + 1:pos - 1])
        pos = opening - 1
    return attributes


def audit_sources(sources):
    """Return violations for all source texts keyed relative to core/src."""
    errors, definitions, calls, constructions, structs = [], [], [], [], []
    for path, source in sorted(sources.items()):
        try:
            items = tokens(source)
            matched = pairs(items)
        except ValueError as error:
            errors.append(f"{path}: lexical input failure: {error}")
            continue
        impls = []
        for i, token in enumerate(items):
            if token != "impl":
                continue
            opening = next((j for j in range(i + 1, len(items)) if items[j] in ("{", ";")), None)
            if opening is None or items[opening] != "{":
                continue
            header = items[i + 1:opening]
            # A where clause follows the implemented type, not the body. Both
            # inherent and trait impls may contain Self proof constructions.
            if "where" in header:
                header = header[:header.index("where")]
            target = header[header.index("for") + 1:] if "for" in header else header
            if "SyncDelta" not in target:
                continue
            impls.append((opening, matched[opening]))
            if "for" in header:
                errors.append(f"{path}: unreviewed trait implementation for SyncDelta")
        constructors = []
        for i in sequences(items, ["fn", CONSTRUCTOR]):
            body = function_body(items, matched, i)
            definitions.append((path, i))
            constructors.append(body)
            if (path != STORAGE or items[max(0, i - 4):i] != ["pub", "(", "crate", ")"]
                    or not any(within(i, span) for span in impls) or body is None):
                errors.append(f"{path}: expected crate-only constructor in impl SyncDelta")
        persists = [function_body(items, matched, i) for i in sequences(items, ["fn", "persist"])]
        gates = [(i + len(GATE) - 1, matched[i + len(GATE) - 1]) for i in sequences(items, GATE)]
        for i, token in enumerate(items):
            if token != CONSTRUCTOR:
                continue
            if i and items[i - 1] == "fn":
                continue
            # The owner also has a field of this name. Qualified constructor
            # values count even when passed/stored rather than called here.
            if items[max(0, i - 1):i] != ["::"] and items[i + 1:i + 2] != ["("]:
                continue
            calls.append((path, i))
            if (path != OWNER or items[max(0, i - 2):i] != ["SyncDelta", "::"]
                    or items[i + 1:i + 2] != ["("]):
                errors.append(f"{path}: unexpected constructor call or constructor reference")
            if not any(within(i, span) for span in persists):
                errors.append(f"{path}: constructor call outside persist")
            if not any(within(i, span) for span in gates):
                errors.append(f"{path}: missing exact validated Exclusive gate")
        for i in sequences(items, ["SyncDelta", "{"]):
            if items[max(0, i - 2):i] == ["-", ">"]:
                continue
            if i and items[i - 1] == "struct":
                structs.append((path, i))
                attributes = attributes_before(items, matched, i - 1)
                if "derive" in attributes and any(name in attributes for name in ("Clone", "Copy", "Default")):
                    errors.append(f"{path}: SyncDelta cannot derive Clone, Copy or Default")
                continue
            if any(i + 1 == span[0] for span in impls):
                continue
            end = matched[i + 1]
            if items[end + 1:end + 2] != ["="]:
                constructions.append((path, i))
                if path != STORAGE or not any(within(i, span) for span in constructors):
                    errors.append(f"{path}: unexpected SyncDelta construction")
        for i in sequences(items, ["Self", "{"]):
            if items[max(0, i - 2):i] == ["-", ">"]:
                continue
            if not any(within(i, span) for span in impls):
                continue
            end = matched[i + 1]
            if items[end + 1:end + 2] == ["="]:
                continue
            constructions.append((path, i))
            if path != STORAGE or not any(within(i, span) for span in constructors):
                errors.append(f"{path}: unexpected Self proof construction")
    for label, found in (("constructor definition", definitions), ("constructor call or constructor reference", calls),
                         ("proof construction", constructions), ("proof struct", structs)):
        if len(found) != 1:
            errors.append(f"expected exactly one {label}; found {len(found)}")
    if structs and structs[0][0] != STORAGE:
        errors.append("proof struct must be in storage/sync.rs")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    root = args.root / "crates/hoyadb-core/src"
    try:
        paths = sorted(root.rglob("*.rs"))
        sources = {path.relative_to(root).as_posix(): path.read_bytes().decode("utf-8") for path in paths}
        errors = audit_sources(sources)
        hashes = {name: hashlib.sha256(text.encode()).hexdigest() for name, text in sources.items()}
    except (OSError, UnicodeError) as error:
        errors, hashes = [f"source read failed: {error}"], {}
    print(json.dumps({"scope": "source-token convention; not Rust semantic proof",
                      "inputs": hashes, "errors": errors, "passed": not errors}, indent=2))
    return bool(errors)


if __name__ == "__main__":
    raise SystemExit(main())
