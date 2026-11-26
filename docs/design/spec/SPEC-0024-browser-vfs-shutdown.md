---
id: SPEC-0024
title: Release browser VFS handles before test namespace cleanup
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [ARC-0002]
satisfies: [REQ-0039]
depends_on: [SPEC-0023, API-0002, DEF-0001]
tags: [browser, lifecycle, regression]
---

# SPEC-0024: Explicit test-worker VFS shutdown

Origin: the authorized SPEC-0023 clean-source verification exposed intermittent
NoModificationAllowedError during unique-namespace deletion after adapter
disposal and worker.terminate(). Data/rollback checks passed first. A second
layer owns resources: sqlite-wasm-vfs 0.1.1 keeps the pool's synchronous access
handles after SQLite connections close. Its checked `pause_vfs()` unregisters
the VFS and synchronously closes every pooled handle, refusing active SQLite
files. Worker termination is not an acknowledged handle-release operation.

The smallest fix is a terminal harness-only `shutdown` request. After disposing
the only adapter, the worker calls the retained approved VFS utility's
`pause_vfs()` and acknowledges success only after it returns. An active adapter
makes shutdown fail without touching rows/handles. Successful shutdown makes
all operations including reinitialization unusable in that worker; then the
main thread removes only its own UUID namespace while the worker is still
alive, proving cleanup does not rely on asynchronous worker termination. The
main thread terminates that worker after successful cleanup. Ordinary restart
cases still actually terminate/recreate workers and do not delete their data.

Keep the namespace marker until cleanup succeeds and expose its name in the
synthetic result object so failed cleanup can be diagnosed. Do not use sleeps,
ignore deletion errors, retry origin-wide deletion, monkeypatch storage APIs,
change browser security or add production failure flags. No runtime dependency,
production storage format, Storage trait or consumer API changes.

TC-0024 records deterministic lifecycle regression cases before code. Observe
RED for the missing shutdown operation, then implement with the reviewed
existing VFS utility and require GREEN in a real browser. Repeat the complete
W01–W09 browser flow and relevant target/host checks. Preserve the actual failed
archive observation; do not overwrite it with a success claim.

The existing STRIDE mitigation boundary is unchanged: owned test messages,
one connection, no personal data, no unsafe code and namespace-only cleanup.
Shutdown rejection is BackendFailure. No promise of immediate OS resource
release after arbitrary worker termination is introduced. Revert only this
harness lifecycle extension on rollback; preserve persistent data for diagnosis.

Rebuild hygiene: the observed browser reused cached module-worker dependencies
across page navigations, despite newly generated assets on the loopback server.
Use a unique query key for each test worker script and its generated JS/WASM
loads, so W09/L02 exercise the just-built source. This is ordinary versioned
asset loading, not a browser cache/security setting change. Keep the same
origin and OPFS namespace across restart/reload; no data fixture is reinserted.
