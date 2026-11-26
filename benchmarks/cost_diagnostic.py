#!/usr/bin/env python3
"""Bounded warm-cohort cost diagnostic: contaminated stage wall times, never samples or budgets.

The separate `cost-diagnostic` binary uses the measured worker's library harness. This
parent bounds its runtime, reaps it, and records observed stage wall times plus a labelled
arithmetic projection. Nothing here enters a latency summary or numerical budget.
"""
import argparse
import os
from pathlib import Path
import subprocess
import sys
import time
from run import (RunnerError, SOURCE_PATHS, canonical, environment, file_hash, require,
                 source_snapshot, strict_json, write_new)


def counter(value):
    return type(value) is int and value >= 0


def parse(lines, samples):
    expected = [("setup", None), ("fixture_validation", None)] + [("sample", index) for index in range(samples)]
    records = []
    for raw in lines:
        require(isinstance(raw, bytes) and raw.startswith(b"DIAGNOSTIC ") and raw.endswith(b"\n"), "unexpected diagnostic output")
        value = strict_json(raw[len(b"DIAGNOSTIC "):-1])
        require(isinstance(value, dict) and set(value) == {"stage", "wall_ns", "index", "timed_value_ns"} and counter(value["wall_ns"]), "invalid diagnostic record")
        records.append(value)
    require([(value["stage"], value["index"]) for value in records] == expected, "incomplete or reordered diagnostic stages")
    later = records[2:]
    require(all(counter(value["timed_value_ns"]) for value in later) and all(value["timed_value_ns"] is None for value in records[:2]), "invalid timed reset value")
    return {"setup_wall_ns": records[0]["wall_ns"], "fixture_validation_wall_ns": records[1]["wall_ns"],
            "sample_wall_ns": [value["wall_ns"] for value in later], "timed_reset_ns": [value["timed_value_ns"] for value in later]}


def projection(observed, warmups=5, measured=30, runs=5):
    samples = observed["sample_wall_ns"]
    require(len(samples) >= 2, "projection needs a first and at least one later sample")
    later = samples[1:]
    # The first sample also performs the initial bootstrap; later samples restore after reset.
    worker = observed["setup_wall_ns"] + 2 * observed["fixture_validation_wall_ns"] + samples[0] + sum(later) * (warmups + measured - 1) // len(later)
    return {"per_worker_ns": worker, "per_cohort_ns": worker * runs,
            "inputs": {"warmups": warmups, "measured": measured, "runs": runs, "later_samples_observed": len(later)},
            "qualification": "integer arithmetic from observed contaminated stage wall times; not a measurement, bound or budget"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--engine", choices=["memory", "native-protected"], required=True)
    parser.add_argument("--n", type=int, choices=[10000], default=10000)
    parser.add_argument("--samples", type=int, choices=[2, 3], default=2)
    parser.add_argument("--timeout-seconds", type=int, default=1800)
    args = parser.parse_args()
    require(0 < args.timeout_seconds <= 7200, "invalid diagnostic timeout")
    root = Path(__file__).resolve().parents[1]
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    frozen = source_snapshot(root, SOURCE_PATHS)
    command = ["cargo", "build", "--release", "--manifest-path", "benchmarks/native/Cargo.toml", "--offline", "--locked"]
    result = {"format": 1, "status": "failed", "kind": "contaminated_cost_diagnostic", "engine": args.engine,
              "parameters": {"operation": "reset", "N": args.n, "Q": 0, "observers": 0}, "samples": args.samples,
              "timeout_seconds": args.timeout_seconds, "build_command": command, "source_before": frozen["sha256"],
              "environment_before": environment()}
    try:
        with (output / "build.txt").open("xb") as log:
            require(subprocess.run(command, cwd=root, stdout=log, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, check=False).returncode == 0, "release build failed")
        executable = root / "benchmarks/native/target/release/cost-diagnostic"
        result["artifact"] = {"path": "benchmarks/native/target/release/cost-diagnostic", "sha256": file_hash(executable),
                              "measured_worker_sha256": file_hash(root / "benchmarks/native/target/release/hoyadb-benchmark")}
        directory = output / "owned-store"
        if args.engine == "native-protected":
            directory.mkdir()
        launch = [str(executable), "reset", str(args.n), str(args.samples), str(directory) if args.engine == "native-protected" else "-"]
        result["command"] = launch
        lines = []
        started = time.monotonic_ns()
        with (output / "stderr.txt").open("xb") as errors:
            process = subprocess.Popen(launch, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=errors)
            result["pid"] = process.pid
            deadline = started + args.timeout_seconds * 1_000_000_000
            import selectors
            selector = selectors.DefaultSelector()
            selector.register(process.stdout, selectors.EVENT_READ)
            buffer = b""
            while True:
                remaining = (deadline - time.monotonic_ns()) / 1e9
                if remaining <= 0:
                    process.kill()
                    result["interrupted"] = "parent timeout; SIGKILL then reaped"
                    break
                if not selector.select(timeout=min(remaining, 5)):
                    continue
                chunk = os.read(process.stdout.fileno(), 65536)
                if not chunk:
                    break
                buffer += chunk
                while b"\n" in buffer:
                    line, buffer = buffer.split(b"\n", 1)
                    lines.append(line + b"\n")
                    print("observed " + line.decode("utf-8", "replace"), flush=True)
            selector.close()
            process.stdout.close()
            result["exit_code"] = process.wait()
        result["launch_to_exit_wall_ns"] = time.monotonic_ns() - started
        result["raw_lines"] = [line.decode("utf-8", "replace") for line in lines]
        require("interrupted" not in result and result["exit_code"] == 0, "diagnostic interrupted or failed")
        result["observed"] = parse(lines, args.samples)
        result["projection"] = projection(result["observed"])
        require(source_snapshot(root, SOURCE_PATHS) == frozen, "source changed during diagnostic")
        result["status"] = "contaminated_cost_diagnostic"
        returncode = 0
    except (RunnerError, OSError, ValueError) as error:
        result["failure"] = str(error)
        returncode = 1
    finally:
        result["environment_after"] = environment()
        write_new(output / "diagnostic.json", result)
    return returncode


if __name__ == "__main__":
    sys.exit(main())
