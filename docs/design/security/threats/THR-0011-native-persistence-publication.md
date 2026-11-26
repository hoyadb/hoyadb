---
id: THR-0011
title: Invalid SQLite rows or failed commits diverge native state
type: threat
status: identified
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [ARC-0002]
tags: [security, stride, sqlite, spike]
---

# THR-0011: Native persistence publication

STRIDE: tampered files can supply malformed types or duplicate IDs; failed
commits can expose memory/notifications inconsistent with disk; hostile file
size can exhaust resources. Plaintext disclosure remains THR-0002. No identity,
audit, privilege boundary or hostile-file sandbox is claimed.
REQ-0037 requires validation before handle publication, parameter binding, and
persist-before-publish for ordinary errors. SPEC-0021/TC-0021 verify synthetic
bounded fixtures; crash recovery and resource limits remain unproved.
