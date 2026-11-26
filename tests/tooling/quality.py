"""SPEC-0044 / TC-0048: validate coverage and cargo-mutants evidence.

Only native workspace Rust is measured here. A missing branch metric is unknown,
and files without LLVM records remain visible instead of shrinking the inventory.
"""

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib

COMPONENTS = ("hoyadb-core", "hoyadb-storage-memory", "hoyadb-storage-sqlite", "hoyadb-bindings")
MUTANTS_VERSION = "27.1.0"
ROOT = Path(__file__).resolve().parents[2]


def capture_measurement(kind, command):
    tools = {"coverage": ("llvm-cov", "cargo-llvm-cov 0.9.1"),
             "mutations": ("mutants", "cargo-mutants " + MUTANTS_VERSION)}
    tool, expected = tools[kind]
    if command[:2] != ["cargo", tool]:
        raise ValueError("unexpected measurement command")
    options = command[:command.index("--")] if "--" in command else command
    compiler = subprocess.check_output(["rustc", "-vV"], cwd=ROOT, text=True).strip()
    version = subprocess.check_output(["cargo", tool, "--version"], cwd=ROOT, text=True).strip()
    if version != expected:
        raise ValueError("wrong measurement tool version")
    host = next(line.removeprefix("host: ") for line in compiler.splitlines() if line.startswith("host: "))

    def option(name, default):
        values = [arg.split("=", 1)[1] for arg in options if arg.startswith(name + "=")]
        values += [options[i + 1] for i, arg in enumerate(options[:-1]) if arg == name]
        if len(values) > 1:
            raise ValueError("ambiguous measurement option: " + name)
        return values[0] if values else default

    # Artifact locations are not an instrumentation difference. All other flags,
    # including features, package selections and test filters, stay comparable.
    normalized = []
    skip = False
    for arg in command:
        if skip:
            skip = False
        elif arg in ("--output-path", "--output"):
            skip = True
        elif not arg.startswith(("--output-path=", "--output=")):
            normalized.append(arg)
    environment = {name: value for name, value in os.environ.items()
                   if name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET",
                               "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                               "LLVM_COV", "LLVM_PROFDATA")
                   or name.startswith("CARGO_PROFILE_")}
    comparison = {"kind": kind, "compiler": compiler, "tool": version,
                  "target": option("--target", os.environ.get("CARGO_BUILD_TARGET", host)),
                  "profile": option("--profile", "release" if {"--release", "-r"}.intersection(options) else "dev"),
                  "command": normalized, "environment": environment}
    return {"command": command, "comparison": comparison,
            "captured_at": datetime.now(timezone.utc).isoformat()}


def workspace_members(root):
    members = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["members"]
    if (len(members) != len(COMPONENTS)
            or set(members) != {"crates/" + name for name in COMPONENTS}):
        raise ValueError("workspace measurement inventory needs updating")
    return [root / member for member in members]


def source_snapshot(root):
    paths = {root / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
                                     ".cargo/mutants.toml")}
    for member in workspace_members(root):
        paths.add(member / "Cargo.toml")
        paths.update(member.glob("*.rs"))  # Includes a package build.rs when present.
        for directory in ("src", "tests", "examples", "benches"):
            paths.update((member / directory).rglob("*.rs"))
    paths.update((root / "tests/conformance").rglob("*.rs"))
    return {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(paths) if path.is_file()}


def count(value):
    if type(value) is not int or value < 0:
        raise ValueError("invalid nonnegative count")
    return value


def metric(summary, name):
    item = summary[name]
    covered, total = count(item["covered"]), count(item["count"])
    if covered > total:
        raise ValueError(f"impossible {name} coverage")
    return {"covered": covered, "count": total}


def coverage_summary(report, root, snapshot):
    if source_snapshot(root) != snapshot:
        raise ValueError("source changed since measurement started")
    if report["type"] != "llvm.coverage.json.export" or not report["data"]:
        raise ValueError("not an LLVM coverage export")
    components = {}
    seen = set()
    for data in report["data"]:
        for item in data["files"]:
            path = Path(item["filename"])
            path = path if path.is_absolute() else root / path
            try:
                relative = path.resolve().relative_to(root.resolve())
            except ValueError:
                continue  # Third-party compiler/dependency source is outside this scope.
            parts = relative.parts
            if len(parts) < 4 or parts[0] != "crates" or parts[2] != "src":
                continue  # Tests and examples are inventoried, not product coverage.
            if parts[1] not in COMPONENTS:
                raise ValueError(f"unknown workspace component: {parts[1]}")
            name = str(relative)
            if name in seen:
                raise ValueError(f"duplicate coverage file: {name}")
            if name not in snapshot:
                raise ValueError(f"coverage file absent from source snapshot: {name}")
            seen.add(name)
            component = components.setdefault(parts[1], {
                **{key: {"count": 0, "covered": 0} for key in ("lines", "regions", "functions")},
                "branches": None, "uncovered": [], "files": 0})
            component["files"] += 1
            for key in ("lines", "regions", "functions"):
                values = metric(item["summary"], key)
                for field, value in values.items():
                    component[key][field] += value
            # Zero branch records on stable Rust mean no instrumentation, not full coverage.
            if "branches" in item["summary"]:
                values = metric(item["summary"], "branches")
                if values["count"]:
                    if component["branches"] is None:
                        component["branches"] = {"count": 0, "covered": 0}
                    for field, value in values.items():
                        component["branches"][field] += value
            missing = sorted({segment[0] for segment in item.get("segments", [])
                              if segment[3] and segment[2] == 0 and segment[4]})
            if missing or item["summary"]["lines"]["covered"] < item["summary"]["lines"]["count"]:
                component["uncovered"].append({"file": name, "region_start_lines": missing})
    if any(component["lines"]["count"] == 0 for component in components.values()):
        raise ValueError("component has no line instrumentation")
    production = {p for p in snapshot
                  if any(p.startswith(f"crates/{name}/src/") for name in COMPONENTS)
                  and p.endswith(".rs")}
    # The memory crate really is one re-export, not an omitted implementation.
    # A new function/module invalidates this exception until it is measured.
    alias = "crates/hoyadb-storage-memory/src/lib.rs"
    if "hoyadb-storage-memory" not in components:
        sources = {p for p in production if p.startswith("crates/hoyadb-storage-memory/")}
        source = "\n".join(line for line in (root / alias).read_text().splitlines()
                           if not line.strip().startswith("//")).strip()
        if sources == {alias} and source == "pub use hoyadb_core::storage::SnapshotStorage as MemoryStorage;":
            components["hoyadb-storage-memory"] = {
                "implementation_in": "hoyadb-core", "lines": None, "regions": None,
                "functions": None, "branches": None, "files": 0, "uncovered": []}
            seen.add(alias)
    if set(components) != set(COMPONENTS):
        raise ValueError("missing coverage component")
    return {"scope": "native Rust workspace source files including inline unit tests; branches unknown unless instrumented",
            "components": components, "source_sha256": snapshot,
            "uninstrumented_sources": sorted(production - seen)}


def mutation_summary(report, selected_count=None, selected_names=None):
    if report["cargo_mutants_version"] != MUTANTS_VERSION or not report["end_time"]:
        raise ValueError("wrong tool version or incomplete mutation run")
    baseline = [o for o in report["outcomes"] if o["scenario"] == "Baseline"]
    if len(baseline) != 1 or baseline[0]["summary"] != "Success":
        raise ValueError("missing or failed mutation baseline")
    names = {"CaughtMutant": "caught", "MissedMutant": "missed", "Timeout": "timeout",
             "Unviable": "unviable", "Success": "success"}
    counts = Counter()
    seen = set()
    for outcome in report["outcomes"]:
        if outcome["scenario"] == "Baseline":
            continue
        if not isinstance(outcome["scenario"], dict) or set(outcome["scenario"]) != {"Mutant"}:
            raise ValueError("unknown mutation scenario")
        if outcome["summary"] not in names:
            raise ValueError("unclassified mutation result")
        name = outcome["scenario"]["Mutant"]["name"]
        if not isinstance(name, str) or name in seen:
            raise ValueError("invalid or duplicate mutant outcome")
        seen.add(name)
        counts[names[outcome["summary"]]] += 1
    if any(count(report[key]) != counts[key] for key in names.values()):
        raise ValueError("mutation counts disagree with outcomes")
    if count(report["total_mutants"]) != sum(counts.values()):
        raise ValueError("incomplete mutation outcomes")
    if selected_count is not None and count(selected_count) != report["total_mutants"]:
        raise ValueError("not all selected mutants completed")
    if selected_names is not None and selected_names != seen:
        raise ValueError("outcomes do not match selected mutants")
    if not counts["caught"] or counts["missed"] or counts["timeout"] or counts["success"]:
        raise ValueError(f"mutation gate failed: {dict(counts)}")
    return {key: counts[key] for key in names.values()} | {
        "scope": "selected native Rust mutants; not a whole-project score",
        "total_mutants": report["total_mutants"],
        "score": 100 * counts["caught"] / (counts["caught"] + counts["missed"]),
        "score_definition": "caught / (caught + missed); timeouts fail separately"}


def check_floors(current, baseline):
    for report in (current, baseline):
        block = report.get("measurement") if isinstance(report, dict) else None
        identity = block.get("comparison") if isinstance(block, dict) else None
        fields = ("kind", "compiler", "tool", "target", "profile")
        if (not isinstance(identity, dict)
                or any(not isinstance(identity.get(key), str) or not identity[key] for key in fields)
                or identity["kind"] != "coverage"
                or not isinstance(identity.get("command"), list)
                or identity["command"][:2] != ["cargo", "llvm-cov"]
                or not all(isinstance(arg, str) for arg in identity["command"])
                or not isinstance(identity.get("environment"), dict)
                or not all(isinstance(key, str) and isinstance(value, str)
                           for key, value in identity["environment"].items())):
            raise ValueError("coverage measurements are not comparable: incomplete identity")
    identity = current.get("measurement", {}).get("comparison")
    if not identity or identity != baseline.get("measurement", {}).get("comparison"):
        raise ValueError("coverage measurements are not comparable")
    if current.get("scope") != baseline.get("scope"):
        raise ValueError("coverage scopes are not comparable")
    if set(current["components"]) != set(baseline["components"]):
        raise ValueError("coverage floor component inventory changed")
    for name, previous in baseline["components"].items():
        for key in ("lines", "branches"):
            if previous[key] is None:
                continue  # Unknown instrumentation establishes no numerical floor.
            old = metric(previous, key)
            new = current["components"][name][key]
            if new is None:
                raise ValueError(f"lost {key} instrumentation for {name}")
            new = metric(current["components"][name], key)
            if not old["count"] or not new["count"]:
                raise ValueError("empty coverage floor")
            if new["covered"] * old["count"] < old["covered"] * new["count"]:
                raise ValueError(f"coverage floor regressed: {name} {key}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("snapshot", "metadata", "coverage", "mutations"))
    parser.add_argument("report", type=Path)
    parser.add_argument("--snapshot", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--measurement", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--tool-exit", type=int, default=0)
    parser.add_argument("--kind", choices=("coverage", "mutations"))
    parser.add_argument("--command", nargs=argparse.REMAINDER)
    arguments = sys.argv[1:]
    command = None
    if "--command" in arguments:
        boundary = arguments.index("--command")
        command = arguments[boundary + 1:]
        arguments = arguments[:boundary]
    args = parser.parse_args(arguments)
    args.command = command
    if args.action == "snapshot":
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(source_snapshot(ROOT), indent=2) + "\n")
        return
    if args.action == "metadata":
        args.report.write_text(json.dumps(capture_measurement(args.kind, args.command), indent=2) + "\n")
        return
    result = {"gate_passed": False, "tool_exit": args.tool_exit}
    report = {}
    try:
        if args.snapshot is not None:
            result["source_sha256"] = json.loads(args.snapshot.read_text())
        if args.measurement is not None:
            result["measurement"] = json.loads(args.measurement.read_text())
        loaded = json.loads(args.report.read_text())
        if not isinstance(loaded, dict):
            raise ValueError("report must be a JSON object")
        report = loaded
        validate(args, report, result, parser)
        result["revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        result["dirty"] = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True))
        if args.tool_exit:
            raise ValueError(f"measurement tool failed with exit {args.tool_exit}")
        result["gate_passed"] = True
    except (ValueError, KeyError, TypeError, IndexError, OSError, subprocess.CalledProcessError) as error:
        result["error"] = str(error)
        if args.action == "mutations":
            result["raw_counts"] = {key: report.get(key) for key in
                                    ("caught", "missed", "timeout", "unviable", "success", "total_mutants")}
        if args.output is not None:
            args.output.write_text(json.dumps(result, indent=2) + "\n")
        raise
    encoded = json.dumps(result, indent=2) + "\n"
    if args.output is not None:
        args.output.write_text(encoded)
    else:
        print(encoded, end="")


def validate(args, report, result, parser):
    if args.action == "coverage":
        if args.snapshot is None:
            parser.error("coverage requires --snapshot captured before running tests")
        result.update(coverage_summary(report, ROOT, json.loads(args.snapshot.read_text())))
        result["floor_status"] = "not_requested"
        if args.baseline is not None:
            if not args.baseline.is_file():
                result["floor_status"] = "not_established"
                raise ValueError(f"coverage floor not established: missing baseline {args.baseline}")
            result["floor_status"] = "failed"
            check_floors(result, json.loads(args.baseline.read_text()))
            result["floor_status"] = "passed"
    else:
        selected = json.loads(args.report.with_name("mutants.json").read_text())
        if not isinstance(selected, list):
            raise ValueError("invalid selected mutant inventory")
        if args.snapshot is None or source_snapshot(ROOT) != json.loads(args.snapshot.read_text()):
            raise ValueError("missing snapshot or source changed since mutation run started")
        result.update(mutation_summary(report, selected_count=len(selected),
                                       selected_names={item["name"] for item in selected}))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, TypeError, IndexError, OSError, subprocess.CalledProcessError) as error:
        print(f"quality evidence rejected: {error}", file=sys.stderr)
        sys.exit(1)
