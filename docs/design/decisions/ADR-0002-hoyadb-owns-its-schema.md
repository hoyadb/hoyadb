---
id: ADR-0002
title: Data model — hoyadb owns its own relational schema
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002, OBJ-0005]
tags: [data-model, schema, relations]
---

# ADR-0002: Data model — hoyadb owns its own relational schema

## Decision

HoyaDB owns a declared relational schema with first-class to-one, has-many and
many-to-many relations. Owning the schema and query layer supports typed
validation, relation traversal and query-result observation without coupling the
local model to a backend database technology.

## Consequences

HoyaDB must define and version its schema language and apply explicit migration
policy. SPEC-0003 and DM-0001 define the model; API-0004 defines the Rust surface.
ADR-0009 governs migration/reset behavior, and ADR-0030 governs indexing. Runtime
support must be established by the applicable conformance tests.
