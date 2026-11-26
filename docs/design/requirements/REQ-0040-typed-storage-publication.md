---
id: REQ-0040
title: Typed storage rejects invalid inputs and publishes only committed changes
type: requirement
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [OBJ-0001, OBJ-0002, OBJ-0005]
mitigates: [THR-0014]
tags: [storage, query, validation, publication]
---

# REQ-0040: Typed storage publication

WHEN a caller uses the supplied-schema storage query or atomic-write capability,
THE adapter SHALL return only schema-valid complete owned rows or committed
before/after changes under API-0003's exact type/query semantics. IF validation,
execution or commit fails, THEN THE adapter SHALL return an explicit typed
error, no partial result/change vector and no partial mutation. SQL names and
operands SHALL remain bound values rather than executable SQL input.

Example: a typed batch replaces order A, then fails to insert existing order B;
A and B retain their original complete rows and no change vector is returned.
An Integer query operand for a Float field returns InvalidQuery, even on an
empty table or at limit zero. Raw API-0002 writes remain an unchecked internal
escape hatch; this requirement does not retrospectively validate them.
