---
id: DEF-0007
title: Explicit observation retry failures become silent
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [REQ-0042, SPEC-0027]
depends_on: [API-0005, THR-0016]
tags: [reactivity, review-resolution]
---

# DEF-0007: Explicit observation retry failures become silent

publish queues error only when a group first becomes dirty. Repeated explicit
refresh returns no result and queues no error; throttled successful recovery
can also produce empty drain, making the state ambiguous. Expected: every failed
explicit refresh produces a typed error; successful clean refresh stays silent.
TC-0031 pins retry failure and retained count deadline.
