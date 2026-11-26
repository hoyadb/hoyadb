---
id: REQ-0045
title: Protected opens fail closed and diagnostics disclose no protected contents
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [OBJ-0006]
mitigates: [THR-0019]
tags: [security, security, confidentiality]
---

# REQ-0045: Protected opens fail closed and diagnostics disclose no protected contents

WHERE protected storage is requested, the system SHALL require externally supplied
key material, authenticate before schema/data access, and reject absent, invalid,
wrong, unsupported or incompatible key/engine/format inputs without plaintext
fallback, implicit conversion/reset or partial data application.

The system SHALL keep key configuration below generic storage traits; it SHALL
NOT generate, persist or manage application keys or implement custom encryption.
Owned transient key buffers SHALL use vetted zeroization, with foreign/runtime
copy and compromised-process limitations stated honestly.

The system SHALL bound diagnostic output to an explicit counts/capabilities
allowlist and SHALL omit keys, identities, payloads, outcomes/reasons, names/IDs,
paths, namespaces, scopes and checkpoints. Complete local retry data SHALL remain
unchanged. No automatic diagnostic egress or library network path is permitted.

Backup capability SHALL distinguish applied OS hints from unsupported platforms;
unsupported or unverified controls SHALL NOT be reported as effective protection.

## Verification status
