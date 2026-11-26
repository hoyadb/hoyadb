"""DEF-0052 / TC-0050 FIX09: unusable input cannot pass the public gate."""

import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import rtm_gen
import trace_check


class InputErrorTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)

    def cli(self, script, root, *arguments):
        return subprocess.run(
            [sys.executable, str(Path(__file__).with_name(script)), str(root), *arguments],
            capture_output=True, text=True, timeout=10,
        )

    def assert_rejected(self, root, expected):
        result = self.cli("trace_check.py", root, "--format", "json")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn(expected, result.stdout)
        self.assertEqual(json.loads(result.stdout)[0]["rule"], "input-invalid")
        self.assertNotIn("Traceback", result.stderr)

    def test_missing_and_nondirectory_roots_are_rejected(self):
        file = self.root / "file.md"
        file.write_text("# Not a directory\n")
        for root in (self.root / "missing", file):
            with self.subTest(root=root):
                self.assert_rejected(root, "scan root must be an existing directory")
                result = self.cli("rtm_gen.py", root, "--stdout")
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertEqual(result.stdout, "")
                self.assertNotIn("Traceback", result.stderr)

    def test_unclosed_frontmatter_and_invalid_utf8_are_rejected(self):
        path = self.root / "REQ-0001.md"
        for data, message in (
            (b"---\nid: REQ-0001\ntype: requirement\nstatus: accepted\n", "unterminated frontmatter"),
            (b"---\nid: REQ-0001\n---\n\xff", "cannot read Markdown input"),
        ):
            with self.subTest(data=data):
                path.write_bytes(data)
                self.assert_rejected(self.root, message)

    def test_unreadable_markdown_is_reported_and_does_not_replace_rtm(self):
        path = self.root / "REQ-0001.md"
        path.write_text("---\nid: REQ-0001\n---\n")
        output = self.root / "traceability/rtm.md"
        output.parent.mkdir()
        output.write_text("previous evidence\n")
        read_text = Path.read_text

        def read(candidate, *args, **kwargs):
            if candidate == path:
                raise PermissionError("synthetic permission denied")
            return read_text(candidate, *args, **kwargs)

        # Permission bits cannot reliably provoke EACCES under privileged CI.
        # Inject only the filesystem failure, exercising both real entry points.
        with patch.object(Path, "read_text", read):
            violations, status = trace_check.run(str(self.root))
            self.assertEqual(status, 1)
            self.assertIn("REQ-0001.md", violations[0].file)
            self.assertIn("cannot read Markdown input", violations[0].message)
            errors = io.StringIO()
            with contextlib.redirect_stderr(errors):
                self.assertEqual(rtm_gen.main([str(self.root)]), 1)
            self.assertIn("cannot read Markdown input", errors.getvalue())
        self.assertEqual(output.read_text(), "previous evidence\n")

    def test_failed_directory_enumeration_is_rejected(self):
        def walk(root, *, onerror):
            onerror(PermissionError(13, "synthetic permission denied", str(root / "locked")))
            return iter(())

        with patch.object(trace_check.os, "walk", walk):
            violations, status = trace_check.run(str(self.root))
        self.assertEqual(status, 1)
        self.assertIn("cannot scan artifact directory", violations[0].message)

    def test_unreadable_root_is_reported_on_python_versions_that_raise(self):
        with patch.object(Path, "is_dir", side_effect=PermissionError("synthetic permission denied")):
            violations, status = trace_check.run(str(self.root))
        self.assertEqual(status, 1)
        self.assertIn("cannot scan artifact directory", violations[0].message)

    def test_rtm_rejects_malformed_input_without_writing_output(self):
        (self.root / "REQ-0001.md").write_text("---\nid: REQ-0001\n")
        for arguments in ((), ("--check",), ("--stdout",)):
            with self.subTest(arguments=arguments):
                result = self.cli("rtm_gen.py", self.root, *arguments)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertEqual(result.stdout, "")
                self.assertIn("unterminated frontmatter", result.stderr)
                self.assertNotIn("Traceback", result.stderr)
                self.assertFalse((self.root / "traceability").exists())

    def test_empty_directory_and_plain_markdown_remain_valid(self):
        for text in (None, "# README\n\n---\nThis is a document separator.\n"):
            if text is not None:
                (self.root / "README.md").write_text(text)
            with self.subTest(text=text):
                self.assertEqual(trace_check.run(str(self.root)), ([], 0))
                result = self.cli("rtm_gen.py", self.root, "--stdout")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn(rtm_gen.TABLE_HEADER, result.stdout)


if __name__ == "__main__":
    unittest.main()
