#!/usr/bin/env python3
"""TC-0037 P04: strict independent raw-sample statistics, no baseline mutation."""
import argparse
import copy
from decimal import Decimal
import json
from pathlib import Path
import sys


class InvalidSamples(ValueError):
    pass


def _require(condition, message):
    if not condition:
        raise InvalidSamples(message)


def _integer(value, minimum=0, maximum=(1 << 64) - 1):
    return type(value) is int and minimum <= value <= maximum


def _median(values):
    values = sorted(values)
    middle = len(values) // 2
    if len(values) % 2:
        return Decimal(values[middle])
    return (Decimal(values[middle - 1]) + Decimal(values[middle])) / 2


def _decimal(value):
    return format(value, "f").rstrip("0").rstrip(".") if value != value.to_integral_value() else str(int(value))


def _pairs(pairs):
    result = {}
    for key, value in pairs:
        _require(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def read(path):
    try:
        return json.loads(Path(path).read_text(encoding="utf-8"), object_pairs_hook=_pairs,
                          parse_constant=lambda value: (_ for _ in ()).throw(InvalidSamples("non-finite JSON number")))
    except (OSError, UnicodeError, ValueError, TypeError) as error:
        raise InvalidSamples("invalid sample file") from error


def summarize(documents):
    _require(isinstance(documents, list) and documents, "no sample documents")
    identity = protocol = None
    runs = {}
    for document in documents:
        _require(isinstance(document, dict) and type(document.get("format")) is int and document["format"] == 1, "invalid format")
        candidate = document.get("identity")
        _require(isinstance(candidate, dict), "missing identity")
        for key in ("class", "profile", "unit", "endpoint"):
            _require(isinstance(candidate.get(key), str) and candidate[key], "missing identity field")
        _require(candidate["unit"] in ("ns", "ms", "bytes"), "unknown unit")
        for key in ("artifact_sha256", "workload_sha256"):
            value = candidate.get(key)
            _require(isinstance(value, str) and len(value) == 64 and all(c in "0123456789abcdef" for c in value), "invalid hash")
        _require(isinstance(candidate.get("parameters"), dict), "missing parameters")
        try:
            json.dumps(candidate, allow_nan=False)
        except (ValueError, TypeError) as error:
            raise InvalidSamples("nonstandard identity") from error
        if identity is None:
            identity = candidate
        _require(candidate == identity, "mixed class/profile/unit/artifact/workload identity")
        declared = document.get("protocol")
        _require(isinstance(declared, dict) and set(declared) == {"runs", "warmups", "samples"}, "invalid protocol")
        _require(_integer(declared["runs"], 1) and _integer(declared["warmups"]) and _integer(declared["samples"], 1), "empty protocol")
        if protocol is None:
            protocol = declared
        _require(declared == protocol, "mixed sampling protocol")
        _require(isinstance(document.get("runs"), list), "missing runs")
        for run in document["runs"]:
            _require(isinstance(run, dict) and _integer(run.get("run"), 1, protocol["runs"]), "invalid run")
            run_id = run["run"]
            _require(run_id not in runs, "duplicate run")
            _require(run.get("status") == "ok", "failed or contaminated run")
            _require(isinstance(run.get("samples"), list), "missing samples")
            phases = {"warmup": {}, "measured": {}}
            for sample in run["samples"]:
                _require(isinstance(sample, dict), "invalid sample")
                _require(sample.get("unit",identity["unit"])==identity["unit"] and sample.get("profile",identity["profile"])==identity["profile"], "mixed sample unit/profile")
                phase = sample.get("phase")
                _require(phase in phases and _integer(sample.get("index")), "invalid sample index/phase")
                index = sample["index"]
                _require(index not in phases[phase], "duplicate sample")
                _require(sample.get("status") == "ok" and sample.get("correct") is True, "failed or incorrect sample")
                _require(_integer(sample.get("value")), "invalid duration/quantity")
                phases[phase][index] = sample["value"]
            _require(set(phases["warmup"]) == set(range(protocol["warmups"])), "missing warmups")
            _require(set(phases["measured"]) == set(range(protocol["samples"])), "missing measured samples")
            values = sorted(phases["measured"].values())
            # Exact nearest-rank ceil(95*n/100), never floating interpolation.
            rank = (95 * len(values) + 99) // 100
            runs[run_id] = {"run": run_id, "count": len(values), "median": _decimal(_median(values)),
                            "p95": values[rank - 1], "min": values[0], "max": values[-1]}
    _require(set(runs) == set(range(1, protocol["runs"] + 1)), "missing independent runs")
    ordered = [runs[key] for key in sorted(runs)]
    return {"format": 1, "identity": copy.deepcopy(identity), "sample_count": sum(r["count"] for r in ordered),
            "runs": ordered, "envelope": {"median_run_median": _decimal(_median([Decimal(r["median"]) for r in ordered])),
                                           "min_run_p95": min(r["p95"] for r in ordered),
                                           "max_run_p95": max(r["p95"] for r in ordered)}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="+")
    args = parser.parse_args()
    try:
        result = summarize([read(path) for path in args.files])
    except InvalidSamples as error:
        print(str(error), file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
