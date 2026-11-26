"""TC-0054: keep GitLab gates and untrusted CI boundaries during the migration.

These source-contract checks complement actionlint's actual YAML/expression lint.
They intentionally do not implement a general YAML parser.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class WorkflowTests(unittest.TestCase):
    def test_github_preserves_gitlab_verification_commands(self):
        github = (ROOT / ".github/workflows/ci.yml").read_text()
        gitlab = (ROOT / ".gitlab-ci.yml").read_text()
        scripts = re.findall(r"^  script:\n((?:    .*\n)+)", gitlab, re.M)
        self.assertTrue(scripts)
        for script in scripts:
            for line in script.splitlines():
                command = line.removeprefix("    - ")
                # Platform setup is supplied by the local Rust action; the
                # quality wrappers already add their own tool path.
                if command.startswith(("apt-get ", "export PATH=", "clang --version", "llvm-ar --version")):
                    continue
                with self.subTest(command=command):
                    self.assertIn(command, github)
        self.assertLess(github.index("sh crates/hoyadb-web/build.sh"), github.index("npm test"))
        self.assertIn("npm run test:packaging", github)
        self.assertIn("cargo fetch --manifest-path spikes/cross-platform/Cargo.toml --locked", github)

    def test_external_actions_are_immutable_and_pull_requests_are_read_only(self):
        for path in (ROOT / ".github").rglob("*.yml"):
            for action in re.findall(r"uses: ([^\s#]+)", path.read_text()):
                with self.subTest(path=path.name, action=action):
                    self.assertTrue(action.startswith("./") or re.fullmatch(r"[\w.-]+/[\w./-]+@[0-9a-f]{40}", action))
        ci = (ROOT / ".github/workflows/ci.yml").read_text()
        for forbidden in ("pull_request_target", "self-hosted", "secrets.", "contents: write", "id-token: write", "continue-on-error"):
            self.assertNotIn(forbidden, ci)
        self.assertIn("contents: read", ci)
        for path in (ROOT / ".github/workflows").glob("*.yml"):
            if "self-hosted" in path.read_text():
                self.assertNotIn("pull_request", path.read_text())
                self.assertIn("github.ref == 'refs/heads/main'", path.read_text())

    def test_pages_deploys_only_after_complete_verification_and_browser_checks(self):
        pages = (ROOT / ".github/workflows/pages.yml").read_text()
        commands = ["python3 docs/public/verify.py", "python3 docs/public/build.py\n",
                    "node docs/public/browser-check.mjs", "python3 docs/public/build.py --skip-rustdoc",
                    "uses: actions/upload-pages-artifact@"]
        positions = [pages.index(command) for command in commands]
        self.assertEqual(positions, sorted(positions))
        self.assertIn("needs: build", pages)
        self.assertIn("path: docs/public/dist", pages)

    def test_required_status_covers_all_jobs_even_when_a_dependency_fails(self):
        ci = (ROOT / ".github/workflows/ci.yml").read_text()
        jobs = set(re.findall(r"^  ([\w-]+):\n    (?:runs-on|name):", ci, re.M))
        aggregate = ci.split("  required-checks:\n", 1)[1]
        self.assertIn("if: always()", aggregate)
        needs = re.search(r"needs: \[([^\]]+)\]", aggregate).group(1)
        self.assertEqual(set(needs.replace(" ", "").split(",")), jobs - {"required-checks"})
        self.assertIn("CI_JOB_RESULTS: ${{ toJSON(needs) }}", aggregate)

    def test_all_npm_packages_include_the_selected_license(self):
        for name in ("client", "testing", "react-native"):
            self.assertEqual((ROOT / "packages" / name / "LICENSE").read_bytes(), (ROOT / "LICENSE").read_bytes())
        for folder in ("crates/hoyadb-native/third-party", "bindings/android/third-party"):
            self.assertEqual((ROOT / folder / "LICENSE").read_bytes(), (ROOT / "LICENSE").read_bytes())

    def test_browser_harness_dispatches_shared_review_regressions(self):
        base = ROOT / "spikes/storage-browser"
        entity = (base / "src/entity_conformance.rs").read_text()
        page = (base / "entity-page.js").read_text()
        for case in ("pull_tombstone_mapping_target", "pull_after_rolled_back_sent_cancellation"):
            with self.subTest(case=case):
                self.assertIn(f"pull::{case}(fresh());", entity)
                self.assertIn(f"'{case}'", page)
        self.assertIn("model_cases::review_m01_backfill_reaches_implicit_key_of_same_migration_to_one",
                      (base / "src/model_evidence.rs").read_text())
        self.assertIn("'REVIEW_M01'", (base / "tests.js").read_text())
        self.assertIn("group==='entity'?15:", (base / "security-page.js").read_text())


if __name__ == "__main__":
    unittest.main()
