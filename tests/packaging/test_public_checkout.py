"""SPEC-0045 / TC-0049: public tooling works without the maintainer repository."""
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class PublicCheckoutTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="hoyadb-public-checkout-")
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        shutil.copytree(ROOT / "tools/traceability", self.root / "tools")
        shutil.copytree(self.root / "tools/testdata/good", self.root / "design")

    def run_tool(self, name, root="design", *arguments):
        return subprocess.run(
            ["python3", str(self.root / "tools" / name), str(self.root / root), *arguments],
            cwd=self.root, text=True, capture_output=True, timeout=10,
        )

    def test_checker_passes_valid_artifacts_and_rejects_invalid_ones(self):
        valid = self.run_tool("trace_check.py")
        self.assertEqual(valid.returncode, 0, valid.stdout + valid.stderr)
        for fixture, message in [("dangling-link", "REQ-9999"), ("missing-field", "status")]:
            with self.subTest(fixture=fixture):
                invalid = self.run_tool("trace_check.py", "tools/testdata/" + fixture)
                self.assertEqual(invalid.returncode, 1, invalid.stdout + invalid.stderr)
                self.assertIn(message, invalid.stdout + invalid.stderr)

    def test_rtm_check_rejects_stale_output_without_rewriting_it(self):
        generated = self.run_tool("rtm_gen.py")
        self.assertEqual(generated.returncode, 0, generated.stdout + generated.stderr)
        checked = self.run_tool("rtm_gen.py", "design", "--check")
        self.assertEqual(checked.returncode, 0, checked.stdout + checked.stderr)
        output = self.root / "design/traceability/rtm.md"
        self.assertIn("REQ-0001", output.read_text())
        output.write_text("stale matrix\n")
        stale = self.run_tool("rtm_gen.py", "design", "--check")
        self.assertEqual(stale.returncode, 1, stale.stdout + stale.stderr)
        self.assertEqual(output.read_text(), "stale matrix\n")

    def test_public_ci_does_not_fetch_private_governance(self):
        ci = (ROOT / ".gitlab-ci.yml").read_text()
        self.assertIn("python3 tools/traceability/trace_check.py docs/design", ci)
        self.assertIn("python3 tools/traceability/rtm_gen.py docs/design --check", ci)
        self.assertNotIn("ai-governance", ci)
        self.assertNotIn("GIT_SUBMODULE_STRATEGY: normal", ci)

    def test_javascript_ci_receives_ordinary_wasm_from_a_producer(self):
        """DEF-0050: a clean Node job must receive its ignored integration inputs."""
        ci = (ROOT / ".gitlab-ci.yml").read_text()
        jobs = dict(re.findall(r"^([\w.-]+):\n((?:[ \t].*\n|\n|#.*\n)*)", ci, re.M))
        consumer = jobs["javascript-host-quality"]
        self.assertIn("- npm test", consumer)
        # Resolve the producer from the consumer's artifact dependency, rather
        # than assuming files left on a developer machine will be present.
        needs = re.search(r"^  needs:\n((?:    .*\n)+)", consumer, re.M)
        self.assertIsNotNone(needs, "JavaScript CI has no generated-asset dependency")
        producers = re.findall(r"^    - job: ([\w.-]+)$", needs.group(1), re.M)
        self.assertTrue(producers, "JavaScript CI must name an artifact producer")
        self.assertIn("artifacts: true", needs.group(1))
        asset = "crates/hoyadb-web/pkg/"
        self.assertTrue(any(
            "sh crates/hoyadb-web/build.sh\n" in jobs.get(name, "")
            and "--test-controls" not in jobs[name]
            and "artifacts:\n" in jobs[name]
            and asset in jobs[name]
            for name in producers
        ), "A needed job must build and upload ordinary WASM assets")


if __name__ == "__main__":
    unittest.main()
