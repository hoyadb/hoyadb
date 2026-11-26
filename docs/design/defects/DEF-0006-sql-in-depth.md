---
id: DEF-0006
title: SQL In expansion rejects ordinary large result sets
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [REQ-0042, SPEC-0027, SPEC-0025]
depends_on: [API-0005, THR-0016]
tags: [reactivity, review-resolution]
---

# DEF-0006: SQL In expansion rejects ordinary large result sets

Claude reproduced native observations failing at 997 IDs and public In and
navigation failing near that size. OR per operand grows SQLite expression depth.
Large bound lists also reach variable limits. Expected: flat IN plus temporary
bound-set execution for oversized operands, preserving SQL semantics and schema.
Permanent TC-0031 large-set regressions cover memory/native/browser parity.
