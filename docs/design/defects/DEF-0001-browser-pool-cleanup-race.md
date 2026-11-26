---
id: DEF-0001
title: Browser test cleanup races worker VFS handle release
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [SPEC-0023]
affects: [REQ-0039, SPEC-0023, THR-0013]
tags: [browser, lifecycle]
---

# DEF-0001: Browser pool cleanup race

Observed during real-browser testing of an archive of a19d223: W02/W03 complete
rows and rollback survived page reload, then namespace removeEntry failed with
NoModificationAllowedError after `dispose` and `worker.terminate()`. Earlier
runs passed; no database state failure was observed.

Root mechanism: connection disposal does not close pooled OPFS sync handles.
Checked sqlite-wasm-vfs 0.1.1 sahpool.rs release_access_handles/pause_vfs explicitly
own this resource layer. The harness discarded the install utility and had no
acknowledged VFS shutdown before deleting its files. Termination return did not
establish completion in the observed failure.

SPEC-0024/TC-0024 propose retaining the VFS utility and acknowledging explicit
pool release, then deleting the unique namespace before worker termination.
The regression first failed after disposal with BackendFailure `stale or
disposed handle` because no shutdown operation existed. The fix retains the
approved VFS utility and acknowledges pause_vfs before namespace deletion.
Actual browser L01/L02 now pass: active adapter rejection, unchanged rows,
actual blocked deletion while the pool owns handles, then successful deletion
while the same worker remains alive. No data reset, origin-wide deletion or dependency change is needed.

Fresh asset URLs are used for test worker/JS/WASM loads; the message handler is
installed before awaiting module load so early requests are not lost. The
original failed archive observation is preserved; it is not claimed green.
