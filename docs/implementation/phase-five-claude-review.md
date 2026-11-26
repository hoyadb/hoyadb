# Claude Opus 5 Phase 5 checkpoint

One bounded read-only review was requested through the Paseo skill and
`claude/claude-opus-5`, high thinking, auto permission mode, agent
`6661f176-4c3a-48d1-bf1c-35c8c8d588b0`. No profiles were configured.

Verdict: **PASS — no demonstrated defect beyond the already known optimistic
pull/checkpoint issue**. The reviewer checked production core/storage against
API-0009/SPEC-0031 at documentation HEAD `e4e1cf2`. It reviewed the pre-fix pull
source (`c51acf1f60d5d32b` prefix), so this review is not evidence for the later
protected-overlap fix. That fix has a separate contract clarification,
memory/SQLite RED/GREEN, final shared OPFS and actual Swift regression evidence.

Traced areas included strict format-2 packets, revision accounting, complete
reset inventory inside transactions, atomic metadata/mapping rekey, guarded
promotion/correction, merge-hook restrictions, namespaced IDs and frozen wire.
The reviewer ran 78 core tests and memory adapter/shared cases successfully.
It did not claim SQLite/host/WASM/container or governance checks.

Nonblocking observations were distinguished from defects: trim could defensively
clear already-empty metadata undo; equal-millisecond stamps merit future focused
coverage but had no demonstrated wrong result; incoming pages deliberately allow
all declared tables; conservative recovery conflicts require outbound resolution
before refetch. None justifies speculative behavior changes in this phase.

The performance observation is material: one comparison packet is retained per
(table, record ID), and current adapters rewrite the entire logical sync table
per commit. Cost therefore grows with total comparison metadata as well as the
current page/queue. This is an explicit experimental limit, not an optimized
storage or universal latency claim. Tombstones remain until explicit reset.

The full response is retained below with its original qualifications.

No reachable parent agent channel (both sends rejected), so report here.

# PASS — no demonstrable defect beyond known issue

Read-only. Repo untouched. HEAD `e4e1cf2`. `pull.rs` still pre-fix (`c51acf1f`) — scope agent's conservative-overlap change had **not** landed during review.

**Sources reviewed** (sha256): entity.rs `715632089b8e8bfe` · entity_owner.rs `21225b8b340b6ec4` · pull.rs `c51acf1f60d5d32b` · records.rs `8a4cf5ffa42f98fb` · reconcile.rs `cd33873c6d54a42b` · owner.rs `38487b43c3f81688` · command/sync.rs `97aaa3df8c903efb` · queue.rs `3bec45b6a376c2a1` · storage/sync.rs `3538587fcd2eafb9` · snapshot/sync.rs `15f109b8a92b3aaa` · sqlite/sync.rs `dfb93b95ef8efa2d`

## Verified correct vs API-0009 / SPEC-0031 (traced)

- **Format-2 ledger** — prefix/format discrimination, state+record+metadata packet shapes, base-record requirement, empty `command` for entity records, contiguous zero-based ordinals in sorted `(table,id)` order, strict reject on unknown/dup/gap. `records.rs:187-274, 277-308`
- **Revision accounting agrees on both sides** — `persist` bumps iff mutations nonempty or metadata differs (`owner.rs:492-495`); `validate_entity_transition` expects exactly that delta (`storage/sync.rs:187-199`). Empty page advances checkpoint/pages with no bump; both sides consistent. Ticket staleness does fire on any intervening local or command write.
- **Reset** — Remove-only, all guards `Some`, header/namespace/sequence/next_id retained, generation+1 and revision+1, inventory completeness re-checked *inside* the adapter transaction on both adapters. `storage/sync.rs:202-292`, `sqlite/sync.rs:35-43`, `snapshot/sync.rs:48-55`
- **Mapping atomicity** — metadata rekey plus duplicate-target guard before any correction (`reconcile.rs:141-146`); `metadata_undo` rekeyed across all records before `accepted` snapshot, so promotion uses canonical identity.
- **promote_metadata** — traced in both outcome orders for two queued writes to one field. Stamp-equality gating leaves the later optimistic stamp optimistic, rewrites the dependent's undo baseline so its inverse proof still matches, and turns a stale rollback into `RollbackConflict`. Correction clocks and cleared optimistic tombstones survive promotion (struct equality includes the authoritative flag).
- **identity_only shortcut** (`reconcile.rs:126-132`) matches "pure receipt cannot overwrite later local edits"; any corrective write forces the full row+metadata closure proof.
- **Hook clamping** (`entity.rs:315-347`) blocks metadata/presence/identity change and any untouched, tied, or removed field.
- **Frozen wire** — exact key set, no `idempotencyKey`, bytes re-proved against `envelope()` in `validate_record`.
- **Entity IDs** — shape, positive counter, namespace binding, identity-preserving-mapping reference rules incl. survival after producer eviction. `records.rs:353-398, 456-464, 525-536`
- Local writes never call `resolve_entity`; `AuthorityRequired` reachable only via the public pure API since both production callers pass `authoritative=true`.

## Notes — none proven defects, none block freeze

1. **Latent brick, unreachable today** (1-line hardening). `trim` clears `reference_paths/dependencies/created/undo` for retired-unsent cancelled records but not `metadata_undo`, while `validate_record`'s `retired_unsent` predicate (`records.rs:1047`) requires `metadata_undo.is_empty()`. Only production writer of `rolled_back` is `reconcile.rs:365`, which clears `metadata_undo` at 367 — so unreachable. If a future path sets `rolled_back` without clearing it, `trim` clears `created`, `validate()` then rejects on `created.len()!=1`, and every later `to_rows()`/commit fails permanently. Suggest `trim` clear `metadata_undo` too.
2. **Hypothesis + test gap.** `finish_entity_scope` stamps every written field with `context.now_ms` unconditionally (`entity_owner.rs:596-598`). Two queued writes to one field in the same millisecond produce an identical `FieldStamp`, so confirming the first makes `promote_metadata` mark it authoritative while the row holds the second, unconfirmed value. No reachable consequence found — pull always passes `authoritative=true`, and the only authoritative gate is the tombstone-resurrection guard (`reconcile.rs:281-288`), unreachable because `entity_owner.rs:593-595` rejects create/update on a tombstoned record. No conformance case pins same-millisecond stamp reuse.
3. **Conscious contract reading, not a defect.** `apply_pull` accepts any schema table, not only `config.tables` (`pull.rs:325-327`); `EntityLedger::validate` only requires metadata tables exist in schema (`entity.rs:126-133`). `entity_pull.rs:140-145` asserts this deliberately; API-0009 §4 doesn't bind pages to configured tables.
4. **Interaction with the incoming fix** (operational). `protected()`'s `inserting` branch (`pull.rs:470-475`) already conflicts a whole page for any insert into a table holding an unmapped live producer, including entity producers with stable collision-free IDs. Once conservative resolver-equal overlap rejection lands, any queued local write in a table conflicts every page touching that table — bootstrap progresses only if the host drains outbound work first. Worth pinning that "resolve outbound, then refetch" ordering in the host/fixture suite so paged bootstrap can't livelock.
5. **Disclosure number for the Performance row, not a bug.** `to_rows` emits one `entity:metadata` packet per `(table,id)` (`records.rs:296-305`) and both adapters delete+reinsert the whole `__hoya_sync` logical table per commit (`sqlite/sync.rs:58-69`). Every local write and every pull page rewrites O(total comparison metadata) packet rows — the metadata-growth cost SPEC-0031 asks you to measure.

## Evidence

- `cargo test -p hoyadb-core --offline --locked` → 78 passed (2 suites, 6.1s)
- `cargo test -p hoyadb-storage-memory --offline --locked` → exit 0
- `cargo test -p hoyadb-storage-memory --test entity_cases --test pull_cases` → 13 passed

**Not run, so not claimed:** sqlite process/restart suites, spikes, Swift/browser/JS hosts, HTTP fixture, clippy/fmt gates, WASM build, container validation, trace/RTM.
