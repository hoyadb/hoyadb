---
id: THR-0020
title: General bindings stale callbacks or target crypto drift expose or corrupt local recovery
type: threat
status: mitigation-planned
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [ARC-0001, ARC-0002]
tags: [phase-seven, security, ffi, stride, linddun]
---

# THR-0020: General bindings stale callbacks or target crypto drift expose or corrupt local recovery

Trust boundaries: untrusted foreign request bytes/configuration→Rust types;
borrowed native/JNI memory→owned core scope; core state→awaited host merge/command
handler; supplied binary key→target cipher; durable events→UI executor; package
build/source graph→consumer binary. Domain rows in explicit application APIs
remain application data, never safe telemetry or arbitrary diagnostic export.

| STRIDE | Concrete control |
| --- | --- |
| Spoofing | Owner/scope/session/generation checks; no native pointer exposed as handle; backend authenticates transport independently. |
| Tampering | Exact schema/value/query codecs, unchanged core validation, staged custom-proposal checks and complete expected-state guard; authenticated cipher/profile and entropy failure tests on each target. |
| Repudiation | No claim client clocks/context prove server authorship; no hidden or automatic diagnostic egress. |
| Information disclosure | Separate binary key channel, static errors/redacted Debug, no raw JNI/worker errors, memory temps and platform backup capability; test canaries in actual artifacts/sidecars. |
| Denial of service | Bounded frames/nodes/depth, owned finite handles and cancellation, bounded owned prefixes without event loss, no borrow/lock across await, no busy-spin retry or entropy clock fallback. |
| Elevation of privilege | No SQL/extensions/import/rekey or arbitrary path deletion; app-private canonical roots; no unreviewed native dependencies or private Android crypto linkage. |

LINDDUN: identifiers, schemas, scopes, clocks and explicit merge inputs can reveal
application data to its own host hook; do not confuse intentional application
access with safe diagnostics. Aggregate counts reveal volume. Key/cipher cannot
protect a compromised running app or caller logging/copies; OS backup controls
have stated limits. Multi-process same-store coordination is not promised.

REQ-0046/SPEC-0033/API-0011 and TC-0036 mitigate this extension of THR-0019. Preserve
the approved engine scheme; no private cipher/weak-RNG/native plaintext fallback.
Target-only archive selection, source/lock/license manifest and actual independent
consumer runs are part of the security boundary, not paperwork-only claims.

C ABI safety precondition: valid readable nonempty spans/callback lifetime are
caller obligations. Null/length/UTF8 checks and Rust panic containment do not
validate arbitrary pointers or catch foreign memory faults; scoped JNI/Swift
buffers satisfy the obligation. No memory-safety claim is made for forged addresses.

Browser byte-fault evidence uses a default-off test feature with bounded binary
transfers through the pool's supported closed-file API. Enforce its reserved
task-owned logical-name prefix and reject separators/NUL; never clear a shared
SDK directory. Ordinary package export checks must prove the controls absent.
This harness cannot establish a production import/backup API or authorize
inspection or deletion of user stores.
