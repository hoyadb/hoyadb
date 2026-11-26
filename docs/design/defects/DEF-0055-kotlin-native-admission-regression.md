---
id: DEF-0055
title: Kotlin sync regression assumes superseded app-wide native admission
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-31
updated: 2026-10-31
parents: [SPEC-0046]
affects: [SPEC-0037, NFR-0006]
regression_for: [TC-0050]
tags: [review, regression, android, admission]
---

# DEF-0055: Kotlin sync regression assumes app-wide native admission

## Observed failure

The current-source JNI run of `SyncDriverTests` failed K10's assertion that its
reconciliation mapper stays uncalled while a command on a different database is
suspended. A task-owned diagnostic copy preserved every assertion and timeout:
network and blocker latches were both reached; before cancellation, `sends=1`
and `maps=1` failed the old `maps=0` assertion at line 56. The other 20 nearby
groups passed. Logs are in `/tmp/hoyadb-fixes-20261030/` under
`kotlin-nearby-SyncDriverTests.log` and `kotlin-runtime-sync-diagnostic.log`.

## Confirmed cause

Accepted ADR-0037 supersedes app-wide native serialization: the same database
remains serialized while independent databases progress. SPEC-0037 R3 and
`ExecutionCoordinator::for_database` implement separate coordinators for proven
exclusive native stores; WASM/unproven stores retain global admission. The shared
`active_handler_does_not_block` conformance case verifies independent progress.
K10 uses two independent memory databases and therefore no longer creates the
Busy condition it claims. The Android README's unconditional global-FIFO
description is stale for the same reason.

This is a regression-test/documentation defect, not evidence that independent
native progress is wrong. Restoring app-wide core blocking would violate the
accepted decision. Historical passing JNI/consumer reports remain unchanged.

## Acceptance and verification plan

Use the same logical database to exercise actual Busy retention, retaining
single transport/mapper invocation assertions and bounded completion. Add an
independent-database twin that must finish while the other handler remains
suspended. Prefer explicit scheduling barriers to a sleep-only observation.
Update the active Android guide to distinguish per-database native admission
from the conservative/global cases. No runtime behavior or timeout weakening is
required. Run both cases against freshly built current-source JNI, rebuild the
source-bound Kotlin test packet, and obtain Fable review of the correction.

The observed failure above precedes this correction. Final commands, artifacts
and outcomes will be recorded in the remediation report.

## Correction

`SyncDriverTests.verifyAdmission` now runs two cases against the actual native
runtime. The same-database case observes a `receive_wire` Busy reply before
asserting one transport call and zero mapper calls, then cancels the held
handler and requires one final mapper invocation and an Applied outcome. The
independent-database twin requires Applied, one transport call, one mapper call
and no Busy receipt while the other handler's continuation remains suspended.
An explicit native-reply barrier replaces the former 100 ms sleep; timeout
bounds and exact invocation assertions are preserved. No runtime logic changed.

The README now describes per-database FIFO for proven exclusive native stores
and conservative global admission for unproven stores/web. The sync source
comment refers to its database's admission, not a global coordinator.

## Verification

All three `SyncDriverTests` groups passed against the current-source native JNI
on the task-owned x86_64 API37 Android runtime with 16384-byte pages. The same
artifact also passed all 18 `BackgroundErrorTests` groups. Exact commands:

```sh
python3 bindings/android/tools/build.py --out /private/tmp/hoyadb-fixes-20261030/kotlin-admission-reviewed --tests --split-runtime
python3 bindings/android/tools/run-tests.py --out /private/tmp/hoyadb-fixes-20261030/kotlin-admission-reviewed --class-name SyncDriverTests --log /private/tmp/hoyadb-fixes-20261030/kotlin-final-SyncDriverTests.log
python3 bindings/android/tools/run-tests.py --out /private/tmp/hoyadb-fixes-20261030/kotlin-admission-reviewed --class-name BackgroundErrorTests --log /private/tmp/hoyadb-fixes-20261030/kotlin-final-BackgroundErrorTests.log
```

Manifest SHA256 `d969dfbeb11bd83f04b98fc7b93f786ef024c9b890bf2c90cacd8d42086ab94c`
binds all 50 source/tool inputs to the current tree; test DEX SHA256 is
`68bf5d1f6fab115a9531e4305a6db5b01306fd3fd559be12ab3fb4fe6233accf`.
Adjacent runtime JSON records bind both passing logs to those exact artifacts.
The fresh JNI local/device digest is recorded in DEF-0040 and the task report.
No final AAR/ordinary-consumer recertification or arm64 execution is claimed.

Claude Fable 5.1 independently reviewed the correction, verified all 50 current
source/tool hashes and the final manifest/DEX bindings, and confirmed 18 plus
3 passing groups. It found no defect in this correction or masking of a runtime
failure; the two admission cases strengthen coverage of the accepted contract.
The reviewed integration scope is native memory. Review record:
`/tmp/hoyadb-fixes-20261030/fable-review-4.md`. The 19 other nearby passing cases
remain bound to the preceding packet, with unchanged runtime logic.

## Follow-up discovery and acceptance — 2026-10-31

The full current-source Kotlin sweep for SPEC-0047/TC-0051 reproduced the same
obsolete admission assumption in `CommandTests`: K07 expected an independent
owner to remain queued behind a suspended command, and K08 expected raw owner A
to block typed owner B. Both failed; the noncooperative-cancellation case passed,
but its independent-owner successor no longer demonstrated release of A.
The exact original packet is `/tmp/hoyadb-followup-20261031/kotlin/green/`,
bound by `host-freeze.json`; original output is `regression-CommandTests.log`
and its adjacent artifact-binding JSON. No runtime semantic change is indicated.

Before changing these tests, acceptance is: preserve immutable captured
arguments/context and staged invisibility; verify actual same-owner FIFO after a
native false-begin reply, plus an independent-owner progress twin while the
first handler is still suspended; make cancellation's successor use the same
owner; and verify raw/typed same-owner queuing followed by exactly one terminal
wake using a native false-begin barrier. Preserve valid count/result assertions
and timeout bounds. This extends the same ADR-0037 correction; it does not
reinstate app-wide blocking. Original evidence above remains historical.


## Follow-up correction and verification — 2026-10-31

The four `CommandTests` cases now verify same-owner FIFO and immutable capture,
independent-owner progress, a same-owner successor after cancellation of a
noncooperative handler, and raw/typed same-owner admission with exactly one
terminal wake. A wrapper observes the actual native false-begin reply without
altering it; a read-only native-channel barrier makes the pre-release count
assertions deterministic. No production behavior or timeout was changed.

All four passed on freshly built policy-checkpoint JNI on the owned x86_64 API37
runtime with 16384-byte pages. The entire rebuilt packet passed 103 unique cases
in 24 entrypoints. Exact command:

```sh
python3 bindings/android/tools/run-tests.py --out /private/tmp/hoyadb-followup-20261031/kotlin/final-tests --class-name CommandTests --log /private/tmp/hoyadb-followup-20261031/kotlin/final-CommandTests.log
```

`final-tests/manifest.json` SHA256
`ea42cf606acd677ce14198c41a356c314e9eb04e420aeb7c8778a347d15ed2a5`
binds all 53 SDK source/tool inputs. Its test DEX SHA256 is
`9b41eefc4cfcafbcf43e0ad2343658443f063ee066e5fc663085da851c3b0042`.
`final-complete-freeze.json` SHA256
`af6b66c2e9e3a5fd788b9ce7080c6a3cf2fccbdb81042a6420eb030c48b28a20`
binds the complete logs and source inputs. The native checkpoint and separate
25-group packaged-consumer evidence are described in the
[policy verification record](../../implementation/phase-seven-kotlin-policy-followup.md).
The previous Fable review applies to its earlier source packet; this follow-up
has been submitted for separate review.
