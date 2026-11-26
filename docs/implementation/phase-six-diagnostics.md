# Phase 6 diagnostics and immutable handler context

This dependency-free slice implements API-0010 section 3 under reviewed
preparation commit `6f5ffb4`. It is not evidence that engine encryption, backup
controls, actual protected foreign hosts or the complete Phase 6 gate passed.
No runtime dependency was added or activated by this lane.

`CommandDatabase::diagnostics()` returns exactly `DiagnosticSnapshot`'s ten
numeric/Boolean fields: schema_version, pending, sending, awaiting_outcome,
live, blocked, ready, pages, changes and tombstones. It reads only already-owned
state. Its generic `impl<S>` has no storage capability bound. Queue closure uses
the same existing recovery algorithm through a borrowed helper; identifiers and
payloads are not cloned or serialized. The public owned live-key set retains
its previous semantics and existing retention regressions remain green.

Automatic Debug for the context, result, outcome, recovery, transport, pull,
queue, event and error types explicitly listed in API-0010 now emits static
type/variant text. It does not traverse contents. Typed errors, explicit data
access and complete durable retry serialization remain available unchanged.
The canary regression checks every listed type, all StorageError variants,
nested command errors and event variants, with bounded output and unchanged
recovery serialization.

JavaScript handler context now recursively freezes owned children and exposes
mutation-rejecting Proxy views. Frozen objects alone silently ignore assignments
from non-strict handlers; explicit traps throw for both strict and non-strict
callers. Scope context replacement, nested object/array writes, array mutation,
delete, property definition, prototype replacement and descriptor-value bypass
are covered across an await. The view is not structuredClone-able; callers can
read it or serialize its ordinary JSON representation. The separately captured
durable context is unchanged. Actual outgoing-identity/worker verification stays
with the foreign integration gate.

## TDD and observed checks

Local ignored logs under `target/` retain these compiling behavioral REDs:

- `phase-six-diagnostics-debug-red.txt`: automatic Debug disclosed protected
  fields, reported as type names without printing their contents.
- `phase-six-diagnostics-counts-red.txt`: the default snapshot stub returned
  zeros instead of schema, queue closure and progress counts.
- `phase-six-context-red.txt`: a handler assignment did not throw.

An initial core test setup used an unversioned command name and poisoned its
shared fixture lock. That is not behavioral RED evidence; the versioned fixture
was corrected and both intended failures were reproduced independently above.
A later strengthening added intrinsic ledger validation and caught an invalid
seeded dependency on an already-mapped producer. The fixture was corrected to
the accepted live-undo reference retention case. That intermediate failure in
`phase-six-diagnostics-green.txt` is retained; the final files below supersede
it. No production behavior was changed to accommodate an invalid fixture.

Observed final checks:

| Command | Result | Local log |
| --- | --- | --- |
| `cargo test -p hoyadb-core --lib --offline --locked` | 80 passed, zero failed | phase-six-diagnostics-core-final.txt |
| `node --test spikes/storage-browser/command-client.test.mjs` | 15 passed, zero failed | phase-six-context-final.txt |
| `cargo clippy -p hoyadb-core -p hoyadb-storage-memory --all-targets --offline --locked -- -D warnings` | Passed | phase-six-diagnostics-clippy-final.txt |
| `cargo build -p hoyadb-core -p hoyadb-storage-memory --target wasm32-unknown-unknown --offline --locked` | Passed | phase-six-diagnostics-wasm.txt |
| `rustfmt --check --config-path rustfmt.toml --config skip_children=true` on owned core files, plus owned diff whitespace check | Passed | phase-six-diagnostics-format-final.txt |

The initial Clippy check found a test-only field reassignment after Default;
the initializer was corrected and its failed log remains separate. No warnings
were disabled. Core/memory WASM builds do not activate a cipher provider.

The touched core files are command/{mod,security,owner,entity,records,queue,pull}.rs
and storage.rs; the JS files are command-client.js and command-client.test.mjs.
Pre-existing formatting changes remain preserved. The parent owns the shared
diagnostics RPC and full integration report; no Git commit was made by this lane.
