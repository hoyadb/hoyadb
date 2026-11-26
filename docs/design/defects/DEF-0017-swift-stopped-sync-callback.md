---
id: DEF-0017
title: A stopped Swift sync driver can resume into a new application callback
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, THR-0020, TC-0036]
affects: [REQ-0046, THR-0020]
regression_for: [TC-0036]
tags: [phase-seven, swift, scheduler, lifecycle]
---

# DEF-0017: A stopped Swift sync driver can resume into a new application callback

## Source observation and qualification

In the Swift foundation at95bcc96, SyncDriver.call checks liveness and task
cancellation before its request. Its successful branch awaits syncRequest and
the release-wake discard, then returns without checking its stopped state or
cancellation again. Database.syncRequest checks driver identity before awaiting
execute; Database.request checks owner and epoch after its native reply.

Stopping or replacing a sync driver leaves the database owner and epoch intact.
If a successful prepare_push reply is suspended across stopSync, the old call
can therefore return envelopes to cycle, which invokes the old captured
transport before its next cancellation check. The analogous pending-reconciliation
reply can lead to a new invocation of the old captured mapper. These are old
callbacks after retirement, not delivery to replacement callbacks or loss of
the durable queue.

This is a source-only peer-review finding during the benchmark quiet window.
No runtime reproduction has been executed yet. Verify the held-success scenarios
after this preparation is reviewed and committed before implementing a fix.

## Intended correction and limits

Revalidate driver lifetime/cancellation after successful suspended stages and
before starting the next application transport or mapper. Preserve same-owner
current-driver behavior, Busy retention, exact frozen bytes and durable attempts.
Do not add a second queue, repeat a mapper/transport after Busy, or hold a native
lock across a callback. Stopping does not retract a callback already invoked or
a committed prepare_push; its Sent work remains recoverable by the new driver.
No ABI, storage-format, dependency or product-policy change. Rollback is a source
revert with stores and recovery records preserved.

## Regression owed before implementation

TC-0036 G19 needs two deterministic compiling behavioral REDs: hold a successful
prepare reply and, separately, a pending-reconciliation reply; stop or replace
the driver on the same database without close/reset; then release the reply.
Assert zero newly invoked old transport/mapper callbacks, no stale onError, and
correct continued operation of a newly installed driver. Include current-driver
positive controls so rejecting every reply cannot pass. Capture exact RED/GREEN
logs and source/artifact identity; earlier35-test evidence remains historical.

## Resolution evidence

After preparation dac055f, both held-success cases failed by invoking the
retired transport/mapper. The initial null-argument fixture failures are
preserved separately and do not count as these reproductions. SyncDriver.call
now rechecks cancellation and liveness after the successful suspended stages.
The replacement driver retains actual Sent attempts/deadlines and can recover
the pending outcome. All44 Swift tests, separate strict formatting and the
outside-checkout public macOS writer/reader pass on the recorded source cut.
[Exact logs and hashes](../../implementation/phase-seven-swift-public-testing/README.md)
preserve RED/GREEN and current platform limits. Current iOS acceptance remains
open; no iOS result is inferred from these host tests.
