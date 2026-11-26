---
id: THR-0010
title: Browser coercion or stale ownership changes spike data
type: threat
status: identified
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [ARC-0002]
tags: [security, stride, wasm]
---

# THR-0010: Browser boundary coercion and ownership

JavaScript numbers can lose integer precision and generated i64 bindings may
wrap out-of-range BigInts. Reusing disposed objects or holding a mutable Rust
borrow across host callbacks can also fail the host process or corrupt results.

STRIDE: tampering through numeric coercion; denial of service through invalid
lifecycle calls; disclosure through accidental stale ownership. No identity,
audit, personal-data or privilege boundary is claimed by this synthetic spike.
REQ-0036 requires exact decimal parsing, owned snapshots and callback dispatch
after drain returns. SPEC-0017 B03–B05 exercise bounded cases. Resource limits,
malicious same-origin code and production isolation are not established.
