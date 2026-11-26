---
id: DEF-0020
title: Native reactive benchmark identity names the wrong timed endpoint
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [TC-0037]
affects: [SPEC-0033, TC-0037]
regression_for: [TC-0037]
tags: [sdk, benchmark, metadata]
---

# DEF-0020: Native reactive benchmark identity names the wrong timed endpoint

## Source observation

benchmarks/run.py assigns serialize/facade-call/decoded-return to every warm
identity. In benchmarks/native/src/lib.rs, reactive_sample explicitly replaces
the primary sample value with last_callback_ns. It also records first_callback_ns,
return_to_first_callback_ns, return_to_last_callback_ns and dispatch_complete_ns.
The generic identity therefore mislabels the reactive primary statistic. The
actual clock captures remain available; no duration fabrication or failure of
the measured database behavior was found by this source audit.

This was found during a frozen full warm experiment. Its inputs and originals
must remain unchanged until that experiment finishes. A source-only peer review
confirmed that a separate derivation can faithfully name the already captured
endpoint. No such derivation has been implemented or executed yet.

## Intended correction and preservation

For future reactive cohorts, identify the primary endpoint as
caller-harness-invocation-to-last-callback. This is the raw facade/drain Rust
consumer closure, not a managed Swift/JavaScript callback or an SQLite commit
timestamp. Other operations retain their actual decoded-return endpoint.

Preserve original raw reports, summaries and hashes byte-for-byte. A separate
versioned analysis may derive the corrected endpoint for operation=reactivity
only, after validating the successful complete original experiment, source and
artifact identity, independent fixture proofs, protocol and all callback fields.
Require every primary value to equal last_callback_ns. Retain all warmups and
measured samples and record original-file hashes plus transform source/version.
Never pool the original generic endpoint and corrected callback endpoint as
though they were one already-labelled observation. Missing or inconsistent
captures fail derivation and require new measurements for the desired endpoint.
No arbitrary exclusions, workload/count changes or frozen-input edits.

## Test cases before implementation

1. Given a reactive cohort dispatched through the real runner, when its raw
   report is written, then its identity names invocation-to-last-callback and
   its primary values equal the captured last callback clocks. Observe the old
   generic label as the formal RED after the quiet window.
2. Given an original complete reactive report, when the versioned derivation
   runs, then only the endpoint identity changes; all samples and values remain
   identical, and provenance records the original bytes and transform identity.
3. Given an ordinary point/batch/cold report, when classification occurs, then
   its endpoint is not rewritten to a callback label.
4. Given missing, negative, non-integer or inconsistent callback fields, failed
   samples, incomplete runs, mixed identities or invalid fixture proof, when
   derivation is attempted, then it rejects without changing originals or
   producing an accepted summary. Synthetic fixtures prove validation only.
5. Given a fresh output location and preserved original summaries, when actual
   measured data is derived, then independent summary checks retain the complete
   declared count and no fabricated-clock input becomes performance evidence.

Rollback removes the unpublished derivation/future-label change; original raw
evidence remains available. No database or customer-store operation is involved.

## Implementation and evidence (2026-10-24)

Future label: `benchmarks/run.py` now names warm `operation=reactivity`
identities `caller-harness-invocation-to-last-callback`; every other warm,
cold and resource label and every captured clock field are unchanged. Test 1
is the opt-in actual regression `benchmarks/actual_reactive_endpoint.py`: the
real runner with memory cohorts 0 and 32, five release workers each, run
status `contaminated` and no summaries. Its formal RED failed only on the
generic label while all 175 cohort-32 samples already had value equal to
`last_callback_ns`, first callback at or before last and one callback; the
point cohort kept decoded-return. The same command passed after the label
change. Those outputs are functional calibration, not performance evidence.

Derivation: `benchmarks/derive_reactive_endpoint.py`, transform
`def-0020-reactive-endpoint/1`, covers tests 2–5 with
`benchmarks/test_reactive_derivation.py`. It accepts only an ok, quiet-attested,
source-stable original with five complete runs, exact 5+30 sample sequences,
nonnegative integer native callback captures, value equal to last callback,
callback count equal to observers, matching configuration/report identity,
initial and final proofs equal to the recomputed literal oracle and an
original summary equal to the independent recomputation. It writes only to a
new directory, changes only `identity.endpoint`, requires identical derived
statistics and records every original file hash plus the transform source
hash. Its compiling stub first failed all four tests; 21 rejection subcases
then each failed for their own reason. The interrupted ccf8ebe experiment and
the contaminated calibration outputs reject unchanged. Native callback
captures are nonnegative return-to-callback durations; the browser's signed
callback order is a separate class and is not derived here. No accepted
complete reactive original exists, so no derived performance dataset exists.
