"""SPEC-0047 / DEF-0056: enforce retained public API evidence."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[2]
HISTORY = Path("tools/release/baseline")
HISTORY_SHA256 = "5d6091bafb922aa55d630cb8289f03c95198eb6508307e6a8f3e81694adbd801"
CURRENT = Path("tools/release/current-api.json")
SWIFT_ACCEPTED = Path("tools/release/swift-accepted-changes.json")
HEADER = "crates/hoyadb-native/include/hoyadb.h"
MODULE_MAP = "crates/hoyadb-native/include/HoyaNative/module.modulemap"
CODEGEN = "packages/react-native/src/NativeHoyaDB.ts"
SOURCE_PATTERNS = [HEADER, MODULE_MAP, CODEGEN, "packages/client/*.d.ts",
                   "packages/react-native/index.d.ts", "packages/react-native/src/index.js",
                   "packages/react-native/package.json"]
SWIFT_PATTERNS = [HEADER, MODULE_MAP, str(SWIFT_ACCEPTED), "bindings/swift/Package.swift",
                  "bindings/swift/Sources/HoyaDB/**/*.swift",
                  "bindings/swift/Sources/HoyaDBTesting/**/*.swift", str(HISTORY / "*")]


class CompatibilityError(ValueError):
    pass


def digest(path):
    try:
        data = path.read_bytes()
    except OSError as error:
        raise CompatibilityError(f"Cannot read {path}: {error}") from error
    if not data:
        raise CompatibilityError(f"Empty evidence: {path}")
    return hashlib.sha256(data).hexdigest()


def read_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise CompatibilityError(f"Invalid JSON evidence {path}: {error}") from error


def capture(root, patterns):
    result = {}
    for pattern in patterns:
        paths = sorted(path for path in root.glob(pattern) if path.is_file())
        if not paths:
            raise CompatibilityError(f"No source inputs for {pattern}")
        for path in paths:
            result[path.relative_to(root).as_posix()] = digest(path)
    return dict(sorted(result.items()))


def check_inputs(expected, actual):
    if not expected or expected != actual:
        changes = sorted(key for key in expected.keys() | actual.keys()
                         if expected.get(key) != actual.get(key))
        raise CompatibilityError(f"Compatibility inputs changed: {', '.join(changes)}")


def history(root):
    path = root / HISTORY / "snapshot.json"
    if digest(path) != HISTORY_SHA256:
        raise CompatibilityError("Historical snapshot.json changed; do not rebaseline retained evidence")
    baseline = read_json(path)
    for module, value in baseline["swift"].items():
        dump = root / HISTORY / f"{module}.api.json"
        if digest(dump) != value["dump_sha256"]:
            raise CompatibilityError(f"Historical {module} API dump changed")
        check_swift_dump(dump, module)
    declaration = root / HISTORY / "react-native-index.d.ts"
    if digest(declaration) != baseline["react_native"]["declarations"]["sha256"]:
        raise CompatibilityError("Historical React Native declarations changed")
    return baseline


def check_sources(root):
    baseline = history(root)
    current = read_json(root / CURRENT)
    if (not isinstance(current, dict) or current.get("schema") != 1
            or not isinstance(current.get("review"), list) or not current["review"]
            or not all(isinstance(item, str) and item.strip() for item in current["review"])
            or not isinstance(current.get("inputs"), dict)
            or not all(isinstance(value, str) and re.fullmatch(r"[a-f0-9]{64}", value)
                       for value in current["inputs"].values())):
        raise CompatibilityError("Current API manifest requires reviewed, nonempty SHA256 inputs")
    actual = capture(root, SOURCE_PATTERNS)
    retained = {HEADER: baseline["c_header"]["sha256"],
                MODULE_MAP: baseline["c_header"]["module_map"]["sha256"],
                CODEGEN: baseline["react_native"]["turbo_module_spec"]["sha256"]}
    check_inputs(retained, {name: actual[name] for name in retained})
    check_inputs(current["inputs"], actual)
    return {"scope": "reviewed source drift; not runtime or semantic API certification",
            "inputs": actual, "manifest_sha256": digest(root / CURRENT)}


def run(command, commands=None):
    try:
        result = subprocess.run([str(arg) for arg in command], capture_output=True,
                                text=True, encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise CompatibilityError(f"Cannot run {command[0]}: {error}") from error
    if commands is not None:
        commands.append({"argv": [str(arg) for arg in command], "status": result.returncode,
                         "stdout": result.stdout, "stderr": result.stderr})
    if result.returncode:
        raise CompatibilityError(f"Tool failed ({result.returncode}): {' '.join(map(str, command))}\n"
                                 + (result.stdout + result.stderr)[-6000:])
    return result


def check_swift_result(result, accepted=()):
    """The API digester can exit zero while reporting an API breakage."""
    diagnostics = result.stdout + result.stderr
    if result.returncode:
        raise CompatibilityError(diagnostics)
    for diagnostic in accepted:
        if diagnostic not in diagnostics.splitlines():
            raise CompatibilityError(f"Reviewed Swift change not observed: {diagnostic}")
    diagnostics = "\n".join(line for line in diagnostics.splitlines() if line not in accepted)
    if "API breakage:" in diagnostics:
        raise CompatibilityError(diagnostics)
    remainder = re.sub(r"/\*.*?\*/", "", diagnostics, flags=re.DOTALL).strip()
    if "/* Removed Decls */" not in diagnostics or remainder:
        raise CompatibilityError(f"Empty or unrecognized Swift diagnostic evidence: {diagnostics}")


def check_swift_dump(path, module):
    data = read_json(path)
    root = data.get("ABIRoot") if isinstance(data, dict) else None
    children = root.get("children") if isinstance(root, dict) else None
    if (not isinstance(root, dict) or root.get("kind") != "Root" or root.get("name") != module
            or not isinstance(children, list) or not any(
                isinstance(child, dict) and child.get("kind") in ("TypeDecl", "Function", "Var")
                for child in children)):
        raise CompatibilityError(f"Missing public declarations for {module} in {path}")


def check_swift(root, output):
    """Compile sources into a new directory; never consume cached package modules."""
    history(root)
    before = capture(root, SWIFT_PATTERNS)
    accepted = read_json(root / SWIFT_ACCEPTED)
    if (not isinstance(accepted, dict) or set(accepted) != {"HoyaDB", "HoyaDBTesting"}
            or not all(isinstance(entries, list) for entries in accepted.values())
            or not all(isinstance(entry, dict)
                       and isinstance(entry.get("diagnostic"), str)
                       and entry["diagnostic"].startswith("API breakage:")
                       and "\n" not in entry["diagnostic"]
                       and isinstance(entry.get("review"), str) and entry["review"].strip()
                       for entries in accepted.values() for entry in entries)):
        raise CompatibilityError("Invalid exact Swift change review")
    output.mkdir(parents=True, exist_ok=False)
    commands = []
    report = {"scope": "fresh macOS Swift source API; not binary ABI/runtime certification",
              "inputs": before, "commands": commands, "accepted_changes": accepted, "passed": False}
    try:
        sdk = run(["xcrun", "--sdk", "macosx", "--show-sdk-path"], commands).stdout.strip()
        if not Path(sdk).is_dir():
            raise CompatibilityError(f"Invalid macOS SDK: {sdk!r}")
        run(["xcrun", "swiftc", "--version"], commands)
        target = "x86_64-apple-macosx13.0"
        include = root / Path(MODULE_MAP).parent
        common = ["-I", output, "-I", include, "-sdk", sdk, "-target", target,
                  "-module-cache-path", output / "cache"]
        failures = []
        for module in ("HoyaDB", "HoyaDBTesting"):
            sources = sorted((root / "bindings/swift/Sources" / module).rglob("*.swift"))
            run(["xcrun", "swiftc", "-emit-module", "-parse-as-library", "-module-name", module,
                 "-swift-version", "6", *common, "-emit-module-path", output / f"{module}.swiftmodule",
                 *sources], commands)
            current = output / f"{module}.api.json"
            run(["xcrun", "swift-api-digester", "-dump-sdk", "-abort-on-module-fail",
                 "-module", module, *common, "-o", current], commands)
            check_swift_dump(current, module)
            result = run(["xcrun", "swift-api-digester", "-diagnose-sdk", "-compiler-style-diags",
                          "-input-paths", root / HISTORY / f"{module}.api.json",
                          "-input-paths", current], commands)
            try:
                check_swift_result(result, [entry["diagnostic"] for entry in accepted[module]])
            except CompatibilityError as error:
                failures.append(f"{module}: {error}")
        check_inputs(before, capture(root, SWIFT_PATTERNS))
        report["dumps"] = {path.name: digest(path) for path in output.glob("*.api.json")}
        if failures:
            raise CompatibilityError("\n".join(failures))
        report["passed"] = True
    except CompatibilityError as error:
        report["error"] = str(error)
        raise
    finally:
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return report


def check_exports(output, expected, kind):
    symbols = set()
    for line in output.splitlines():
        if not line.strip():
            continue
        if kind == "apple" and line.rstrip().endswith(":"):
            continue  # archive member headings emitted by nm
        match = re.fullmatch(r"\s*(?:[0-9a-fA-F]+|-+)\s+([A-Za-z])\s+(\S+)\s*", line)
        if not match:
            raise CompatibilityError(f"Malformed nm export evidence: {line}")
        symbol = match.group(2)
        if match.group(1) == "U":
            raise CompatibilityError(f"Undefined symbol is not an export: {symbol}")
        if kind == "android" and symbol != "HOYADB_1" and not symbol.endswith("@@HOYADB_1"):
            raise CompatibilityError(f"Changed Android default symbol version: {symbol}")
        symbols.add(symbol.split("@")[0])
    if kind == "apple":
        # Static Rust archives have internal globals; only the public C/JNI boundary is frozen.
        symbols = {symbol for symbol in symbols if symbol.startswith(("_hoya_", "_Java_"))}
    else:
        symbols.discard("HOYADB_1")
    if not expected or symbols != expected:
        raise CompatibilityError(f"Export mismatch: missing={sorted(expected - symbols)}, "
                                 f"unexpected={sorted(symbols - expected)}")


def check_library(root, library, nm, kind, slice_name):
    baseline = history(root)
    if kind == "apple":
        expected = set(baseline["apple_static_exports"].get(slice_name, []))
        flags = ["-gU"]
    else:
        expected = {symbol.split("@")[0] for symbol in
                    baseline["android_jni"].get(slice_name, {}).get("exports", [])}
        flags = ["--dynamic", "--defined-only", "--extern-only"]
    if not expected:
        raise CompatibilityError(f"Unknown retained {kind} slice: {slice_name}")
    tool = shutil.which(nm)
    if tool is None:
        raise CompatibilityError(f"Cannot find nm tool: {nm}")
    before = {"library": digest(library), "nm": digest(Path(tool))}
    commands = []
    result = run([tool, *flags, library], commands)
    check_exports(result.stdout, expected, kind)
    check_inputs(before, {"library": digest(library), "nm": digest(Path(tool))})
    return {"scope": "supplied artifact exports; not source provenance or runtime certification",
            "library": str(library), "sha256": before["library"], "nm_sha256": before["nm"],
            "kind": kind, "slice": slice_name, "symbols": sorted(expected),
            "commands": [{"argv": item["argv"], "status": item["status"],
                          "stdout_sha256": hashlib.sha256(item["stdout"].encode()).hexdigest(),
                          "stderr": item["stderr"]} for item in commands]}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    actions = parser.add_subparsers(dest="action", required=True)
    actions.add_parser("sources", help="compare source hashes to explicitly reviewed current surface")
    candidate = actions.add_parser("candidate", help="write an unreviewed source candidate; never approve it")
    candidate.add_argument("--output", type=Path, required=True)
    swift = actions.add_parser("swift", help="compile fresh modules and compare retained Swift API")
    swift.add_argument("--output", type=Path, required=True, help="new evidence directory (must not exist)")
    exports = actions.add_parser("exports", help="inspect a supplied native artifact")
    exports.add_argument("--kind", choices=("apple", "android"), required=True)
    exports.add_argument("--slice", required=True)
    exports.add_argument("--library", type=Path, required=True)
    exports.add_argument("--nm", default="nm")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        if args.action == "candidate":
            # No approval, no overwrite, and never write into retained historical evidence.
            output = args.output.resolve()
            if output == root / CURRENT or (root / HISTORY) in output.parents:
                raise CompatibilityError("Write candidates outside retained/reviewed baselines")
            report = {"schema": 1, "review": [], "inputs": capture(root, SOURCE_PATTERNS)}
            with output.open("x", encoding="utf-8") as handle:
                handle.write(json.dumps(report, indent=2) + "\n")
            print(f"Unreviewed candidate: {output}; this is not a compatibility pass")
            return 0
        if args.action == "sources":
            report = check_sources(root)
        elif args.action == "swift":
            report = check_swift(root, args.output.resolve())
        else:
            report = check_library(root, args.library.resolve(), args.nm, args.kind, args.slice)
        print(json.dumps(report, indent=2))
        return 0
    except (CompatibilityError, OSError) as error:
        print(f"Compatibility check failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
