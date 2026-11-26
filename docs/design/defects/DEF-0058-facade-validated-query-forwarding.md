---
id: DEF-0058
title: Facade engine drops the validated query path
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-31
updated: 2026-10-31
parents: [SPEC-0047]
affects: [SPEC-0037, API-0011, NFR-0001]
regression_for: [TC-0041, TC-0051]
tags: [bindings, performance, forwarding]
---

# DEF-0058: Facade engine drops validated query forwarding

## Symptom and confirmed source cause

The facade's `Engine` forwards `TypedStorage::query` but does not override
`query_validated` (`crates/hoyadb-bindings/src/engine.rs`). The provided default
in `crates/hoyadb-core/src/storage/typed.rs` calls ordinary `query`, dropping
the proof supplied by a strictly opened Exclusive `CommandDatabase`. Both
concrete adapters implement the SPEC-0037 R8 bounded ordered path, but facade
calls cannot reach it through this wrapper. Result values remain correct;
the approved work bounds are lost at the binding boundary.

## Regression and acceptance

Before changing the forwarding implementation, add a public facade regression
at N=10 and N=1000 with a fixed limit and direct required order column. Verify
exact rows and ascending-ID tie order, bounded memory row visits and zero
SQLite sort/fullscan with equal bounded row reads/VM work. Test instrumentation
is already enabled only by the bindings' existing work-counters test feature.
Keep ordinary/raw query validation unchanged. TC-0041's adapter cases did not
cover the facade wrapper; this adds that boundary, not a new query algorithm.

## Correction and verification (2026-10-31)

The public facade regression compiled and failed twice before implementation:
memory visited 10 rows for a limit of 3, and the SQLite ordered query performed
one sort. The tests also checked exact rows before their counter assertions.
The source freeze for the policy artifact checkpoint then completed.

`Engine::query_validated` now forwards the existing opaque proof to the concrete
adapter, using the same ready-future forwarding convention as ordinary queries.
No proof is constructed in the facade and the ordinary query path is unchanged.
Both regression cases pass for unordered, ascending and descending limits at
N=10 and N=1000, including ties. The 21 existing general facade cases also pass.

Commands: `cargo test -p hoyadb-bindings --test query_scaling --release --offline
--locked` (RED, exit 101), then the same command with `--test general` added
(GREEN, 23 passed, exit 0). Logs: `engine-query-red.log` and
`engine-query-green.log` under `/tmp/hoyadb-followup-20261031/`.
Earlier native artifacts remain bound to their retained policy checkpoint;
these Rust checks do not recertify those artifacts against the changed facade.
