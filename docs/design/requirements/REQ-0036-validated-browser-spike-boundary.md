---
id: REQ-0036
title: The browser spike preserves integer precision and snapshot ownership
type: requirement
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [OBJ-0002]
mitigates: [THR-0010]
tags: [spike, browser, safety]
---

# REQ-0036: Validated browser spike boundary

WHEN JavaScript inserts a total, THE bridge SHALL preserve every signed i64
value exactly and reject out-of-range inputs before mutating the store.
THE harness SHALL release owned store/row/event snapshots, render fixture values
as text, and invoke subscription callbacks only after the WASM mutable drain
has returned so that callbacks can re-enter for reads without retained borrows.
SPEC-0017 B03–B05 verify the experimental subset, not a public production API.
