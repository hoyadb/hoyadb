---
id: REQ-0039
title: Browser SQLite acknowledges only typed committed persistent state
type: requirement
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [OBJ-0001, OBJ-0002]
mitigates: [THR-0013]
tags: [proposal, storage, browser, sqlite, opfs, safety]
---

# REQ-0039: Proposed browser persistence safety

WHERE the proposed browser SQLite experiment is adopted after explicit runtime
dependency approval, THE adapter SHALL use the shared Rust storage semantics
through one dedicated worker and one active OPFS SQLite connection. It SHALL
validate owned boundary inputs and exact typed values before mutation, preserve
private-format ownership checks, and acknowledge writes only after the real
transaction commits. WHEN required capabilities, initialization, opening or
ordinary writes fail, THE adapter SHALL return an explicit error without a
memory fallback, database reset or partial-success acknowledgement. WHEN the
worker or page is restarted at the same origin, THE test harness SHALL verify
complete committed rows and failed-batch rollback from persistent storage before
recreating fixtures. Test cleanup SHALL be restricted to the namespace created
by the current harness.

SPEC-0023/TC-0023 propose bounded verification. This draft grants no dependency
approval and establishes no concurrent-client, power-loss, eviction, encryption,
hostile-file or full production storage-conformance guarantee.
