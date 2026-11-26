---
source: https://vlcn.io/docs/cr-sqlite/crdts/table-crdts
source_type: doc-site
fetched: 2026-09-13
---

# cr-sqlite — Overview: turning tables into CRDTs

Observed, not accepted — this describes cr-sqlite, not hoyadb.

**Note on source quality**: this fetch returned thinner detail than most
other units in this reference set — several specifics (the exact
`crsql_as_crr` syntax, a full definition of "CRR") were referenced but not
spelled out in what was retrieved. Recorded honestly below; not filled in
with invented specifics.

## What's confirmed

- A regular SQLite table is converted to a CRDT-backed table (a "CRR" —
  the term is used but not fully defined in what was fetched) via a
  function referenced as `crsql_as_crr` (exact call syntax not confirmed
  in this pass).
- **Per-table CRDT type choice controls merge semantics**: "the type of
  CRDT chosen for a table controls what rows are included or excluded when
  merging tables together from different databases" — i.e. this is a
  per-table setting, not one fixed global merge rule.
- **Default table CRDT type is a "causal length set"**, whose defining
  behavior is explicitly named: "deleted rows can later be re-inserted if
  desired" — notably different from every delete-handling rule seen
  elsewhere in this reference set (WatermelonDB/PowerSync/Realm's
  historical OT all treat delete as a terminal, winning state).

## Not confirmed in this pass

- Exact conversion syntax/command.
- Full definition of "CRR."
- How ordinary SQL reads/writes continue to work unmodified after
  conversion (implied by the existence of standard SQL-compatible API
  methods elsewhere in the docs, but not explicitly stated on this page).

## Relevance to hoyadb

The causal-length-set default's "deletes are not necessarily terminal, a
later re-insert can resurrect a row" is a real, structural alternative to
every other reference source's "deletes always win" convention — worth
flagging as a genuinely different design point for hoyadb's own delete
semantics, even though the detail here is thinner than ideal. Per-table
CRDT type choice (rather than one global merge rule) is also worth noting
as a pattern: hoyadb's own conflict/merge strategy need not be uniform
across every table if it doesn't want it to be.
