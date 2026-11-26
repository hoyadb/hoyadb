"""Minimal Java class-file reader (constant pool, this class, declared methods). Parses bytes only."""
from dataclasses import dataclass, field
import struct

_SIZES = {3: 4, 4: 4, 5: 8, 6: 8, 7: 2, 8: 2, 9: 4, 10: 4, 11: 4, 12: 4, 15: 3, 16: 2, 17: 4, 18: 4, 19: 2, 20: 2}


@dataclass
class ClassInfo:
    name: str
    methodrefs: set = field(default_factory=set)   # (owner, name, descriptor)
    class_refs: set = field(default_factory=set)   # internal names
    strings: set = field(default_factory=set)
    methods: list = field(default_factory=list)    # (name, descriptor) declared
    utf8: set = field(default_factory=set)         # every UTF-8 constant (descriptors included)


def _fail(reason):
    raise ValueError('malformed class file: ' + reason)


def parse(data):
    if len(data) < 10 or data[:4] != b'\xca\xfe\xba\xbe':
        _fail('magic')
    try:
        count = struct.unpack_from('>H', data, 8)[0]
        pool, offset, index = [None] * count, 10, 1
        while index < count:
            tag = data[offset]
            if tag == 1:
                length = struct.unpack_from('>H', data, offset + 1)[0]
                pool[index] = (1, data[offset + 3:offset + 3 + length].decode('utf-8', 'surrogateescape'))
                offset += 3 + length
            elif tag in _SIZES:
                size = _SIZES[tag]
                pool[index] = (tag, data[offset + 1:offset + 1 + size])
                offset += 1 + size
                if tag in (5, 6):
                    index += 1
            else:
                _fail('constant tag %d' % tag)
            index += 1

        def utf8(i):
            entry = pool[i]
            if entry is None or entry[0] != 1:
                _fail('utf8 index')
            return entry[1]

        def klass(i):
            entry = pool[i]
            if entry is None or entry[0] != 7:
                _fail('class index')
            return utf8(struct.unpack('>H', entry[1])[0])

        info = ClassInfo(name='')
        for entry in pool:
            if entry is None:
                continue
            tag, raw = entry
            if tag == 1:
                info.utf8.add(raw)
            elif tag == 7:
                info.class_refs.add(utf8(struct.unpack('>H', raw)[0]))
            elif tag == 8:
                info.strings.add(utf8(struct.unpack('>H', raw)[0]))
            elif tag in (10, 11):
                owner_index, nat_index = struct.unpack('>HH', raw)
                nat = pool[nat_index]
                if nat is None or nat[0] != 12:
                    _fail('name-and-type index')
                member, descriptor = struct.unpack('>HH', nat[1])
                info.methodrefs.add((klass(owner_index), utf8(member), utf8(descriptor)))
        _, this_index, _ = struct.unpack_from('>HHH', data, offset)
        info.name = klass(this_index)
        offset += 6
        interfaces = struct.unpack_from('>H', data, offset)[0]
        offset += 2 + 2 * interfaces
        for kind in ('fields', 'methods'):
            members = struct.unpack_from('>H', data, offset)[0]
            offset += 2
            for _ in range(members):
                _, name_index, descriptor_index, attributes = struct.unpack_from('>HHHH', data, offset)
                offset += 8
                for _ in range(attributes):
                    length = struct.unpack_from('>I', data, offset + 2)[0]
                    offset += 6 + length
                if kind == 'methods':
                    info.methods.append((utf8(name_index), utf8(descriptor_index)))
        if offset > len(data):
            _fail('truncated')
        return info
    except (struct.error, IndexError):
        _fail('truncated')
