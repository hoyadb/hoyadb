---
id: THR-0009
title: Native pointer or ownership misuse corrupts the host process
type: threat
status: identified
owner: contact@btk-consulting.fr
created: 2026-10-20
updated: 2026-10-20
parents: [ARC-0002]
tags: [security, stride, ffi]
---

# THR-0009: Native pointer or ownership misuse

The C ABI introduces foreign input and borrowed memory. A buggy caller can pass
stale pointers, retain callback bytes, reuse disposed stores, or re-enter while
Rust holds an exclusive borrow. Consequences include corruption or process failure.

| STRIDE category | Assessment |
| --- | --- |
| Spoofing | Handles provide no identity or authorization claim. |
| Tampering | Invalid inputs or aliasing can corrupt process state. |
| Repudiation | No audit guarantee in the synthetic harness. |
| Information disclosure | Invalid reads may expose process memory. |
| Denial of service | Invalid pointers, panic or unbounded recursion can crash. |
| Elevation of privilege | No new privilege; memory corruption remains a risk. |

REQ-0035 proposes validation, owned copies, explicit lifetimes and borrow release;
SPEC-0018 specifies them and TC-0018 tests bounded valid-memory cases. Invalid
arbitrary pointers cannot be made safe by null/UTF-8 checks; caller preconditions
and focused unsafe-code review remain necessary. No personal data is used.
This threat remains identified until review and execution provide evidence;
it is not a claim of production ASVS compliance or complete mitigation.
