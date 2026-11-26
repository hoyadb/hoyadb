---
id: DEF-0062
title: SQLite correction fault fixture assumes whole-ledger packet reinsertion
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-31
updated: 2026-10-31
parents: [SPEC-0047]
affects: [SPEC-0037, API-0007, NFR-0006]
regression_for: [TC-0041, TC-0051]
tags: [sqlite, regression, atomicity]
---

# DEF-0062: SQLite correction fault trigger

## Observed failure

The frozen R9 full workspace run exits 101 in
`command_sync_cases::c09_c12_c16_c19_dispatch_delivery`, at the assertion that
the armed terminal correction returns `Storage(BackendFailure)`.
The retained evidence is `r9-integrated-checks/workspace.log` and `results.json`
under `/tmp/hoyadb-followup-20261031/`; its source capture is unchanged.

The fixture installs a guarded BEFORE INSERT trigger on sync packet fields.
Its history-zero correction maps the application row and trims the command.
The new delta path only persists changed packets, so an unchanged header is
not reinserted. A delete-only packet transition may therefore bypass the
fixture's intended failure point. A focused diagnostic against the unchanged
trigger observed `Ok(())`, confirming that the fixture had not injected an
error. No runtime correction was needed.

## Acceptance

Demonstrate the actual old-trigger result. If it bypasses the intended fault,
arm a trigger on an operation this genuine terminal transition performs, after
application writes but before commit. Require the specific fixture failure,
unchanged application/ledger state, no data or terminal notifications, retained
pending reconciliation, successful retry after disarming, and exact reopened
history. Preserve all existing assertions and use only a task-owned store.
Do not restore full-ledger writes or weaken rollback/error expectations.

## Correction and verification

The trigger now guards deletion of an actual `command:` row from
`__hoya_sync`. Its predicate also requires the mapped application row
`parents/server:fault` to exist, so the specific `fixture correction failure`
BackendFailure proves the application mapping already ran inside the failed
transaction. The assertion requires that exact message. All original rollback,
no-event, pending-reconciliation, retry, terminal-outcome and reopened-history
assertions remain. Exact queue equality, unchanged owner epoch, the successful
mapped row and a strict CommandDatabase reopen strengthen the control.

The original bundled dispatch/correction/graph/zero-history/capacity/terminal
test remains intact. A second focused terminal test uses a distinct temporary
filename, so both can run under the ordinary parallel test scheduler. No
production source, test hook, dependency or runtime behavior changed.

Evidence: `/tmp/hoyadb-followup-20261031/sqlite-terminal-fault/`. The immutable
183-input `red-work`/`red-source.json` diagnostic exits 101 and prints
`armed correction result: Ok(())` in `red.log`. The final source snapshot differs
from the integrated R9 snapshot only in the SQLite test file. Both final test
entries pass under default parallel scheduling (25.81 seconds; `final.log`).
Scoped Clippy with `-D warnings`, source formatting, diff and traceability checks
pass. Claude Fable 5.1 independently reviewed the frozen source and RED/GREEN
evidence and reported no findings; it confirmed this is a stronger fixture for
the actual R9 write path. The reviewer did not rerun tests. Its report is
`sqlite-terminal-fault/fable-review.md` under the same evidence root. The broader
integrated rerun remains a separate gate.
