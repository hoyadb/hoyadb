"""The public snapshot contains current contracts without development archives."""
from pathlib import Path
import re
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
ARCHIVES = ["docs/" + "implementation", "docs/" + "references", "release-check"]
PRODUCT_NAMES = re.compile(r"\b(?:WatermelonDB|RxDB|PouchDB|CouchDB|PowerSync|ElectricSQL|ObjectBox|Realm)\b|cr-sqlite")
STAGES = re.compile(r"\bPhase\s+[0-7]\b", re.I)


class PublicSnapshotTests(unittest.TestCase):
    def test_archived_trees_and_implementation_plans_are_absent(self):
        for name in [*ARCHIVES, "IMPLEMENTATION_PLAN.md", "ROADMAP.md"]:
            with self.subTest(path=name):
                self.assertFalse((ROOT / name).exists())

    def test_public_narrative_has_no_comparisons_or_implementation_stage_labels(self):
        paths = subprocess.check_output(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT
        ).decode().split("\0")
        offenders = []
        for name in sorted(set(paths)):
            path = ROOT / name
            if (not path.is_file() or path.suffix not in {".md", ".html"}
                    or name.startswith(("licenses/", "docs/implementation/", "docs/references/"))
                    or "/third-party/" in name or "/testdata/" in name):
                continue
            text = path.read_text()
            if (PRODUCT_NAMES.search(text) or STAGES.search(text)
                    or "gitlab.com/alocas-ia/" in text or "`legacy/" in text):
                offenders.append(name)
        self.assertEqual(offenders, [])

    def test_current_tools_do_not_reference_removed_archive_paths(self):
        paths = subprocess.check_output(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT
        ).decode().split("\0")
        offenders = []
        for name in set(paths):
            path = ROOT / name
            if not path.is_file() or path.resolve() == Path(__file__).resolve():
                continue
            try:
                text = path.read_text()
            except UnicodeError:
                continue
            if any(prefix + "/" in text for prefix in ARCHIVES[:2]):
                offenders.append(name)
        self.assertEqual(sorted(offenders), [])


if __name__ == "__main__":
    unittest.main()
