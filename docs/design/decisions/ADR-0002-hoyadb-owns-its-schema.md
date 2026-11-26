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

Source: `legacy/refined-vision/02-data-model/alternatives.md`, forked into
`legacy/docs/03-data-model/schema.md` and `legacy/docs/03-data-model/relations.md`.

## Context and problem statement

`legacy/NEEDS.md`'s "Data model" section states a preference for a
"relational-ish" model over pure document or key-value, while leaving the
exact shape undecided. The choice determines whether relations are a
first-class construct, whether query-result-level reactivity
(`OBJ-0005`) is achievable at all, and whether hoyadb ends up coupled to a
specific backend database technology before its storage engine and sync
protocol are even settled.

## Considered options

- **hoyadb owns a declared relational schema** — tables, typed columns, and
  to-one/has-many/many-to-many relations as first-class schema constructs
  in hoyadb's own schema language.
- **Document-oriented with relations layered on afterward** — RxDB's
  schema-based-but-document-oriented approach, with "population"/relations
  bolted on top.
- **Fully schemaless** — PouchDB's approach.
- **Map the local schema onto an existing external database** — Zero,
  PowerSync and ElectricSQL all map onto or read an existing Postgres
  schema unmodified.

## Decision outcome

In the context of defining hoyadb's data model, facing `legacy/NEEDS.md`'s
relational-ish preference and the dependency of query-level reactivity on
owning a query layer, we decided for **hoyadb owning its own declared
relational schema with first-class relations** and against a
document-oriented model with relations bolted on, a fully schemaless model,
and mapping onto an existing external database, to achieve first-class
relations and the query-result-level reactivity `OBJ-0005` needs, accepting
that hoyadb must now design, version and migrate a schema language of its
own rather than inheriting one.

### Consequences

**Good**
- Relations are declared constructs with their own load behaviour, not a
  convention an app hand-builds from filtered queries. hoyadb's position
  combines WatermelonDB's "own the schema" with Realm/Zero's richer native
  relations: all three relation kinds are first-class from the start.
- Owning the query layer is what positions hoyadb to do better than the
  coarse whole-table change feed every schemaless reference system falls
  back to. The reference research found a direct correlation between having
  no owned schema/query layer and lacking query-result-level reactivity.
- No hard coupling to one backend database technology.

**Bad**
- hoyadb must own schema evolution itself — hence `ADR-0009`'s migration
  policy, which would not exist under a schemaless model.
- The exact built-in column-type set, and whether app-defined custom
  types/validators are supported, is **not fixed** — implementation-time
  detail, per `legacy/docs/03-data-model/schema.md`'s "What's still open".
- Whether a many-to-many join table is always app-visible and app-queryable
  or can be an implicit hoyadb-managed table is **not decided**
  (`legacy/docs/03-data-model/relations.md`).
- **Indexing is not decided** anywhere in this tree — a real, disclosed
  gap, tracked as `RISK-0004`.

### Confirmation

Confirmed by `SPEC-0003` (schema declaration and relations), `DM-0001`, and
the schema/relation test cases in `TP-0001`.

## Pros and cons of the options

### hoyadb owns its schema (chosen)
- Good: first-class relations; enables query-level reactivity; no backend
  coupling.
- Bad: hoyadb owns migrations, the type catalogue, and indexing.

### Document-oriented, relations layered on
- Bad: works against `legacy/NEEDS.md`'s explicit relational-ish preference, and
  treats relations as an add-on rather than a first-class concept.

### Fully schemaless
- Bad: rejected outright — correlates with losing query-result-level
  reactivity, which would put `OBJ-0005` at risk.

### Map onto an existing external database
- Good: no schema language to design or migrate.
- Bad: hard-couples hoyadb to one backend database technology before the
  storage engine and sync protocol are even settled, against `legacy/NEEDS.md`'s
  generic-core goal.

## Traceability

- Parents: `OBJ-0002`, `OBJ-0005`.
- Referenced by: `ARC-0001`, `SPEC-0003`, `DM-0001`.
- Risk: `RISK-0004`.
