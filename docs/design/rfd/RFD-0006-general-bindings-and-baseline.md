---
id: RFD-0006
title: General consumer bindings reusable testing and measured performance baseline
type: rfd
status: committed
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-31
parents: [ARC-0002, OBJ-0002, OBJ-0004, REQ-0034, NFR-0001, NFR-0005]
tags: [phase-seven, bindings, packaging, baseline]
---

# RFD-0006: General consumer bindings reusable testing and measured performance baseline

Operator resolution on 2026-10-24: ADR-0035 adopts the recommended policies and
records explicit approval of the exact dependency bundle/restricted build policy
and prefixed command-ID clarification. The proposal text below preserves the
pre-approval reasoning; its pending notices are historical, not current gates.

The operator requested full Phase 7 with recommended policies after Phase 6
(commit `c01d2ec`), including final Claude cross-review. This proposal records
those recommendations and the exact implementation gate. New runtime graphs
still need explicit CHG-4 approval; the TC-0016 ID-example conflict remains an
explicit decision. This is preparation, not implementation or a support claim.

## Verified starting point

ROADMAP Phase 7 and IMPLEMENTATION_PLAN Stage9 require general platform bindings,
observability/backoff, TC-0016/0017, consumer packaging and actual performance /
memory/storage/boundary baselines. ARC-0002 requires one Rust semantic core.
Current Rust schemas/migrations/queries/commands/entities/pull are general; the
common CommandFixture hardcodes tables, schema1/2 and synthetic roots and is
explicitly experimental. No npm/SwiftPM/Gradle SDK manifests existed when checked.
API-0010 protects engines/diagnostics but does not certify mobile bindings.

## Recommended policies

### P1 — general facade and thin language packages

Add one reusable Rust facade crate depending only on existing core/memory/
protected SQLite. An engine enum forwards storage traits without reimplementing
queries, validation, merge, reactivity or sync. Test the known first-poll-ready
adapter futures; asynchronous OPFS setup stays outside synchronous owner calls.
One core/facade instance is linked per application process. Public consumer schema,
authored migrations, arbitrary valid names, all value kinds/relations/queries and
all owner operations cross API-0011's exact codecs. Do not promote a fixed-schema
fixture into an SDK or carry fixture temp-path restrictions into consumer APIs.

Use checked C ABI + Swift, narrow C/C++ JNI + Kotlin, and official RN TurboModule
adapters to that same native facade, sharing the transport-independent JS client
with browser worker/OPFS. No new UniFFI/craby/JNA/N-API/runtime generator is needed
for the initial passive scope protocol. Rust owns admission and durable state;
platforms own handlers, transport, timers, cancellation and UI executors.

### P2 — secure keys paths lifetimes and supported platforms

Keep supplied32-byte protected key default; memory is explicit and hermetic.
Open has a separate binary key channel, never a key JSON field or default key.
No implicit unprotected fallback, conversion, reset, SQL/extension/export/rekey.
Borrowed/native buffers are copied only as needed and owned key buffers use the
already-approved zeroization; no claim of erasing every foreign/runtime copy.

Default Apple directory policy remains verified backup exclusion; Android uses
a dedicated child of Context.noBackupFilesDir by default, explicitly filesDir for
backup opt-in; browser backup control remains unsupported. These are platform
controls, not a promise against external copies/compromised apps. Native callers
may supply a dedicated canonical app-owned root; reject traversal/symlink aliases
and same-process duplicate opens before owner publication. Do not add filesystem
touches to memory opens or delete unrelated paths.

Claimed runtime matrix: browser+OPFS, macOS Swift, iOS simulator Swift, Android
emulator Kotlin, and RN on iOS+Android, with actual local packaged consumers.
Build device ABI slices and distinguish build-only from executed hardware. Desktop
shells follow their web/native path; do not claim unexecuted Windows/Linux/Tauri
integration as certified. No task uses/reconfigures the user's existing simulator
or AVD; fixtures use task-owned identities. Remaining same-store multi-process /
cross-tab synchronization stays unsupported and disclosed (RISK0003), not silently
promised; same-process canonical duplicates reject. A lease/leader design requires
separate reviewed scope if added. Version/OS minima are pinned in package metadata
and support tables; actual acceptance cannot be replaced by “source ready.”

### P3 — passive host custom merge with core validation

Existing synchronous Rust MergeHook is a function pointer called while the owner
borrows its pull snapshot. Arbitrary JS/Swift/Kotlin callbacks must not be invoked
there or emulated through private globals/thread-local callback addresses.
Add an owned sequential core merge-plan session: capture expected complete model/
comparison/ledger state, compute the existing default resolution in core, release
all permits/borrows before offering immutable inputs, accept only a candidate row,
and validate it using the exact existing custom-hook validator. No host LWW,
clock/tombstone/authority edits or staged writes escape before atomic commit.
Later changes in the same page see prior validated proposals in core order.
Any intervening state change makes the session stale; discard it and refetch from
the unchanged checkpoint. Busy alone may retry original bytes. Close/reset/abort
retire sessions and input cursors. Explicit authoritative correction continues its
existing no-hook path. API-0011 specifies exact lifecycle and proposed core seam.

### P4 — deterministic IDs without reentrant host calls

Retain Database::with_id_generator and CommandDatabase::set_id_generator.
Add a narrow optional proposed-ID input to the existing insertion path, including
scope/batch/managed-link creation, so the host samples its injected function
before entering Rust. Core allocation/validation/duplicate detection/journaling/
rollback remain authoritative; no host shadow counter or persistent generator
replacement. An absent proposal uses the existing core generator/counter.
Automatic entity IDs retain their strict namespace/counter allocator; host ID
injection does not replace it or bypass shape checks.

**Resolved by ADR-0035 on 2026-10-24:** the operator approved the exact prefixed
command ID and unprefixed ordinary CRUD ID described below. TC-0016 now records
the explicit supersession. The following pre-approval question is retained as
historical context, not a current acceptance hold.

**Historical question — explicit operator clarification still needed:** TC-0016's original feature
expects command-created `test-fixed-id-001`, while API-0006/current finish requires
surviving command-created IDs with `local:` prefix. Do not silently rewrite the
scenario or relax recovery admission. Recommend command test
`local:test-fixed-id-001` (exact supplied value) plus an ordinary unconfigured CRUD
test with exact `test-fixed-id-001`. Preserve the historical scenario and record
its explicit supersession only after this concrete choice is approved. Until
then this acceptance subcase is blocked, not claimed green.

### P5 — existing backoff and observability through every SDK

Preserve SyncOptions defaults capacity1000/history100/initial1000ms/multiplier2/
basecap60000ms,0–2500bp positive jitter, persisted frozen retry bytes and deadlines.
Host production schedulers sample bounded randomness; tests inject clock/jitter.
No second retry state machine, automatic cancellation or network ownership.
Add QueueEntry.attempts and next_due_ms: attempts is persisted u32 count;
next_due_ms is Some(record.due_ms) iff state is Sent, otherwise None.
Status remains payload-free, including no reasons/results/identities. Count-
only safe diagnostics are unchanged and separate. Existing command/entity
correlation, bounded history, outcome/watch and cancel semantics remain API-0006/9.

### P6 — hermetic helpers local packages and compatibility

Local npm tarballs/declarations/WASM assets, SwiftPM/XCFramework and Android AAR,
plus official RN autolink/Codegen metadata, are tested from independent consumer
projects outside the checkout. Names/version are explicitly assigned in API-0011;
no registry publication, push, merge, release or tag is implied. Optional testing
entry point uses real Rust memory engine and same transport seam; fake responses
attach command key or exact entity table/recordId/writeId, never array positions.
Pure owner-independent core JSON tagged decode/encode helpers prevent fake
transport from rounding opaque canonical i64 through JavaScript JSON Numbers.
Explicit retained memory-store handles and consuming core into_storage enable
real hermetic migration/reopen; failed reopen preserves the closed original.
Memory open requires preloaded runtime and accepts an explicitly injected32hex
namespace for tests, so it does not fetch assets or invoke entropy/storage hooks.
Clock/ID/jitter injection is real consumer API. Complete all original TC-0016/17
scenario mappings and applicable prior conformance/security cases.

Establish initial0.1.0 unpublished SDK API snapshots and versioned ABI1 before
release; changes to these published snapshots require governed compatibility
review. Preserve existing Rust/experimental fixtures and on-disk logical formats.
No upgrade reset is default; application-authored migrations and existing missing-
chain behavior stay as reviewed, while recovery/reset guards remain in core.

### P7 — measure before assigning budgets

TC-0037 fixes datasets, optimized encrypted artifacts, timers, sampling and raw
evidence first. Measure API latency, callback cost, memory, complete owned-store
and package footprints on stated hardware/runtime/engine classes. Do not use
contended debug suite duration, plaintext/memory or JSON metadata length as a
protected consumer performance claim. Populate OBJ-0004/NFR-0001 numerical Past/Must/
Target only from reviewed measured repeatability, then enforce hold/improve.
No invented absolute SLO or guessed regression percentage.

## Dependency and Android provider gate

Core/memory remain dependency-free. Swift Foundation/concurrency, platform JNI
and Kotlin stdlib avoid new generators. Candidate RN 0.86.3 / React 19.2.3 / CLI 20.2.0
is a reviewed candidate after the official template CLI 20.1.0 graph exposed
an advisory. CLI 20.2.0 is not yet a proven consumer combination; complete
npm/Maven/Pods/Hermes graph and tools must be locked/reviewed/explicitly approved
before activation. Existing caches are not approval or an offline graph.

Prefer a target archive from approved SQLCipher419.0.6+LibTomCrypt1.18.2 via the
upstream externally-linked static-library route, not private Android libcrypto.
Native LTC shares real pthread layout/configuration; remove weak clock-fallback
entropy dispatch from the build and supply a reviewed public-OS entropy adapter,
not custom cryptography. Prove target-only linking, fail-closed entropy, actual
profile,16KiB ELF/ZIP compatibility and restart/corruption/sidecar behavior.
If a maintained integration cannot remain small, propose the exact vendored
OpenSSL graph separately; do not silently add it. Metadata recipe and primary
sources are in [the Android provider review](../../implementation/phase-seven-android-crypto.md); no compile proof yet.

## Confirmation and rollback

SPEC-0033/API-0011/TC-0036/TC-0037 define required RED→GREEN and actual host gates.
Independent issue-focused Claude review precedes closure. No source revert may
erase/convert/reset stores; preserve files and original keys for a compatible
reader. On-disk format changes, external publication and unexecuted platforms are
not hidden inside packaging work. Phase 6 closure precedes Phase 7 source changes.
