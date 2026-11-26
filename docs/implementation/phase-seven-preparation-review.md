# Phase 7 preparation cross-review

Claude Opus 5 reviewed preparation `8d4c74d` before behavior implementation.
The exact response follows; corrections are recorded afterward.

## Phase 7 preparation review — proceed, with 2 mandatory corrections

Scope: API-0011 §8 first slice plus the core/FFI/key/Android-provider boundary decisions it depends on. I checked the contract claims against the actual core code rather than reading the docs alone.

### Mandatory before first-slice TDD

**1. `protection` is undefined for the memory engine.** §4 line 277 fixes the result as `{encrypted:Boolean,cipher:String,version:String,backup:"host_managed"|"unsupported"}`, and §8 puts memory opens in the first slice — but nothing says what a `{kind:"memory"}` owner returns. `backup` has no variant that fits an in-process store (it is neither host-managed nor an unsupported platform control), and `cipher`/`version` have no defined values. An implementer would have to invent all three at TDD time, which is exactly the unreviewed invention the Phase 6 honesty rules exist to prevent — and the failure mode is a consumer reading a misleading `encrypted` or `cipher`. Fix by naming the memory triple explicitly in §4 (and reconciling it with §95's "no production unprotected open", since memory *is* a key-free production engine).

**2. §3 uses three type tokens it never defines, and one sentence contradicts §4.** §3 defines `U`, `I`, `Boolean`, `Handle`, but §3.2/§4 use the literal tokens `U32` (`Schema.version`, `multiplier`, `attempts`), `U16` (`jitter_basis_points`) and `U64` (`drain.now_ms`) in a section that claims to be the exact grammar. Separately, §3 line 138 states "Times are I with API-0006/9's 0..253402300799999 range", while §4 line 296 types `drain {now_ms:U64}`. I checked core: `drain_notifications(now_ms: u64)` and `drain_events(now_ms: u64)` (`owner.rs:653`, `sync.rs:386`) genuinely take `u64`, whereas `prepare_push` is signed — so §4 is right and the §3 blanket sentence is the defect. Fix: define `U32`/`U16`/`U64` as `U` checked to that width, and scope the "Times are I" sentence to the signed operations.

### Scope note, not a blocker

§353 specifies one new core method, `CommandDatabase::into_storage(self)->S`. Reaching the storage needs three: `db` is `pub(super)` on `CommandDatabase`, `ReactiveDatabase.db` is `pub(crate)`, and `Database.storage` is `pub(crate)`. Worth saying so now, so the "no algorithm leaves core" check at review time isn't surprised by two extra consuming accessors.

### Assumptions I verified as sound

- **Memory-store clone seam holds.** `SnapshotStorage` (`snapshot/mod.rs:12-16`) is two `BTreeMap`s with a derived `Clone` — a genuine deep copy, no `Arc` aliasing. §349-352's "clone a candidate, retain the closed original until migration succeeds" is safe; a failed v1→v2 migration cannot corrupt the retained slot.
- **`into_storage` is implementable and leaks nothing.** `CommandDatabase` has no `Drop` impl, so destructuring compiles even though `execution_owner` does. `ExecutionOwner::drop` (`execution.rs:177-190`) removes the owner from the coordinator, drops its queued requests and clears `active` — so §354's "abort/drop execution-owner tickets, then extract" releases the global permit rather than stranding it.
- **Parking the extracted backend cannot persist uncommitted work.** `StagedScope::begin` (`execution.rs:215-247`) copies rows into a *separate* `SnapshotStorage`, and `scope_insert` writes into that shadow's `database_mut()`; the real storage is touched only by `persist()` at finish. A close during an active command therefore parks clean state. §353-358 silently depends on this and the dependency holds.
- **Proposed-ID seam really does leave the enum alone.** Core `Write` is the three-variant enum at `database.rs:21-35`; `apply_batch_proposed(Vec<(Write,Option<String>)>)` carries the proposal alongside rather than inside it. Consistent, and the wire `Write`'s `proposed_id` lowers cleanly.
- **Panic containment precondition is currently satisfied** — the root `Cargo.toml` declares no `[profile]`, so `panic = "unwind"` is in force. §80 correctly states this as a requirement rather than assuming it.
- **C ABI language is honest.** §76-83 explicitly disclaims validating arbitrary address accessibility, disclaims SIGSEGV/foreign-fault containment under `catch_unwind`, and forbids foreign unwind across the boundary. That matches what `catch_unwind` can actually deliver; no overclaim.
- **Android provider proposal is coherent.** Excluding only the upstream entropy-dispatch unit and supplying `rng_get_bytes` backed by OS entropy addresses a real weakness (the LTC chain falls through to a clock-jitter generator when the hook returns 0), fails closed, and the minSDK24 `/dev/urandom` path is specified rather than hand-waved, with an explicit warning against silently linking API28 `getrandom` into an API24 artifact.

### Left untouched, as instructed

TC-0016's unprefixed command-ID example remains an open operator choice; §415 says so and I am not treating it as resolved. Exact RN/Kotlin/new-runtime graph approval is still pending, and nothing here activates it. No edits, no commits, no long suites run — I read code and docs only, and ran nothing in this pass.

## Root resolution

The two mandatory corrections are applied in API-0011 before implementing the
first slice: memory reports `encrypted:false,cipher:"none",version:"not_applicable",
backup:"not_applicable"`; U16/U32/U64 are defined and UTC comparison clocks are
separate from unsigned monotonic drain time and full-i64 row Timestamp cells.
Traceability and RTM checks passed after these contract edits.

The scope note about two extra consuming accessors is unnecessary:
`Database::into_storage` and `ReactiveDatabase::into_storage` already exist in
`crates/hoyadb-core/src/model/{database,reactive}.rs`. Only the proposed consuming
`CommandDatabase::into_storage` is new. This does not change the review's
confirmed storage/permit reasoning. No implementation or mobile support is
inferred from this preparation review.
