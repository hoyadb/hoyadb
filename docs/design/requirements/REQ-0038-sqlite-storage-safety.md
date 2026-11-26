---
id: REQ-0038
title: Native SQLite storage preserves typed rows and rolls back ordinary failures
type: requirement
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [OBJ-0001, OBJ-0002]
mitigates: [THR-0012]
tags: [storage, sqlite, safety, foundation]
---

# REQ-0038: SQLite storage safety

THE native SQLite adapter SHALL represent logical names and values as bound
SQL values, preserve the exact API-0002 typed subset, and reject incompatible
stored field types rather than returning coerced or partial rows. WHEN an
ordinary operational write or commit error occurs, THE adapter SHALL return
BackendFailure and roll back the whole single mutation or ordered batch.
WHEN an existing table or row violates a domain precondition, THE adapter SHALL
preserve API-0002's exact typed error and original coordinates. Opening failure
SHALL be explicit without memory fallback or automatic database reset. THE
adapter SHALL initialize its versioned private format only for a new or empty
database and reject unrelated or incompatible existing files before modifying
their schema or data.

SPEC-0022/TC-0022 verify this initial internal subset. It does not establish
full schema/query conformance, encryption, crash-tested queue durability or a
production hostile-file boundary.
