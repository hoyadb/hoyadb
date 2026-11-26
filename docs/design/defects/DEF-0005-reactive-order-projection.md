---
id: DEF-0005
title: Strict projected order cells break observation query parity
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [REQ-0042, SPEC-0027]
depends_on: [API-0005, THR-0016]
tags: [reactivity, regression]
---

# DEF-0005: Phase 3 review finding

Review found that the uncommitted key cache requested its ordering field as a
strict application cell. API-0003 sorts a wrong-kind related ordering cell as
Null, so fetch succeeds while observe_count returned InvalidRow. Expected:
project a separate normalized ordering key, preserving strict validation for
explicitly requested cells. Both roles must still reject malformed consulted
physical payload. TC-0030 permanently compares fetch and observation with a
wrong-kind target cell and verifies strict requested-cell behavior. No storage
format or runtime dependency changes.
