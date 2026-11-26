#!/usr/bin/env python3
"""Requirements Traceability Matrix generator.

Walks a directory the same way tools/trace_check.py does (and reuses its
parser rather than duplicating it — see CODE-2/CODE-6) and renders a Markdown
table of `requirement`/`nfr` artifacts against the specs that satisfy them and
the test-cases that verify them.

Not itself covered by SPEC-0001's acceptance criteria (see that spec's "out of
scope" section) — this is a companion generator, not a validator.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import List

sys.path.insert(0, str(Path(__file__).resolve().parent))
import trace_check  # noqa: E402  (needs sys.path set up first)

TABLE_HEADER = "| Requirement | Status | Specs | Test cases | Result |"
TABLE_DIVIDER = "| --- | --- | --- | --- | --- |"


def build_rtm(root: Path) -> str:
    artifacts = trace_check.collect_artifacts(root)
    requirements = [a for a in artifacts if a.type in ("requirement", "nfr")]

    rows = [TABLE_HEADER, TABLE_DIVIDER]
    for req in sorted(requirements, key=lambda a: a.id or ""):
        specs = sorted(
            a.id
            for a in artifacts
            if a.id and req.id in trace_check.as_list(a.front_matter.get("satisfies"))
        )
        tests = sorted(
            a.id
            for a in artifacts
            if a.type == "test-case"
            and a.id
            and req.id in trace_check.as_list(a.front_matter.get("verifies"))
        )
        result = "Covered" if specs and tests else "Orphan"
        rows.append(
            "| {req} | {status} | {specs} | {tests} | {result} |".format(
                req=req.id,
                status=req.status or "",
                specs=", ".join(specs) or "—",
                tests=", ".join(tests) or "—",
                result=result,
            )
        )
    return "\n".join(rows) + "\n"


def main(argv: List[str] = None) -> int:
    parser = argparse.ArgumentParser(description="Generate the Requirements Traceability Matrix.")
    parser.add_argument("root", help="Directory to scan for *.md artifacts")
    parser.add_argument(
        "--stdout", action="store_true", help="Print the RTM instead of writing it to disk"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Exit 1 if the generated RTM differs from what's on disk, without writing",
    )
    args = parser.parse_args(argv)

    root = Path(args.root)
    try:
        content = build_rtm(root)
    except trace_check.ArtifactInputError as error:
        print(f"RTM input rejected: {error}", file=sys.stderr)
        return 1
    out_path = root / "traceability" / "rtm.md"

    if args.check:
        existing = out_path.read_text(encoding="utf-8") if out_path.exists() else None
        if existing != content:
            print(f"{out_path} is out of date with the generated RTM")
            return 1
        print(f"{out_path} is up to date")
        return 0

    if args.stdout:
        print(content, end="")
        return 0

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(content, encoding="utf-8")
    print(f"wrote {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
