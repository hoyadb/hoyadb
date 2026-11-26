"""Rust source closure of the Android native archive, as bindings/swift/build-artifacts.py binds it.

Shared by build-jni.py (recorded when the JNI library is linked) and
bindings/android/tools/package-aar.py (compared before a package is certified).
Additions and removals count as drift: the result is the complete current file set.
"""
import hashlib
from pathlib import Path

CRATES = ['hoyadb-core', 'hoyadb-storage-memory', 'hoyadb-storage-sqlite', 'hoyadb-bindings', 'hoyadb-native']


def rust_source_paths(root):
    root = Path(root)
    paths = {root / 'Cargo.toml', root / 'Cargo.lock', root / 'rust-toolchain.toml', root / 'crates/hoyadb-native/Cargo.lock'}
    for name in CRATES:
        crate = root / 'crates' / name
        paths.add(crate / 'Cargo.toml')
        paths.add(crate / 'build.rs')
        paths.update(p for p in (crate / 'src').rglob('*') if p.is_file() or p.is_symlink())
        # Only build configuration, never Cargo credentials or generated files.
        for directory in (root, root / 'crates', crate):
            paths.update((directory / '.cargo/config', directory / '.cargo/config.toml'))
    paths.update(p for p in (root / 'crates/hoyadb-native/include').rglob('*') if p.is_file() or p.is_symlink())
    return sorted(p for p in paths if p.is_file() or p.is_symlink())


def rust_sources(root):
    """Relative path -> SHA256 for every bound file; a symlinked source is never accepted."""
    root = Path(root)
    result = {}
    for path in rust_source_paths(root):
        if path.is_symlink():
            raise SystemExit('native Rust source closure mismatch')
        result[path.relative_to(root).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    return result
