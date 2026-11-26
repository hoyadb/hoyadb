#!/usr/bin/env python3
"""Traceability checker for this repository's Markdown artifacts.

See specs/SPEC-0001-trace-check.md for the full spec this implements.

IMPORTANT: the front-matter parser in this file is NOT a general YAML parser.
It understands only the restricted subset artifact front-matter is written
in: `key: value` scalars, `key: [a, b]` inline lists, and `key:` followed by
indented `- item` block lists. Anything outside that subset (nested maps,
multi-line scalars, anchors, flow maps, ...) is not supported by design —
see SPEC-0001's "Out of scope" section.

Dependency-free by policy (CODE-6): stdlib only, no PyYAML.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional

REQUIRED_KEYS = ["id", "title", "type", "status", "owner", "created", "updated"]
LINK_KEYS = ["parents", "satisfies", "verifies", "mitigates", "depends_on", "supersedes"]
ID_RE = re.compile(r"^[A-Z]+-\d{4}$")
ID_EXEMPT_TYPES = {"ruleset"}

# Status lifecycle table, hardcoded per SPEC-0001. Types not listed here are
# not status-checked (they still get every other check).
STATUS_BY_TYPE: Dict[str, List[str]] = {}


def _register_statuses(types: List[str], statuses: List[str]) -> None:
    for t in types:
        STATUS_BY_TYPE[t] = statuses


_register_statuses(
    ["requirement", "nfr", "spec", "architecture", "api", "component", "ux", "tokens", "data-model"],
    ["draft", "review", "accepted", "deprecated", "removed"],
)
_register_statuses(["adr"], ["proposed", "accepted", "rejected", "deprecated", "superseded"])
_register_statuses(
    ["rfd"],
    ["prediscussion", "ideation", "discussion", "published", "committed", "abandoned"],
)
_register_statuses(
    ["threat"],
    ["identified", "analyzed", "mitigation-planned", "mitigated", "accepted-risk"],
)
_register_statuses(["test-case", "test-plan"], ["draft", "active", "quarantined", "retired"])
_register_statuses(
    ["vision", "objective", "risk", "defect", "runbook", "release", "ruleset"],
    ["draft", "active", "deprecated"],
)


@dataclass
class Violation:
    file: str
    rule: str
    message: str

    def to_dict(self) -> dict:
        return {"file": self.file, "rule": self.rule, "message": self.message}

    def __str__(self) -> str:
        return f"{self.file}: {self.message}"


class ArtifactInputError(ValueError):
    """Input cannot be inspected reliably; never validate a partial graph."""

    def __init__(self, path: Path, message: str):
        self.path = path
        self.message = message
        super().__init__(f"{path}: {message}")


@dataclass
class Artifact:
    path: Path
    front_matter: Dict[str, object] = field(default_factory=dict)

    @property
    def id(self) -> Optional[str]:
        value = self.front_matter.get("id")
        return value if isinstance(value, str) else None

    @property
    def type(self) -> Optional[str]:
        value = self.front_matter.get("type")
        return value if isinstance(value, str) else None

    @property
    def status(self) -> Optional[str]:
        value = self.front_matter.get("status")
        return value if isinstance(value, str) else None


def _strip_quotes(value: str) -> str:
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in ("'", '"'):
        return value[1:-1]
    return value


def as_list(value) -> List[str]:
    """Normalise a front-matter value that may be a scalar, list, or missing."""
    if value is None:
        return []
    if isinstance(value, list):
        return [str(v) for v in value]
    return [str(value)]


def parse_front_matter_lines(lines: List[str]) -> Dict[str, object]:
    """Parse the restricted YAML subset described in the module docstring."""
    data: Dict[str, object] = {}
    i = 0
    n = len(lines)
    key_re = re.compile(r"^([A-Za-z0-9_\-]+):\s*(.*)$")
    block_item_re = re.compile(r"^\s*-\s+(.*)$")
    while i < n:
        line = lines[i]
        if not line.strip() or line.strip().startswith("#"):
            i += 1
            continue
        match = key_re.match(line)
        if not match:
            i += 1
            continue
        key, rest = match.group(1), match.group(2).strip()
        if rest == "":
            # Possibly a block list: subsequent indented "- item" lines.
            items: List[str] = []
            j = i + 1
            while j < n and block_item_re.match(lines[j]):
                items.append(_strip_quotes(block_item_re.match(lines[j]).group(1)))
                j += 1
            if items:
                data[key] = items
                i = j
            else:
                data[key] = ""
                i += 1
        elif rest.startswith("[") and rest.endswith("]"):
            inner = rest[1:-1].strip()
            data[key] = [] if inner == "" else [_strip_quotes(x) for x in inner.split(",")]
            i += 1
        else:
            data[key] = _strip_quotes(rest)
            i += 1
    return data


def extract_front_matter_block(text: str) -> Optional[List[str]]:
    """Return the lines between the leading '---' delimiters, or None.

    A file is only an "artifact" if its very first line is exactly '---'
    (plain docs like READMEs are not artifacts and are skipped entirely).
    """
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return None
    for i in range(1, len(lines)):
        if lines[i].strip() == "---":
            return lines[1:i]
    raise ValueError("unterminated frontmatter: opening '---' has no closing delimiter")


def load_artifact(path: Path) -> Optional[Artifact]:
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        raise ArtifactInputError(path, f"cannot read Markdown input: {error}") from error
    try:
        block = extract_front_matter_block(text)
    except ValueError as error:
        raise ArtifactInputError(path, str(error)) from error
    if block is None:
        return None
    return Artifact(path=path, front_matter=parse_front_matter_lines(block))


def collect_artifacts(root: Path) -> List[Artifact]:
    def scan_error(error: OSError) -> None:
        raise ArtifactInputError(Path(error.filename or root),
                                 f"cannot scan artifact directory: {error}") from error

    try:
        is_directory = root.is_dir()
    except OSError as error:
        scan_error(error)
    if not is_directory:
        raise ArtifactInputError(root, "scan root must be an existing directory")

    artifacts = []
    # pathlib glob can suppress filesystem errors, yielding an incomplete graph.
    paths = [Path(directory) / name
             for directory, _, names in os.walk(root, onerror=scan_error)
             for name in names if name.endswith(".md")]
    for path in sorted(paths):
        artifact = load_artifact(path)
        if artifact is not None:
            artifacts.append(artifact)
    return artifacts


def load_registry_ids(registry_path: Path) -> set:
    """Parse an external registry file for extra ids link checks may target.

    Uses the same restricted parser, expecting an `ids:` inline or block list.
    """
    text = registry_path.read_text(encoding="utf-8")
    lines = text.splitlines()
    # A registry file may or may not have '---' delimiters; parse the whole
    # file as one front-matter-style block either way.
    block = extract_front_matter_block(text)
    if block is None:
        block = lines
    data = parse_front_matter_lines(block)
    return set(as_list(data.get("ids")))


def _rel(path: Path, root: Path) -> str:
    try:
        return str(path.relative_to(root))
    except ValueError:
        return str(path)


def check_required_fields(artifact: Artifact, rel: str, violations: List[Violation]) -> None:
    for key in REQUIRED_KEYS:
        if key not in artifact.front_matter or artifact.front_matter.get(key) in ("", None):
            violations.append(
                Violation(rel, "missing-field", f"missing required field: {key}")
            )


def check_id_format(artifact: Artifact, rel: str, violations: List[Violation]) -> None:
    artifact_id = artifact.id
    if artifact_id is None:
        return  # already reported by check_required_fields
    if artifact.type in ID_EXEMPT_TYPES:
        return
    if not ID_RE.match(artifact_id):
        violations.append(
            Violation(
                rel,
                "id-format",
                f"id '{artifact_id}' does not match required format ^[A-Z]+-\\d{{4}}$",
            )
        )


def check_duplicate_ids(artifacts: List[Artifact], root: Path, violations: List[Violation]) -> None:
    by_id: Dict[str, List[Artifact]] = {}
    for artifact in artifacts:
        if artifact.id:
            by_id.setdefault(artifact.id, []).append(artifact)
    for artifact_id, owners in by_id.items():
        if len(owners) > 1:
            for owner in owners:
                violations.append(
                    Violation(
                        _rel(owner.path, root),
                        "id-duplicate",
                        f"id '{artifact_id}' is used by {len(owners)} files",
                    )
                )


def check_status(artifact: Artifact, rel: str, violations: List[Violation]) -> None:
    if artifact.type is None or artifact.status is None:
        return  # already reported by check_required_fields
    allowed = STATUS_BY_TYPE.get(artifact.type)
    if allowed is None:
        return  # type not covered by the lifecycle table: out of scope
    if artifact.status not in allowed:
        violations.append(
            Violation(
                rel,
                "status-invalid",
                f"status '{artifact.status}' is not legal for type '{artifact.type}' "
                f"(allowed: {', '.join(allowed)})",
            )
        )


def check_links(
    artifact: Artifact, rel: str, all_ids: set, violations: List[Violation]
) -> None:
    for key in LINK_KEYS:
        for target in as_list(artifact.front_matter.get(key)):
            if target and target not in all_ids:
                violations.append(
                    Violation(
                        rel,
                        "link-dangling",
                        f"{key} references unknown id '{target}'",
                    )
                )


def check_orphans(artifacts: List[Artifact], root: Path, violations: List[Violation]) -> None:
    satisfies_targets = set()
    verifies_targets_by_test_case = set()
    for artifact in artifacts:
        for target in as_list(artifact.front_matter.get("satisfies")):
            satisfies_targets.add(target)
        if artifact.type == "test-case":
            for target in as_list(artifact.front_matter.get("verifies")):
                verifies_targets_by_test_case.add(target)

    for artifact in artifacts:
        if artifact.type not in ("requirement", "nfr"):
            continue
        if artifact.status != "accepted":
            continue
        rel = _rel(artifact.path, root)
        if artifact.id not in satisfies_targets:
            violations.append(
                Violation(
                    rel,
                    "orphan-satisfies",
                    f"accepted {artifact.type} '{artifact.id}' has no artifact satisfying it",
                )
            )
        if artifact.id not in verifies_targets_by_test_case:
            violations.append(
                Violation(
                    rel,
                    "orphan-verifies",
                    f"accepted {artifact.type} '{artifact.id}' has no test-case verifying it",
                )
            )


def run(root: str, fmt: str = "text", registry: Optional[str] = None) -> "tuple[List[Violation], int]":
    """Run all checks against `root`. Returns (violations, exit_code).

    Refactored out of main() so tests can call it directly without shelling
    out to a subprocess.
    """
    root_path = Path(root)
    try:
        artifacts = collect_artifacts(root_path)
    except ArtifactInputError as error:
        return [Violation(_rel(error.path, root_path), "input-invalid", error.message)], 1
    violations: List[Violation] = []

    for artifact in artifacts:
        rel = _rel(artifact.path, root_path)
        check_required_fields(artifact, rel, violations)
        check_id_format(artifact, rel, violations)
        check_status(artifact, rel, violations)

    check_duplicate_ids(artifacts, root_path, violations)

    all_ids = {a.id for a in artifacts if a.id}
    if registry:
        try:
            all_ids |= load_registry_ids(Path(registry))
        except (OSError, ValueError) as error:
            violations.append(Violation(registry, "input-invalid", f"cannot read registry input: {error}"))
    for artifact in artifacts:
        rel = _rel(artifact.path, root_path)
        check_links(artifact, rel, all_ids, violations)

    check_orphans(artifacts, root_path, violations)

    exit_code = 1 if violations else 0
    return violations, exit_code


def render_report(violations: List[Violation], fmt: str) -> str:
    if fmt == "json":
        return json.dumps([v.to_dict() for v in violations], indent=2)
    lines = [str(v) for v in violations]
    lines.append(f"{len(violations)} violation(s)")
    return "\n".join(lines)


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate the traceability system of Markdown artifacts."
    )
    parser.add_argument("root", help="Directory to scan for *.md artifacts")
    parser.add_argument(
        "--format", choices=["text", "json"], default="text", help="Report format"
    )
    parser.add_argument(
        "--registry",
        default=None,
        help="Optional external registry file of extra ids link checks may resolve against",
    )
    args = parser.parse_args(argv)

    violations, exit_code = run(args.root, args.format, args.registry)
    print(render_report(violations, args.format))
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
