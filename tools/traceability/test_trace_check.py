#!/usr/bin/env python3
"""Tests for tools/trace_check.py.

One test method per Gherkin scenario in specs/tests/*.feature. Each method's
docstring quotes the scenario's Given/When/Then. Fixture repos live as real
files under tools/testdata/<case>/ so they double as documentation.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import trace_check  # noqa: E402

TESTDATA = Path(__file__).resolve().parent / "testdata"


class TraceCheckTests(unittest.TestCase):
    def test_valid_repo_reports_no_violations(self):
        """specs/tests/valid-repo.feature

        Given a directory of Markdown artifacts where every file has valid
          front-matter with a unique, well-formed id, a legal status for its
          type, and every link resolves to a real id
        And every "requirement"/"nfr" with status "accepted" is satisfied by
          another artifact and verified by a test-case
        When the traceability checker is run against that directory
        Then it exits with status code 0
        And it reports zero violations
        """
        violations, exit_code = trace_check.run(str(TESTDATA / "good"))
        self.assertEqual(exit_code, 0)
        self.assertEqual(violations, [])

    def test_missing_required_field_is_reported(self):
        """specs/tests/missing-required-field.feature

        Given a Markdown artifact whose front-matter has "id", "title",
          "type", "owner", "created", and "updated" but is missing "status"
        When the traceability checker is run against the directory
          containing it
        Then it reports a violation naming that file and the missing
          "status" field
        And it exits with a non-zero status code
        """
        violations, exit_code = trace_check.run(str(TESTDATA / "missing-field"))
        self.assertEqual(exit_code, 1)
        messages = [str(v) for v in violations]
        self.assertTrue(
            any("REQ-0001.md" in m and "missing required field: status" in m for m in messages),
            messages,
        )

    def test_duplicate_id_is_reported_for_both_files(self):
        """specs/tests/duplicate-id.feature

        Given two Markdown artifacts in the scanned directory that both
          declare the same "id" in their front-matter
        When the traceability checker is run against that directory
        Then it reports a violation for the first file naming the duplicate id
        And it reports a violation for the second file naming the duplicate id
        And it exits with a non-zero status code
        """
        violations, exit_code = trace_check.run(str(TESTDATA / "duplicate-id"))
        self.assertEqual(exit_code, 1)
        dup_violations = [v for v in violations if v.rule == "id-duplicate"]
        files_flagged = {v.file for v in dup_violations}
        self.assertIn("REQ-0001-a.md", files_flagged)
        self.assertIn("REQ-0001-b.md", files_flagged)

    def test_dangling_link_is_reported(self):
        """specs/tests/dangling-link.feature

        Given a Markdown artifact whose front-matter "satisfies" list names
          an id that is not the id of any artifact in the scanned directory
          and is not in the registry
        When the traceability checker is run against that directory
        Then it reports a dangling-link violation naming the file and the
          missing target id
        And it exits with a non-zero status code
        """
        violations, exit_code = trace_check.run(str(TESTDATA / "dangling-link"))
        self.assertEqual(exit_code, 1)
        messages = [str(v) for v in violations]
        self.assertTrue(
            any("SPEC-0001.md" in m and "REQ-9999" in m for m in messages), messages
        )

    def test_bad_status_is_reported(self):
        """specs/tests/bad-status.feature

        Given a Markdown artifact of type "requirement" whose "status" value
          is not one of "draft", "review", "accepted", "deprecated", "removed"
        When the traceability checker is run against the directory
          containing it
        Then it reports a violation naming the file, its status value, and
          its type
        And it exits with a non-zero status code
        """
        violations, exit_code = trace_check.run(str(TESTDATA / "bad-status"))
        self.assertEqual(exit_code, 1)
        messages = [str(v) for v in violations]
        self.assertTrue(
            any(
                "REQ-0001.md" in m and "in-progress" in m and "requirement" in m
                for m in messages
            ),
            messages,
        )

    def test_orphan_accepted_requirement_is_reported(self):
        """specs/tests/orphan-requirement.feature

        Given a Markdown artifact of type "requirement" with
          "status: accepted"
        And no other artifact in the scanned directory lists that
          requirement's id in its "satisfies" field
        And no "test-case" artifact in the scanned directory lists that
          requirement's id in its "verifies" field
        When the traceability checker is run against that directory
        Then it reports an orphan violation naming the requirement and that
          it has no satisfying artifact
        And it reports an orphan violation naming the requirement and that
          it has no verifying test-case
        And it exits with a non-zero status code
        """
        violations, exit_code = trace_check.run(str(TESTDATA / "orphan-requirement"))
        self.assertEqual(exit_code, 1)
        rules = {v.rule for v in violations}
        self.assertIn("orphan-satisfies", rules)
        self.assertIn("orphan-verifies", rules)


if __name__ == "__main__":
    unittest.main()
