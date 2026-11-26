"""Synthetic class-file/JAR bytes for tests. Never loaded by a JVM; no real dependency bytes."""
import struct
import zipfile


def class_bytes(name, methods=(), invokes=(), strings=(), class_refs=()):
    """Minimal valid Java class file: constant pool, this/super, declared methods (no code)."""
    pool, index = [], {}

    def add(key, raw):
        if key not in index:
            pool.append(raw)
            index[key] = len(pool)
        return index[key]

    def utf8(text):
        data = text.encode('utf-8')
        return add(('u', text), b'\x01' + struct.pack('>H', len(data)) + data)

    def klass(internal):
        return add(('c', internal), b'\x07' + struct.pack('>H', utf8(internal)))

    def nat(member, descriptor):
        return add(('n', member, descriptor), b'\x0c' + struct.pack('>HH', utf8(member), utf8(descriptor)))

    this = klass(name)
    sup = klass('java/lang/Object')
    for owner, member, descriptor in invokes:
        add(('m', owner, member, descriptor), b'\x0a' + struct.pack('>HH', klass(owner), nat(member, descriptor)))
    for text in strings:
        add(('s', text), b'\x08' + struct.pack('>H', utf8(text)))
    for internal in class_refs:
        klass(internal)
    method_info = b''
    for member, descriptor in methods:
        method_info += struct.pack('>HHHH', 0x0009, utf8(member), utf8(descriptor), 0)
    out = b'\xca\xfe\xba\xbe' + struct.pack('>HHH', 0, 52, len(pool) + 1) + b''.join(pool)
    out += struct.pack('>HHHHH', 0x0021, this, sup, 0, 0) + struct.pack('>H', len(methods)) + method_info + struct.pack('>H', 0)
    return out


def jar(path, entries):
    """entries: {name: bytes}. Deterministic synthetic archive."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, 'w') as archive:
        for name, data in sorted(entries.items()):
            archive.writestr(zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0)), data)
    return path


def pom_properties(group, artifact, version):
    return f'#synthetic\ngroupId={group}\nartifactId={artifact}\nversion={version}\n'.encode()
