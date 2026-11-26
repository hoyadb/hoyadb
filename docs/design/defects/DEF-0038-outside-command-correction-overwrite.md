---
id: DEF-0038
title: A correction can overwrite a later local edit outside the command write set
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [SPEC-0029]
affects: [REQ-0043, API-0006]
regression_for: [TC-0032, TC-0042]
tags: [purpose-audit, corrections, integrity]
---

# DEF-0038: Corrections outside the command write set

## Symptom and evidence

Static tracing by Opus e6 and root finds this sequence possible in the current
reconciliation planner: A writes row r; a later direct local write edits unrelated
row s; A receives Corrected; the application's correction plan Puts or Removes s.
The closure proof validates A's rows, then the write loop changes s without an
after-image proof for s. Its later local value can be overwritten. REQ-0043 says
corrections must not overwrite unrelated later changes.

Checked in `crates/hoyadb-core/src/command/reconcile.rs`: `closure` and
`simulate_inverse` check recorded undo identities; the subsequent Put/Remove
loop validates schema and identifiers but imposes no prior-value guard on an
existing row outside that closure. Root then reproduced the overwrite through
the public memory owner using an
isolated diagnostic at `target/purpose-audit-20261026/repeated-edit/src/bin/outside_correction.rs`:
`cargo run --manifest-path target/purpose-audit-20261026/repeated-edit/Cargo.toml
--bin outside_correction --offline --locked` exited 0 and printed
`out-of-command correction: Ok(())`; row s contained the server correction rather
than its later local edit. `outside-correction-run.json` binds the harness, core,
memory sources and log, with all sources unchanged during the run. This diagnostic
asserts the bad observed behavior; it is not the desired acceptance regression
or a production incident.

Root also reproduced a typed Entity-write variant after configuring entity
sync: a later entity replacement of s queues its own optimistic intent, but A's
typed correction to s still returns Ok and overwrites that value. The isolated
`outside_entity_correction` diagnostic exited 0 with all core, memory and
harness sources unchanged. Its fresh build, executable hash, exact source copy
and output are bound in
`target/purpose-audit-20261026/outside-entity-correction/identity.json`.
This is the same unguarded cross-row defect, so the fix must cover ordinary
Put/Remove and typed Entity write routes, rather than only one write variant.

## Root cause

The correction API supplies desired writes without an explicit expected image
for unrelated rows. Proving the originating command's undo does not prove those
other rows have not changed later. A server-side effect on another row is a
legitimate use case, so silently treating arbitrary plan writes as a safe local
overwrite is insufficient.

## Required correction and test gap

Define a reviewed guarded path for existing rows outside the command write set,
or reject an unverifiable overwrite explicitly. Do not silently skip a server
correction or claim convergence. Preserve atomic mapping/data/ledger behavior.
The exact contract and compatibility change require review before regression
tests and implementation.

No desired-behavior regression was found in this review for this scenario. TC-0042
CTRL-K records the existing behavior as characterization, not desired acceptance.
Add a behavioral RED covering Put and Remove, direct edits and unrelated-command
edits, unchanged rows, mapping atomicity, and both memory and SQLite. Do not
weaken REQ-0043 or close this intake merely by documenting the overwrite.

## Status and rollback

Open. No fix is implemented. The operator's request includes concrete findings
discovered during remediation. Preserve all source cuts and failing/passing
evidence; no user store is opened, converted, reset or deleted. Any API change
must carry its rollback and consumer compatibility policy.
