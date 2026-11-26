"""Reader-format property parsing for SPEC-0034 / TC-0038."""

# Written from the public API contract, not a Java implementation:
# https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/util/Properties.html#load(java.io.Reader)


class MalformedProperties(Exception):
    """A Unicode escape is incomplete or contains a non-hexadecimal digit."""


def _logical_lines(text: str):
    pieces = []
    continuing = False
    # splitlines() would also recognize characters the Properties format retains.
    for natural in text.replace('\r\n', '\n').replace('\r', '\n').split('\n'):
        natural = natural.lstrip(' \t\f')
        if not continuing and (not natural or natural[0] in '#!'):
            continue
        trailing = len(natural) - len(natural.rstrip('\\'))
        continuing = trailing % 2 == 1
        pieces.append(natural[:-1] if continuing else natural)
        if not continuing:
            logical = ''.join(pieces)
            if logical:
                yield logical
            pieces.clear()
    if pieces:
        logical = ''.join(pieces)
        if logical:
            yield logical


def _decode(raw: str) -> str:
    result = []
    position = 0
    controls = {'t': '\t', 'n': '\n', 'r': '\r', 'f': '\f'}
    while position < len(raw):
        character = raw[position]
        position += 1
        if character != '\\':
            result.append(character)
            continue
        if position == len(raw):
            break
        character = raw[position]
        position += 1
        if character == 'u':
            digits = raw[position:position + 4]
            if len(digits) != 4 or any(digit not in '0123456789abcdefABCDEF' for digit in digits):
                raise MalformedProperties('Unicode escape requires four hexadecimal digits')
            result.append(chr(int(digits, 16)))
            position += 4
        else:
            result.append(controls.get(character, character))
    return ''.join(result)


def load_reader(text: str) -> dict[str, str]:
    """Parse an already decoded Properties Reader character stream."""
    properties = {}
    for line in _logical_lines(text):
        boundary = 0
        while boundary < len(line):
            character = line[boundary]
            if character == '\\':
                boundary += 2
            elif character in '=:\t\f ':
                break
            else:
                boundary += 1
        value_start = boundary
        while value_start < len(line) and line[value_start] in ' \t\f':
            value_start += 1
        if value_start < len(line) and line[value_start] in '=:':
            value_start += 1
        while value_start < len(line) and line[value_start] in ' \t\f':
            value_start += 1
        properties[_decode(line[:boundary])] = _decode(line[value_start:])
    return properties
