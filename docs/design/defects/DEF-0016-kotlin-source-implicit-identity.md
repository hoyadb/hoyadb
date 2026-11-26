---
id: DEF-0016
title: Kotlin relation source implicitly formats its row identity
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, THR-0020, TC-0036]
affects: [REQ-0046, SPEC-0033, THR-0020]
regression_for: [TC-0036]
tags: [sdk, kotlin, privacy, formatting]
---

# DEF-0016: Kotlin relation source implicitly formats its row identity

## Source observation

The uncommitted Kotlin SDK foundation declares Source as a data class holding
table, id and relation in bindings/android/src/main/kotlin/dev/hoyadb/Models.kt.
It has no toString override. The automatically generated representation includes
those properties, including the caller's domain row identity. Query itself and
the other row-bearing DTOs already use opaque representations. The reviewed
bindings/android/PLAN.md explicitly excludes domain payloads from implicit SDK
toString/error/diagnostic formatting; explicit data APIs remain data access.

This was found by source inspection during the native benchmark quiet window.
No runtime canary has been executed for this finding yet. The formal regression
must establish that behavior after this preparation is reviewed and committed.
It is not evidence of an observed external log disclosure.

## Intended correction and rollback

Give Source an opaque toString representation while preserving its public
properties, data-class equality/hash/copy and exact query-source wire encoding.
Do not remove explicit row-ID access or alter query semantics. No core, wire,
dependency or persistent-store change. Revert the unpublished source change to
roll back; no customer data is touched.

## Regression owed before implementation

TC-0036 G01/G24 must cover implicit formatting of a Source containing a synthetic
identity canary, alone and nested in a collection. It must fail for the generated
identity-bearing representation, then pass with an opaque representation. The
same test must independently assert that explicit id access, data-class copy,
equality/hash and Query.toProtocolValue preserve the exact ID. Existing model
and real-owner tests remain required. Prior six-model-test GREEN evidence
predates this additional regression and stays historical.
