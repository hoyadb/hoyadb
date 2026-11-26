---
id: DEF-0039
title: Equal-clock entity confirmation promotes a different pending value
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-27
updated: 2026-10-27
parents: [SPEC-0031, SPEC-0038]
affects: [REQ-0044, API-0009]
regression_for: [TC-0034, TC-0042]
tags: [purpose-audit, entity, reconciliation, authority]
---

# DEF-0039: Equal-clock confirmation promotes a different pending value

## Symptom and evidence

At ec895a8, static tracing by root and independent reviewer Opus e6 finds this
sequence: entity A writes f=A with client time 20; pending entity E later
writes f=E with the same client time 20. A receives Applied and is confirmed
with an empty C1 reconciliation. The current cell still contains E, but its
stamp and E's after-image stamp become authoritative, although the backend
has not accepted E's different value.

In the opposite order, E is Applied and confirmed first. E.before(f), which holds
A's value, and E.after(f) carry equal stamps, so `promote_metadata` skips f
entirely. The accepted current E therefore stays authoritative=false, a false
negative. A's images also stay false.

Checked in `crates/hoyadb-core/src/command/entity_owner.rs`: local fields take
`context.now_ms` directly with authoritative=false. Updating header.last_ms
with max does not make that field timestamp unique. In
`crates/hoyadb-core/src/command/reconcile.rs`, `promote_metadata` compares only
FieldStamp equality before setting authoritative=true, both in current
metadata and in other records' metadata undo images. It does not compare the
corresponding row cell's value or presence with the accepted after-image.

The intake mechanism was source-confirmed, then reproduced by assertion on
base 6ec725f for both confirmation orders and optional removal versus explicit
Null, on memory and SQLite. The mapped-create variant also failed on that base.
How far a merge hook or another consumer could propagate the original authority
error remains unverified. No data-loss or production-incident claim is made.

## Root cause

A FieldStamp contains a timestamp and authoritative flag, but does not bind a
value. Equal clocks for independent pending writes are legal. Stamp equality
alone therefore cannot prove that the current cell is the accepted write.
REQ-0044 requires preserving backend authority; API-0009 limits promotion to
the accepted optimistic write.

## Required correction

Make confirmation promotion value-aware. Proposed normative text: SPEC-0038 C1a.
- **Candidates.** Only the accepted write's declared `args.changes` keys and
  `removed_fields` are candidates. Explicitly corrected fields are excluded and
  keep their selected stamps.
- **Match.** Promote a field only when both hold: its paired row cell exactly
  equals the accepted after-image (same value, or both absent; explicit Null is
  not absence), and its stamp is present with the same timestamp. The
  authoritative flag is ignored in the comparison, so a correct match is not
  missed (no false negative). A missing stamp is never synthesized, not even as
  timestamp 0. A present stamp with timestamp 0 is valid: it is preserved and
  eligible for promotion.
- **Targets.** Apply the same check to the current row and metadata, and to each
  other record's paired row-undo and metadata-undo images (before and after).
- **Delete.** A declared Delete promotes only a matching tombstone where the
  paired row is absent.
- **Preserved.** Explicit Null versus absence, and all frozen send bytes.

Both orders are defects: a false positive when A confirms first, and a false
negative when E confirms first. Required results:
- **A confirms first.** E.before(f), holding A's value, becomes authoritative,
  while current f and E.after(f), holding E's different equal-clock value, stay
  authoritative=false.
- **E confirms first.** Current f, holding E's value, becomes authoritative, while
  A's undo images holding A's value stay authoritative=false. A later
  confirmation of A then promotes nothing on f.

Authority flags already written by the old rule in reconciled stores are not
repaired automatically: no migration or demotion without evidence.

The proposed O2/C4 recompute must also apply backend selections and promotions
only while building eff0, then recompute later intents last. It must not run
ordinary correction or promotion again over the recomputed row or images.
That integration rule is separate from reproducing and fixing this existing C1
promotion defect.

## Regression gap

TC-0034 covers typed backend correction and authority boundaries; TC-0042 C1
covers confirmation under later edits. Neither currently enumerates an
accepted value and a different pending value with the same field clock.

Before implementation, add reviewed cases. They are proposed as TC-0042 C1a-A,
C1a-B, C1a-SAME, C1a-NULL, C1a-CORRECTED, C1a-DELETE and C1a-CTRL. They cover:
- equal-clock different values in both confirmation orders;
- declared same-value equal clocks (a pure planner case if the public API elides
  them);
- optional removal versus explicit Null;
- corrected fields retaining their selected stamps;
- delete tombstones and distinct-clock controls;
- exact pending undo metadata, on both adapters, with SQLite restart.

After C1 of A, E.before(f) is authoritative=true, and current f and E.after(f) stay
authoritative=false. When E confirms first, the expected RED is that unchanged code
skips f and leaves the accepted current E authoritative=false. Under the fix,
current E becomes true and A's different-valued images stay false, as an
accompanying control. A later confirmation of A changes nothing on f. A
present-zero versus missing-stamp control is included. Run the expected assertion
RED on unchanged code, then the smallest value-aware fix. C1a is fixed before the
O2/C4 slice.
No missing symbol or fixture error counts as RED.

## Status and rollback

The C1a correction is implemented in 1b4624e, following reviewed cases in
21afca7 and the unique paired-image clarification in 1774ba0. Component review
approved the value-aware candidate and the defensive pairing guards. The latter
have separate assertion REDs on the first C1a implementation; they do not claim
additional user-reachable symptoms of the original defect.

The exact committed candidate passed 312 selected tests under the default
parallel runner, with formatting and both feature-on and feature-off Clippy
checks clean and source hashes unchanged. Tests cover both adapters, exact undo
images, mapped identities, Null versus absence, present-zero and missing stamps,
no retroactive repair, and fresh SQLite connections for every SQLite promotion case.
The final combined remediation run and three clean final reviews remain pending;
component approval does not advance that counter.

There is no store migration or new persisted shape. Revert the C1a source and
regression tests together to restore the earlier rule for future confirmations.
Authority flags already written under C1a are neither repaired nor demoted by
that revert. No user store was opened or modified. Diagnostic evidence and
existing user work are preserved.
C4 remains a separate implementation slice, and G1 is not claimed as resolved.
