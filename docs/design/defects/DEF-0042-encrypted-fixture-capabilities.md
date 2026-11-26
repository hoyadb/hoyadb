---
id: DEF-0042
title: Encrypted conformance fixture hides native adapter capabilities
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-29
updated: 2026-10-29
parents: [SPEC-0044]
affects: [SPEC-0037, NFR-0006]
regression_for: [TC-0048]
tags: [testing, encrypted-storage, conformance]
---

# DEF-0042: Encrypted fixture capability mismatch

## Symptom and root cause

Opus 5.5 identified that the encrypted conformance `Fixture` in
`crates/hoyadb-storage-sqlite/tests/security_engine_cases.rs` forwards storage
operations but inherits the default `ownership`, `reads_ready`, and
`query_validated` implementations. Native SQLite overrides all three. The
wrapped scenarios therefore miss the production exclusive/read-ready/validated
path while presenting themselves as identical public conformance tests.

## Fix and regression

Forwarded all three overrides without changing production code. TC-0048 QUALITY08
asserts native capability parity and execution of the concrete validated path.
Keep the existing conservative/false-capability suites. The capability assertion failed with `Unproven` versus `Exclusive` on the
original wrapper. All 55 encrypted conformance tests passed after forwarding was
restored. A fixture counter verifies entry into the validated override and its
exact row result; it does not measure SQLite's internal bounded-walk cost.
