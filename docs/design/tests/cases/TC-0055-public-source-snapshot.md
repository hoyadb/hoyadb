---
id: TC-0055
title: Public snapshot retains contracts without development archives
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-26
updated: 2026-11-26
parents: [TP-0001]
verifies: [SPEC-0051]
tags: [distribution, regression]
---

# Public snapshot checks

1. Archive/report trees and obsolete implementation plans are absent.
2. Public narrative has no product comparisons or implementation-stage labels.
3. Compatibility checks retain the same baseline bytes and still reject drift.
4. Notice checks retain license bytes, exact dependency identities and rejection
   of compiler/runtime/provider mismatches. Git filter tests retain a CRLF
   negative control independently of removed reports.
5. Public source links and traceability resolve. Reviewed dependency updates
   remain SHA-pinned and satisfy the workflow contract and lint.

Run public packaging, delivery, release and native tooling unit tests, Git byte
preservation tests, documentation build checks, traceability and affected builds.
