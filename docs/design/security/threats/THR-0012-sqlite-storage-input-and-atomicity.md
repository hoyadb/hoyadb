---
id: THR-0012
title: SQLite storage input or operational errors corrupt typed rows
type: threat
status: identified
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [ARC-0001]
tags: [security, stride, sqlite, storage]
---

# THR-0012: SQLite values, decoding and failed writes

STRIDE: interpolated logical names or values can tamper with unrelated state;
modified database bytes can supply incompatible types or impersonate an owned
format; automatic initialization can alter an unrelated file; multi-statement write
failures can expose partially replaced rows or batch prefixes. Database locks
and large or hostile files can deny service. Plaintext disclosure remains
THR-0002; there is no authentication, audit, privilege or hostile-file sandbox
claim in this local foundation.

REQ-0038 requires ownership/version checks before initialization, bound values,
strict typed decoding and real transactions
with explicit operational errors. SPEC-0022/TC-0022 exercise bounded synthetic
fixtures for these mitigations. Encryption, resource limits, crash/power-loss
recovery and shared concurrent clients remain unproved. Diagnostic errors must
not cause the adapter to emit row contents or telemetry.
