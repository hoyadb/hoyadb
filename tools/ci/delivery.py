"""SPEC-0050: dependency-free GitHub result and release preflight gates."""
import argparse
import json
import os
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
PACKAGES = ("client", "testing", "react-native")


def check_results(results):
    if not isinstance(results, dict) or not results:
        raise ValueError("prerequisite job results are missing")
    for name, job in results.items():
        if not isinstance(job, dict) or job.get("result") != "success":
            raise ValueError(f"prerequisite did not succeed: {name}")


def check_release(root, tag, repository):
    if not re.fullmatch(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", tag):
        raise ValueError("release requires a strict vMAJOR.MINOR.PATCH tag")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("GitHub owner/repository is missing")
    try:
        license_text = (root / "LICENSE").read_text()
        if not license_text.startswith("MIT License\n") or "Permission is hereby granted, free of charge" not in license_text:
            raise ValueError("the selected MIT license is missing")
        for name in PACKAGES:
            manifest = json.loads((root / "packages" / name / "package.json").read_text())
            if (manifest.get("name") != "@hoyadb/" + name or manifest.get("version") != tag[1:]
                    or manifest.get("private", False) is not False or manifest.get("license") != "MIT"
                    or manifest.get("publishConfig", {}).get("access") != "public"):
                raise ValueError(f"release metadata does not match {tag}: {name}")
            if manifest.get("repository", {}).get("url") != f"https://github.com/{repository}.git":
                raise ValueError(f"repository metadata must match the GitHub provenance identity: {name}")
            field = "dependencies" if name == "testing" else "peerDependencies"
            if name != "client" and manifest.get(field, {}).get("@hoyadb/client") != tag[1:]:
                raise ValueError(f"client dependency must match the release: {name}")
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError("release inputs are missing or unreadable") from error
    return tag[1:]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("results", "release"))
    parser.add_argument("--tag", default="")
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", ""))
    args = parser.parse_args()
    try:
        if args.command == "results":
            check_results(json.loads(os.environ.get("CI_JOB_RESULTS", "null")))
            print("All prerequisite jobs succeeded")
        else:
            print(check_release(args.root, args.tag, args.repository))
    except (ValueError, TypeError, AttributeError) as error:
        parser.exit(1, f"Delivery rejected: {error}\n")


if __name__ == "__main__":
    main()
