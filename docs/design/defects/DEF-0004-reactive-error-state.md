---
id: DEF-0004
title: Reactive errors retain stale count state and duplicate caches
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [REQ-0042, SPEC-0027]
depends_on: [API-0005, THR-0016]
tags: [reactivity, regression]
---

# DEF-0004: Phase 3 review finding

Review found that an uncommitted dirty group retained a pending count from a
prior commit; equivalent registration could allocate a second cache; clearing
last delivery on error then bypassed the 16 ms interval. These violate API-0005,
SPEC-0027 and THR-0016. Expected: discard stale pending counts, refuse equivalent
registration until explicit recovery, retain delivery time across errors, and
republish the exact recovered value even if equal to the last successful count.
Keep value validity separate from the monotonic delivery deadline. TC-0030
covers these partitions permanently. No shipped store format is changed.
