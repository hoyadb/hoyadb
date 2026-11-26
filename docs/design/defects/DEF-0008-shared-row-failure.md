---
id: DEF-0008
title: Row materialization failure invalidates count observers
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [REQ-0042, SPEC-0027]
depends_on: [API-0005, THR-0016]
tags: [reactivity, review-resolution]
---

# DEF-0008: Row materialization failure invalidates count observers

A group refresh conflates candidate computation and full-row delivery. A row
read failure dirties every listener even when its projected count is valid.
Expected: commit valid shared keys/counts, retain row-only retry state, report
row failures to affected row listeners, recover exact latest rows independently.
TC-0031 covers mixed modes, actual SQL decoder failure and host error delivery.
