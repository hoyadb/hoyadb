---
id: THR-0013
title: Browser storage coercion or initialization loses persistent SQLite state
type: threat
status: identified
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [ARC-0002]
tags: [security, stride, browser, sqlite, opfs, persistence]
---

# THR-0013: Browser worker and OPFS persistence boundary

STRIDE: unvalidated worker messages, stale handles, integer coercion or modified
origin storage can tamper with typed rows. Failed VFS initialization followed by
memory fallback can falsely acknowledge persistence; wrong VFS configuration
or reset-on-open can destroy prior state. Concurrent owners or abandoned sync
handles can deny service. Same-origin code already has access to origin data;
the experiment adds no authentication or isolation against hostile same-origin
scripts, no audit/nonrepudiation guarantee and no privilege elevation API.
Plaintext disclosure remains THR-0002; this is not encrypted storage.

REQ-0039 proposes checked owned inputs, one worker/connection, explicit
failure without fallback/reset, preserved private-format validation, transaction
commit acknowledgement and observed restart tests. SPEC-0023/TC-0023 define
bounded synthetic cases with the operator-approved graph; implementation review
and executed mitigation evidence remain pending.
Runtime/browser restrictions, resource limits, multi-tab coordination, eviction,
power loss, hostile-file protection and at-rest security remain unproved.
No production mitigation claim follows from this preparation. Test cleanup must
never delete unrelated origin storage or retain personal data in diagnostics.
