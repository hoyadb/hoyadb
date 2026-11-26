---
id: REQ-0043
title: Command recovery preserves ownership committed data and retry identity
type: requirement
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [OBJ-0002, OBJ-0003]
mitigates: [THR-0017]
tags: [phase-four, commands, integrity]
---

# REQ-0043: Command recovery integrity

WHEN a command is staged, THE system SHALL isolate its writes and validate scope
ownership/liveness before every operation. Failure/abort SHALL discard staging.
WHEN a command commits on a transactional engine, THE system SHALL atomically
persist its local mutations, envelope and required recovery/dependency state.
WHEN retrying a sent command, THE system SHALL reuse its exact frozen envelope
and key. Receipt SHALL stop retry without losing later-outcome recovery state.
Rollback/correction SHALL NOT overwrite unrelated later changes, and unsupported
schema/reset/recovery states SHALL reject without deleting local or pending work.
All external payload decoding SHALL be bounded and all callbacks/messages owned;
diagnostics SHALL NOT expose identity, arguments, undo images or row contents.

RFD-0003 proposes the exact lifecycle and conflict choices. This draft requirement
does not assert those policies were approved or any Phase 4 implementation exists.
