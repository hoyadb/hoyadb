---
id: ADR-0038
title: Use stable metadata identities for new entity ledgers with explicit offline conversion
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [OBJ-0002, OBJ-0003]
depends_on: [RFD-0007, DEF-0034, API-0007, API-0009]
tags: [purpose-audit, sync, storage, compatibility]
---

# ADR-0038: Stable identities for new entity metadata

## Context and problem statement

DEF-0034 records complete-ledger serialization and replacement on narrow sync
commits. Even after redundant work is removed, format-2 metadata uses sorted
contiguous ordinals: arbitrary-order insertion renumbers a suffix. Fixed-page
bootstrap therefore retains quadratic worst-case work. RFD-0007 separates a
new format from conversion of existing stores.

## Considered options

- Stable metadata identities for new entity ledgers and a reviewed offline
  forward/reverse conversion tool.
- Keep format 2 everywhere, accepting arbitrary-order suffix rewriting and
  quadratic worst-case bootstrap.
- Automatic conversion on open: rejected because ordinary open must not change
  a user's persistent compatibility boundary.

## Decision outcome

In the context of paged entity sync, facing metadata renumbering proportional
to unrelated stored entries, we decided for stable bounded identities in new
entity ledgers and explicit offline reversible conversion, and against retaining
ordinal identities for all new ledgers or converting on ordinary open, to achieve
change-proportional metadata persistence, accepting a new-reader requirement for
new-format state and an explicit maintenance step before downgrade.

The operator selected "Approve new-ledger format and offline converter
(Recommended)" in response to RFD-0007 F-A. This approves building and testing
the mechanism; it authorizes no conversion of an actual user's store.

## Consequences

- First entity configuration with no prior entity-format state uses the new
  reviewed format. This includes an existing command-only store newly adopting
  entity sync. Older binaries require reverse conversion before reading that
  new entity state.
- Existing format-2 entity stores retain their format on open, reset and
  rebind. Their arbitrary-order suffix-rewrite bound stays explicit.
- Exact markers, injective table/id encoding and bounds require their own
  reviewed spec and failing cases before implementation. Mixed, malformed and
  unknown formats reject. No unapproved SHA dependency enters core.
- The converter is an offline tool, with dry-run, full validation, atomicity
  and forward/reverse fixture tests. It is not an automatic SDK upgrade API.
  An operator or integrating developer may run it only in a separately
  authorized, backed-up maintenance operation without a live owner.
- Exclusive ownership, complete validation on command-capable open, touched
  state validation and exact prior-image guards remain integrity preconditions.
  Custom or browser adapters retain conservative behavior until their required
  capabilities are demonstrated.

## Confirmation and rollback

Physical counters must measure rows and metadata actually visited, cloned,
parsed, encoded and written, rather than trait calls. New-format arbitrary-order
page work must depend on its changes. Legacy-format limits remain visible.
Memory atomicity must not be obtained by hidden full-store candidate copies on
the claimed fast path. Open, reset and conversion intentionally visit total data.

This decision supplies no execution evidence. An older binary cannot read new
entity-format state directly; rollback requires the independently reviewed
reverse converter. No automatic reset, migration or data deletion is permitted.
