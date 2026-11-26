# R10 ledger format 3 — runtime, storage guards and API-0009 read-only review (Claude Fable 5.1)

Date: 2026-10-31. Scope: the ten-file import in `r10-codec-work/import-plan.json`
(`import_authorized: false`), bound to `r10-marker-final-work` (182 inputs, packet
`bfdc12a8…103b3`) and `r10-codec-work/final-packet.json` (`7839329f…`). No repository edit; no
build/test run by me. Not a whole-workspace, platform or R11 verdict.

## Binding

- Import plan: every `main_before_sha256` equals current main; every `r10_after_sha256` equals
  the `r10-codec-work` file and, for the nine code/test files, the `r10-marker-final-work` copy.
  `API-0009` exists only in `r10-codec-work` (docs not in the marker snapshot).
- `r10-marker-final-work`: `source-before == source-after` (182); tree matches; owned
  `storage/sync.rs` `3b26c32d…`; `records.rs` `c817f766…` (allocation-free decoder).
  Differences from main are exactly the five runtime files, the four new/updated tests and the
  pre-DEF-0062 `command_sync_cases.rs` (`10cbe772…`, not part of the import; the plan's
  `def0062-test-import.json` keeps main's `07cd9ea4…`).
- Results: core 210 unit + 3 supervised + 6 compile-fail; public acceptance 14; memory and SQLite
  scaling 5 each; core all-target and adapter test-target Clippy `-D warnings` exit 0; fmt exit 0.
  RED: storage guards 8 flip refusals FAIL / 3 controls PASS (`r10-marker-red-work`); codec lane
  `red.log` 10 runtime failures / 4 v2 controls on the unchanged 14-group file; owner proof
  boundary, header helper and internal-format REDs each 1 runtime failure; scaling packet 3+3
  REDs with legacy twins PASS. Codec-work addendum reports 243 + 116 + 118 nearby passing.

## Verified against SPEC-0037 R10 (§654-668) and API-0009

1. **Marker.** `EntityLedger.ledger_format: u8` (entity.rs), `new()` → 3; `header_json` adds
   `ledger_format: 3` only for format 3; `header_from_json` accepts exactly the 7-key set
   (→ 2) or the 8-key set with `ledger_format == 3`, so `null`, `"3"`, `2`, `4` and mixed key
   sets reject; `validate_header_intrinsic` and `entity_header_row` refuse internal values
   outside `2 | 3`. Outer packet `format` stays 2 (acceptance `assert_format` line 149-153).
2. **Identity codec** (records.rs). `identity_metadata_id`: `len(table)+len(id) ≤ MAX_BYTES`,
   `u32_be(len(table)) ‖ table ‖ id` as lowercase hex via a fixed table — injective by the
   fixed-width prefix (SI-F04 pins `00000002616263` vs `00000001616263`).
   `validate_metadata_suffix` format 3: raw length `8 ≤ len ≤ 2·(4+MAX_BYTES)`, even, lowercase
   hex, all before any decode; then length prefix, `table_length == table.len()`,
   remainder `== id.len()`, non-empty id, `check_name(table)`, and a byte-wise `eq` of the hex
   pairs against `table.bytes().chain(id.bytes())` with no allocation. Payload mismatch, odd or
   uppercase hex, overrun, invalid UTF-8 (cannot equal a valid `str`), invalid names, empty id,
   duplicates (`ids` set and `metadata.insert(..).is_some()`), and ordinal packets under a
   format-3 header (length 16 ≠ prefix rule unless it decodes to a matching payload, which the
   equality check refuses) all reject. Format 2 keeps the exact 16-hex ordinal, contiguity and
   ordering checks; identity packets under a v2 header reject (SI-F03 control).
   `std::slice::as_chunks` / `is_multiple_of` are stable on the pinned 1.98.1 toolchain.
3. **Serialization.** `to_rows` emits identity packets and sorts all packets by id for format 3;
   format-2 bytes and order unchanged. Orphan metadata/extension packets without `entity:state`
   reject (records.rs:281). Roundtrip equality pinned for both formats.
4. **Retention through lifecycle.** `EntityHeader` in `LedgerTxn` carries and reinstalls
   `ledger_format` (ledger_txn.rs), so staging, rollback and `edit_entity_header` cannot drop it;
   `reset` and `reconfigure_entities` clone the ledger; adoption of a command-only store uses
   `EntityLedger::new` → 3 (SI-F01). Acceptance `retention(2|3)` covers open, configure, pull,
   PreserveRecovery reset, reconfigure, patch, DiscardRecovery reset, reconfigure, rebind with a
   checkpoint after each step.
5. **No implicit conversion.** Three independent guards: owner `validate_ledger_transition`
   (owner.rs:1039, first check in the `(Some, Some)` arm, new `schema_conflict` message);
   storage delta predicate (`storage/sync.rs:220`), Normal (`:509`) and both Reset modes
   (`:535`) as first disjuncts of existing predicates with unchanged messages and unchanged
   preceding checks (reconfiguration/progress errors keep their order). Adoption
   `(None, Some)` arms untouched. Tests: 2→3 and 3→2 for Normal, both Resets and delta parts,
   each after same-format controls for both formats with exact roundtrip; invalid markers 0/4 in
   prior or next inputs reject; public raw-plan flips reject with the store unchanged.
6. **Bounded owner packet path** (owner.rs:905-917). Format 3: one identity packet per changed
   identity (present → upsert, absent → delete), `retain` on unchanged bytes, no rank changes;
   `metadata_inventory` builds ranks only for format 2, so the v2 suffix algorithm is untouched
   and never consulted for v3. SI-Y02 ten descending pages and SI-F03 middle insertion equal at
   M=10/1000 on both adapters including `sqlite_rows_read`; v2 twins keep the exact `M−i+1`
   suffix and growth control.
7. **Legacy fixtures.** `si_y05_format2_rank_changes…`, storage `sync.rs` revision fixture and
   records `entity_ledger()` now seed format 2 explicitly through JSON/field assignment with
   unchanged assertions, so the new default cannot silently reinterpret ordinal fixtures.
8. **API-0009 text.** Adds `ledger_format` (2|3, intrinsic), marker semantics (absent = 2,
   outer format 2), identity-id layout, decode checks and bound, sorted v3 `to_rows`, retention
   through lifecycle, explicit "does not authorize an online conversion operation";
   `depends_on` gains SPEC-0037. Consistent with the approved spec; no new contract assumption.
   Rust structs are outside the C/JS/RN compatibility manifest, to be documented separately as
   root stated.

## Findings

No actionable defect. Notes:

1. **INFO — error text.** Owner-side flip refusal introduces a new `SchemaConflict` message
   ("entity ledger format cannot change implicitly") while the storage guards reuse existing
   messages. Acceptable (same error kind); mention in the TC-0041 SI-F02 wording if exact
   messages are pinned anywhere.
2. **INFO — evidence ordering.** The 182-input snapshot carries the pre-DEF-0062 SQLite fixture;
   `command_sync_cases` was not run in that packet, so no contradiction, but the integrated gate
   must use main's `07cd9ea4…` fixture (the plan does). The 187-input `r10-integrated-work` run
   is the gate; not treated as passed.
3. **INFO — earlier suggestions closed.** Outer `format == 2` under v3 is asserted in the
   acceptance file; `sqlite_rows_read` equality added to the scaling suites.

## Limits

Not rerun by me. Host Rust only; no AAR/Apple/RN/browser/WASM artifact claim. R11 converter and
its SI-X cases remain a separate decision. Native nearby suites and the full workspace run are
root's pending gates.
