#!/usr/bin/env python3
"""Build only the approved native SQLCipher/LTC sources with API24 OS entropy.

No dependency resolution/download/install, hooks, system crypto or source fallback.
Use a task-owned output directory; runtime support requires separate device tests.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tomllib


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command, log):
    log.write(json.dumps([str(part) for part in command]) + "\n")
    log.flush()
    subprocess.run([str(part) for part in command], check=True, stdout=log, stderr=log)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ndk", required=True, type=Path)
    targets = {"x86_64": "x86_64-linux-android", "arm64-v8a": "aarch64-linux-android",
               "armeabi-v7a": "armv7a-linux-androideabi", "x86": "i686-linux-android"}
    parser.add_argument("--abi", required=True, choices=targets)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    manifest = here.parent / "Cargo.toml"
    meta = json.loads(subprocess.check_output([
        "cargo", "metadata", "--manifest-path", str(manifest), "--offline", "--locked",
        "--format-version", "1",
    ]))
    versions = [p for p in meta["packages"] if p["name"] == "sqlcipher-src"]
    assert len(versions) == 1
    package = versions[0]
    expected = "419.0.6+sqlcipher-4.19.0-sqlite-3.53.4-libtomcrypt-1.18.2"
    assert package["version"] == expected
    source = Path(package["manifest_path"]).parent
    locked = tomllib.loads((here.parent / "Cargo.lock").read_text())
    lock = next(p for p in locked["package"] if p["name"] == "sqlcipher-src")
    archive = source.parents[2] / "cache" / source.parent.name / (source.name + ".crate")
    assert sha(archive) == lock["checksum"]
    # Cargo's cache may omit the old per-file checksum marker. Verify source
    # bytes directly against its exact locked archive, without unpacking it.
    with tarfile.open(archive, "r:gz") as package_archive:
        for member in package_archive.getmembers():
            if member.isfile():
                relative = Path(member.name).relative_to(source.name)
                assert ".." not in relative.parts
                content = package_archive.extractfile(member).read()
                assert sha(source / relative) == hashlib.sha256(content).hexdigest(), str(relative)
    sources = re.findall(r'"(libtomcrypt/[^"\n]+\.c)"', (source / "src/libtomcrypt_sources.rs").read_text())
    excluded = "libtomcrypt/prngs/rng_get_bytes.c"
    assert sources.count(excluded) == 1 and len(sources) == len(set(sources))
    sources.remove(excluded)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    for directory in ["obj", "lib", "include", "lib/pkgconfig"]:
        (output / directory).mkdir(parents=True, exist_ok=True)
    host = "darwin-x86_64" if sys.platform == "darwin" else "linux-x86_64"
    toolchain = args.ndk.resolve() / "toolchains/llvm/prebuilt" / host
    triple = targets[args.abi]
    clang = toolchain / "bin/clang"
    flags = [
        f"--target={triple}24", f"--sysroot={toolchain / 'sysroot'}", "-O2", "-fPIC", "-pthread",
        "-fstack-protector-strong", "-D_FORTIFY_SOURCE=2", "-include", str(here / "ltc_config.h"),
        "-I", str(source / "sqlcipher/libtomcrypt/headers"),
    ]
    definitions = [
        "SQLITE_CORE", "SQLITE_HAS_CODEC", "SQLCIPHER_CRYPTO_LIBTOMCRYPT", "SQLITE_THREADSAFE=1",
        "SQLITE_TEMP_STORE=3", "SQLITE_DEFAULT_FOREIGN_KEYS=1", "SQLITE_ENABLE_API_ARMOR",
        "SQLITE_ENABLE_COLUMN_METADATA", "SQLITE_ENABLE_DBSTAT_VTAB", "SQLITE_ENABLE_FTS3",
        "SQLITE_ENABLE_FTS3_PARENTHESIS", "SQLITE_ENABLE_FTS5", "SQLITE_ENABLE_JSON1",
        "SQLITE_ENABLE_LOAD_EXTENSION=1", "SQLITE_ENABLE_RTREE", "SQLITE_ENABLE_STAT4",
        "SQLITE_SOUNDEX", "SQLITE_USE_URI", "HAVE_USLEEP=1", "HAVE_ISNAN",
        "_POSIX_THREAD_SAFE_FUNCTIONS", "HAVE_STDINT_H=1",
        "SQLITE_EXTRA_INIT=sqlcipher_extra_init", "SQLITE_EXTRA_SHUTDOWN=sqlcipher_extra_shutdown",
        "SQLCIPHER_OMIT_LOG=1", "SQLCIPHER_OMIT_LOG_DEVICE=1", "SQLCIPHER_OMIT_DEFAULT_LOGGING=1",
    ]
    objects = []
    source_map = {}
    with (output / "commands.txt").open("w") as log:
        units = [(source / "sqlcipher/sqlcipher.c", "sqlcipher", False)]
        units += [(source / "sqlcipher" / name, f"ltc-{index:03d}", True) for index, name in enumerate(sources)]
        units += [(here / "hoya_entropy.c", "hoya-entropy", False)]
        for path, name, ltc in units:
            obj = output / "obj" / (name + ".o")
            run([clang, *flags, *["-D" + d for d in definitions], *(["-DLTC_SOURCE"] if ltc else []),
                 "-c", path, "-o", obj], log)
            objects.append(obj)
            source_map[str(path.relative_to(source)) if path.is_relative_to(source) else str(path.relative_to(here))] = sha(path)
        run([toolchain / "bin/llvm-ar", "rcs", output / "lib/libsqlcipher.a", *objects], log)
        run([toolchain / "bin/llvm-readelf", "--file-header", output / "obj/sqlcipher.o"], log)
    (output / "include/sqlite3.h").write_bytes((source / "sqlcipher/sqlite3.h").read_bytes())
    (output / "lib/pkgconfig/sqlcipher.pc").write_text(
        f"prefix={output}\nlibdir=${{prefix}}/lib\nincludedir=${{prefix}}/include\n"
        "Name: SQLCipher\nDescription: approved SQLCipher4.19.0 with native LibTomCrypt1.18.2\n"
        "Version: 4.19.0\nLibs: -L${libdir} -lsqlcipher\nLibs.private: -ldl -lm\nCflags: -I${includedir}\n")
    evidence = {
        "runtime_verified": False, "abi": args.abi, "min_sdk": 24, "triple": triple,
        "package": expected, "package_sha256": lock["checksum"], "excluded": excluded,
        "ltc_source_count_after_exclusion": len(sources), "definitions": definitions,
        "ndk_source_properties": (args.ndk / "source.properties").read_text(),
        "sources_sha256": source_map,
        "ltc_config_sha256": sha(here / "ltc_config.h"),
        "builder_sha256": sha(Path(__file__)),
        "archive_sha256": sha(output / "lib/libsqlcipher.a"),
        "header_sha256": sha(output / "include/sqlite3.h"),
    }
    (output / "evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")
    print(f"PROVIDER_ARCHIVE_BUILT abi={args.abi} units={len(objects)} runtime_verified=false")


if __name__ == "__main__":
    main()
