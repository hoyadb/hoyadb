---
id: DEF-0052
title: Traceability validation silently accepts invalid scan inputs
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-31
parents: [SPEC-0046]
affects: [SPEC-0045, REQ-0048]
regression_for: [TC-0050]
tags: [review, regression]
---

# DEF-0052: Traceability validation silently accepts invalid scan inputs

## Symptom

The 2026-10-30 review recorded REL-03: A missing scan root and a candidate artifact with unclosed frontmatter both return success with zero violations.

## Acceptance

Reject missing/non-directory roots, unreadable or undecodable candidates and unterminated frontmatter; continue allowing ordinary Markdown without artifact frontmatter and report errors coherently in the RTM consumer.

## Root cause and fix

The loader treated read/decode failures and an unclosed frontmatter block as
non-artifacts; an unchecked root could also produce an empty artifact list.
`tools/traceability/trace_check.py` now reports a structured `input-invalid`
violation for invalid roots, directory enumeration errors, unreadable/undecodable
Markdown and unterminated frontmatter. It refuses to validate a partial graph.
The shared RTM consumer reports the input error and exits before writing output.
Ordinary Markdown without leading frontmatter and existing readable empty
directories remain allowed. Historical extraction hashes are preserved, with
the later hardening explicitly documented in the current tooling README.

## Regression and verification

TC-0050 FIX09 is implemented in `tools/traceability/test_input_errors.py`. CLI
cases cover missing/non-directory roots, invalid UTF-8, unclosed frontmatter and
all RTM output modes. Injected filesystem failures cover permission and directory
enumeration errors without depending on CI privilege levels. A prior RTM remains
unchanged on rejected input. Plain documents, an empty directory and the original
valid fixture remain accepted.

`python3 -m unittest discover -s tools/traceability -p 'test_*.py'` first ran 12
tests with nine expected negative-subcase failures. After the fix the same 12
tests passed. An additional root-permission regression then failed with an
uncaught `PermissionError`; after guarding root inspection all 13 tests passed.
Original extraction fixtures and `test_trace_check.py` remain unchanged. Claude
Fable 5.1 independently reviewed the change and reran the 13 tests without
finding a correctness defect. The artifact retains draft status; see the
[remediation report](../../implementation/review-remediation.md).

